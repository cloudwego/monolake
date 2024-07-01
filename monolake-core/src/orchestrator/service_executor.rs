//! # Worker Management and Service Deployment System
//!
//! This module implements a worker management and service deployment system,
//! supporting both single-stage and two-stage deployment processes for services.
//!
//! ## Key Components
//!
//! - [`ServiceExecutor`]: Manages multiple service deployments across different sites.
//! - [`ServiceDeploymentContainer`]: Handles the lifecycle of individual services, including
//!   precommitting and deployment.
//! - [`ServiceCommand`]: Enum representing various actions that can be performed on services.
//!
//! ## Deployment Process
//!
//! The system supports two deployment models:
//!
//! 1. Two-Stage Deployment:
//!    - Precommit a service [`ServiceCommand::Precommit`]
//!    - Either update an existing service [`ServiceCommand::Update`] or commit a new one
//!      [`ServiceCommand::Commit`]
//!
//! 2. Single-Stage Deployment:
//!    - Create and deploy a service in one step [`ServiceCommand::PrepareAndCommit`]
//!
//! ## Asynchronous Execution
//!
//! The system is designed to work with asynchronous service factories and supports
//! asynchronous execution of service commands.
use std::{cell::UnsafeCell, collections::HashMap, fmt::Debug, rc::Rc, sync::Arc};

use futures_channel::{
    mpsc::Receiver,
    oneshot::{channel as ochannel, Receiver as OReceiver, Sender as OSender},
};
use futures_util::stream::StreamExt;
use monoio::io::stream::Stream;
use service_async::{AsyncMakeService, Service};
use tracing::error;

use super::serve;
use crate::AnyError;

/// Manages multiple service deployments across different sites within a worker thread.
///
/// # Context from service_async
///
/// The `service_async` crate introduces a refined [`Service`] trait that leverages `impl Trait`
/// for improved performance and flexibility. It also provides the [`AsyncMakeService`] trait,
/// which allows for efficient creation and updating of services, particularly useful
/// for managing stateful resources across service updates.
///
/// # State Transfer Usefulness
///
/// State transfer can be particularly useful in scenarios such as:
///
/// 1. Database Connection Pools: When updating a service that manages database connections,
///    transferring the existing pool can maintain active connections, avoiding the overhead of
///    establishing new ones.
///
/// 2. In-Memory Caches: For services with large caches, transferring the cache state can prevent
///    performance dips that would occur if the cache had to be rebuilt from scratch.
///
/// # Service Deployment Models
///
/// This system supports two deployment models:
///
/// ## 1. Two-Stage Deployment
///
/// This model is ideal for updating services while preserving state:
///
/// a) Staging: Prepare a new service instance, potentially using state from an existing service.
///    - Use [`ServiceCommand::Precommit`]
///    - This leverages the `make_via_ref` method from [`AsyncMakeService`], allowing state
///      transfer.
///
/// b) Deployment: Either update an existing service or deploy a new one.
///    - For updates: [`ServiceCommand::Update`]
///    - For new deployments: [`ServiceCommand::Commit`]
///
/// This process allows for careful preparation and validation of the new service
/// before it replaces the existing one, minimizing downtime and preserving valuable state.
///
/// ## 2. Single-Stage Deployment
///
/// This model is suitable for initial deployments or when state preservation isn't necessary:
///
/// - Create and deploy a service in one step using [`ServiceCommand::PrepareAndCommit`]
/// - This is more straightforward but doesn't allow for state transfer from existing services.
///
/// # Worker Thread Execution
///
/// The [`ServiceExecutor::run`] method serves as the main
/// execution loop, processing [`ServiceCommandTask`]s containing
/// [`ServiceCommand`]s. It handles service creation, updates, and removal, coordinating with
/// [`ServiceDeploymentContainer`] instances for each site.
pub struct ServiceExecutor<S> {
    sites: Rc<UnsafeCell<HashMap<Arc<String>, ServiceDeploymentContainer<S>>>>,
}

impl<S> Default for ServiceExecutor<S> {
    fn default() -> Self {
        Self {
            sites: Rc::new(UnsafeCell::new(HashMap::new())),
        }
    }
}

enum ServiceCommandError {
    SiteLookupFailed,
    ServiceNotStaged,
    ServiceNotDeployed,
}

impl<S> ServiceExecutor<S> {
    // Lookup and clone service.
    fn get_svc(&self, name: &Arc<String>) -> Option<Rc<S>> {
        let sites = unsafe { &*self.sites.get() };
        sites.get(name).and_then(|s| s.get_svc())
    }

    // Set parpart slot with given S.
    fn precommit_svc(&self, name: Arc<String>, svc: S) {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites
            .entry(name)
            .or_insert_with(ServiceDeploymentContainer::new);
        let precom_svc_slot = unsafe { &mut *sh.precommitted_service.get() };
        *precom_svc_slot = Some(svc);
    }

    fn update_with_precommitted_svc(&self, name: &Arc<String>) -> Result<(), ServiceCommandError> {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites
            .get_mut(name)
            .ok_or(ServiceCommandError::SiteLookupFailed)?;

        let hdr = sh
            .committed_service
            .as_mut()
            .ok_or(ServiceCommandError::ServiceNotDeployed)?;
        let precom_svc_slot = unsafe { &mut *sh.precommitted_service.get() };
        let precom_svc = precom_svc_slot
            .take()
            .ok_or(ServiceCommandError::ServiceNotStaged)?;

        hdr.slot.update_svc(Rc::new(precom_svc));
        Ok(())
    }

    // Apply prepare to handler slot(must be empty).
    fn deploy_staged_service(
        &self,
        name: &Arc<String>,
    ) -> Result<(ServiceSlot<S>, OSender<()>), ServiceCommandError> {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites
            .get_mut(name)
            .ok_or(ServiceCommandError::SiteLookupFailed)?;
        let precom_svc_slot = unsafe { &mut *sh.precommitted_service.get() };
        let precom_svc = precom_svc_slot
            .take()
            .ok_or(ServiceCommandError::ServiceNotStaged)?;

        let (new_site, stop) = ServiceSlotContainer::create(precom_svc);
        let handler_slot = new_site.slot.clone();
        sh.committed_service = Some(new_site);
        Ok((handler_slot, stop))
    }

    // Remove site.
    fn remove(&self, name: &Arc<String>) -> Result<(), ServiceCommandError> {
        let sites = unsafe { &mut *self.sites.get() };
        if sites.remove(name).is_none() {
            Err(ServiceCommandError::SiteLookupFailed)
        } else {
            Ok(())
        }
    }

    fn abort(&self, name: &Arc<String>) -> Result<(), ServiceCommandError> {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites
            .get_mut(name)
            .ok_or(ServiceCommandError::SiteLookupFailed)?;
        let precom_svc_slot = unsafe { &mut *sh.precommitted_service.get() };
        *precom_svc_slot = None;
        Ok(())
    }
}

/// Manages the deployment lifecycle of an individual service.
///
/// This struct handles both the currently committed service and any precommit service
/// waiting to be deployed. It supports the two-stage deployment process by maintaining
/// separate slots for the commit and precommit services.
///
/// # Type Parameters
///
/// * `S`: The type of the service being managed.
///
/// # Fields
///
/// * `deployed_service`: The currently deployed service, if any.
/// * `staged_service`: A service that has been prepared but not yet deployed.
pub struct ServiceDeploymentContainer<S> {
    /// The currently deployed service, if any.
    committed_service: Option<ServiceSlotContainer<S>>,
    /// A service that has been prepared but not yet deployed.
    precommitted_service: UnsafeCell<Option<S>>,
}

struct ServiceSlotContainer<S> {
    slot: ServiceSlot<S>,
    _stop: OReceiver<()>,
}

impl<S> ServiceDeploymentContainer<S> {
    const fn new() -> Self {
        Self {
            committed_service: None,
            precommitted_service: UnsafeCell::new(None),
        }
    }

    fn get_svc(&self) -> Option<Rc<S>> {
        self.committed_service.as_ref().map(|h| h.slot.get_svc())
    }
}

impl<S> ServiceSlotContainer<S> {
    fn create(handler: S) -> (Self, OSender<()>) {
        let (tx, rx) = ochannel();
        (
            Self {
                slot: ServiceSlot::from(Rc::new(handler)),
                _stop: rx,
            },
            tx,
        )
    }
}

/// Holds the deployed  [`Service`]
pub struct ServiceSlot<S>(Rc<UnsafeCell<Rc<S>>>);

impl<S> Clone for ServiceSlot<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> From<Rc<S>> for ServiceSlot<S> {
    fn from(value: Rc<S>) -> Self {
        Self(Rc::new(UnsafeCell::new(value)))
    }
}

impl<S> From<Rc<UnsafeCell<Rc<S>>>> for ServiceSlot<S> {
    fn from(value: Rc<UnsafeCell<Rc<S>>>) -> Self {
        Self(value)
    }
}

impl<S> ServiceSlot<S> {
    pub fn update_svc(&self, shared_svc: Rc<S>) {
        unsafe { *self.0.get() = shared_svc };
    }

    pub fn get_svc(&self) -> Rc<S> {
        unsafe { &*self.0.get() }.clone()
    }
}

/// Represents commands for managing service deployment in a worker.
///
/// This enum encapsulates the various operations that can be performed on services,
/// supporting both two-stage and one-stage deployment processes. It works in conjunction
/// with the [`ServiceExecutor`] to facilitate the lifecycle management of services.
///
/// The commands align with the concepts introduced in the `service_async` crate,
/// particularly leveraging the [`AsyncMakeService`] trait for efficient service creation
/// and updates.
///
/// # Type Parameters
///
/// * `F`: The service factory type, typically implementing [`AsyncMakeService`].
/// * `LF`: The listener factory type, used for creating service listeners.
///
/// # Deployment Models
///
/// ## Two-Stage Deployment
///
/// This model allows for state transfer and careful preparation before deployment:
///
/// 1. [`Precommit`](ServiceCommand::Precommit): Prepare a service for deployment.
/// 2. Either [`Update`](ServiceCommand::Update) or [`Commit`](ServiceCommand::Commit): Complete the
///    deployment.
///
/// ## One-Stage Deployment
///
/// This model creates and deploys a service in a single step:
///
/// - [`PrepareAndCommit`](ServiceCommand::PrepareAndCommit): Directly create and deploy a service.
///
/// Each variant of this enum represents a specific action in the service lifecycle,
/// providing fine-grained control over service deployment and management.
#[allow(dead_code)]
#[derive(Clone)]
pub enum ServiceCommand<F, LF> {
    /// Precommits a service for deployment without actually deploying it.
    ///
    /// This is the first step in a two-stage deployment process. It leverages the
    /// `make_via_ref` method of [`AsyncMakeService`] to potentially transfer state from
    /// an existing service instance.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service.
    /// * `F` - The factory for creating the service, typically implementing [`AsyncMakeService`].
    Precommit(Arc<String>, F),

    /// Updates an existing deployed service with the version that was previously precommitted.
    ///
    /// This is the second step in a two-stage deployment process for updating existing services.
    /// It allows for a seamless transition from the old service instance to the new one,
    /// potentially preserving state and resources.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service to update.
    Update(Arc<String>),

    /// Commits a previously precommitted service for the first time.
    ///
    /// This is the second step in a two-stage deployment process for new services.
    /// It's used when a new service has been precommitted and needs to be activated with
    /// its corresponding listener.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service to commit.
    /// * `LF` - The listener factory for the service.
    Commit(Arc<String>, LF),

    /// Prepares and commits a service in a single operation.
    ///
    /// This is used for the one-stage deployment process, suitable for initial deployments
    /// or when state preservation isn't necessary. It combines service creation and
    /// listener setup in one step.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service.
    /// * `F` - The factory for creating the service.
    /// * `LF` - The listener factory for the service.
    PrepareAndCommit(Arc<String>, F, LF),

    /// Aborts the precommit process, removing any precommitted service that hasn't been deployed.
    ///
    /// This is useful for cleaning up precommitted services that are no longer needed or
    /// were prepared incorrectly.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the precommitted service to abort.
    Abort(Arc<String>),

    /// Removes a deployed service entirely.
    ///
    /// This directive is used to completely remove a service from the system,
    /// cleaning up all associated resources.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service to remove.
    Remove(Arc<String>),
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError<SE, LE> {
    #[error("build service error: {0:?}")]
    BuildService(SE),
    #[error("build listener error: {0:?}")]
    BuildListener(LE),
    #[error("site not exist")]
    SiteNotExist,
    #[error("preparation not exist")]
    PreparationNotExist,
    #[error("previous handler not exist")]
    PreviousHandlerNotExist,
}

impl<SE, LE> From<ServiceCommandError> for CommandError<SE, LE> {
    fn from(value: ServiceCommandError) -> Self {
        match value {
            ServiceCommandError::SiteLookupFailed => Self::SiteNotExist,
            ServiceCommandError::ServiceNotStaged => Self::PreparationNotExist,
            ServiceCommandError::ServiceNotDeployed => Self::PreviousHandlerNotExist,
        }
    }
}

/// Represents a task encapsulating a [`ServiceCommand`] and a channel for its execution result.
///
/// This struct combines a [`ServiceCommand`] with a mechanism to send back the
/// result of its execution. It's used to queue tasks for the worker thread to process and
/// allows for asynchronous communication of the task's outcome.
///
/// # Type Parameters
///
/// * `F`: The type of the service factory used in the [`ServiceCommand`].
/// * `LF`: The type of the listener factory used in the [`ServiceCommand`].
pub struct ServiceCommandTask<F, LF> {
    cmd: ServiceCommand<F, LF>,
    result: OSender<Result<(), AnyError>>,
}

impl<F, LF> ServiceCommandTask<F, LF> {
    pub fn new(cmd: ServiceCommand<F, LF>) -> (Self, OReceiver<Result<(), AnyError>>) {
        let (tx, rx) = ochannel();
        (Self { cmd, result: tx }, rx)
    }
}

/// A trait for executing service commands within a `ServiceExecutor`.
///
/// This trait defines the interface for executing various service-related commands,
/// such as staging, updating, or removing services.
pub trait Execute<A, S> {
    type Error: Into<AnyError>;
    fn execute(
        self,
        controller: &ServiceExecutor<S>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>>;
}

impl<F, LF, A, E, S> Execute<A, S> for ServiceCommand<F, LF>
where
    F: AsyncMakeService<Service = S>,
    F::Error: Debug + Send + Sync + 'static,
    LF: AsyncMakeService,
    LF::Service: Stream<Item = Result<A, E>> + 'static,
    E: Debug + Send + Sync + 'static,
    LF::Error: Debug + Send + Sync + 'static,
    S: Service<A> + 'static,
    S::Error: Debug,
    A: 'static,
{
    type Error = CommandError<F::Error, LF::Error>;
    async fn execute(self, controller: &ServiceExecutor<S>) -> Result<(), Self::Error> {
        match self {
            ServiceCommand::Precommit(name, factory) => {
                let current_svc = controller.get_svc(&name);
                let svc = factory
                    .make_via_ref(current_svc.as_deref())
                    .await
                    .map_err(CommandError::BuildService)?;
                controller.precommit_svc(name, svc);
                Ok(())
            }
            ServiceCommand::Update(name) => {
                controller.update_with_precommitted_svc(&name)?;
                Ok(())
            }
            ServiceCommand::Commit(name, listener_factory) => {
                let listener = listener_factory
                    .make()
                    .await
                    .map_err(CommandError::BuildListener)?;
                let (hdr, stop) = controller.deploy_staged_service(&name)?;
                monoio::spawn(serve(listener, hdr, stop));
                Ok(())
            }
            ServiceCommand::PrepareAndCommit(name, factory, listener_factory) => {
                let svc = factory.make().await.map_err(CommandError::BuildService)?;
                let listener = listener_factory
                    .make()
                    .await
                    .map_err(CommandError::BuildListener)?;
                controller.precommit_svc(name.clone(), svc);
                let (hdr, stop) = controller.deploy_staged_service(&name)?;
                monoio::spawn(serve(listener, hdr, stop));
                Ok(())
            }
            ServiceCommand::Abort(name) => {
                controller.abort(&name)?;
                Ok(())
            }
            ServiceCommand::Remove(name) => {
                controller.remove(&name)?;
                Ok(())
            }
        }
    }
}

impl<S> ServiceExecutor<S> {
    /// Runs the main control loop for the worker thread.
    ///
    /// This method continuously processes incoming [`ServiceCommand`]s and executes
    /// the corresponding actions on the managed services.
    ///
    /// # Type Parameters
    ///
    /// - `F`: The service factory type
    /// - `LF`: The listener factory type
    /// - `A`: The type of the argument passed to the service
    ///
    /// # Arguments
    ///
    /// * `rx`: A receiver channel for `Update`s containing [`ServiceCommand`]s
    ///
    /// This method will run until the receiver channel is closed.
    pub async fn run<F, LF, A>(&self, mut rx: Receiver<ServiceCommandTask<F, LF>>)
    where
        ServiceCommand<F, LF>: Execute<A, S>,
    {
        while let Some(upd) = rx.next().await {
            if let Err(e) = upd
                .result
                .send(upd.cmd.execute(self).await.map_err(Into::into))
            {
                error!("unable to send back result: {e:?}");
            }
        }
    }
}
