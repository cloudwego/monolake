//! # Worker Management and Service Deployment System
//!
//! This module implements a worker management and service deployment system,
//! supporting both single-stage and two-stage deployment processes for services.
//!
//! ## Key Components
//!
//! - [`WorkerManager`]: Manages multiple service deployments across different sites.
//! - [`ServiceDeploymentManager`]: Handles the lifecycle of individual services, including staging
//!   and deployment.
//! - [`WorkerDirective`]: Enum representing various actions that can be performed on services.
//!
//! ## Deployment Process
//!
//! The system supports two deployment models:
//!
//! 1. Two-Stage Deployment:
//!    - Stage a service [`WorkerDirective::StageService`]
//!    - Either update an existing service [`WorkerDirective::UpdateDeployedWithStaged`] or deploy a
//!      new one [`WorkerDirective::DeployNewFromStaged`]
//!
//! 2. Single-Stage Deployment:
//!    - Create and deploy a service in one step [`WorkerDirective::CreateAndDeploy`]
//!
//! ## Asynchronous Execution
//!
//! The system is designed to work with asynchronous service factories and supports
//! asynchronous execution of worker directives.
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
///    - Use [`WorkerDirective::StageService`]
///    - This leverages the `make_via_ref` method from [`AsyncMakeService`], allowing state
///      transfer.
///
/// b) Deployment: Either update an existing service or deploy a new one.
///    - For updates: [`WorkerDirective::UpdateDeployedWithStaged`]
///    - For new deployments: [`WorkerDirective::DeployNewFromStaged`]
///
/// This process allows for careful preparation and validation of the new service
/// before it replaces the existing one, minimizing downtime and preserving valuable state.
///
/// ## 2. Single-Stage Deployment
///
/// This model is suitable for initial deployments or when state preservation isn't necessary:
///
/// - Create and deploy a service in one step using [`WorkerDirective::CreateAndDeploy`]
/// - This is more straightforward but doesn't allow for state transfer from existing services.
///
/// # Worker Thread Execution
///
/// The [`WorkerManager::run_controller`] method serves as the main
/// execution loop, processing [`WorkerDirectiveTask`]s containing
/// [`WorkerDirective`]s. It handles service creation, updates, and removal, coordinating with
/// [`ServiceDeploymentManager`] instances for each site.
pub struct WorkerManager<S> {
    sites: Rc<UnsafeCell<HashMap<Arc<String>, ServiceDeploymentManager<S>>>>,
}

impl<S> Default for WorkerManager<S> {
    fn default() -> Self {
        Self {
            sites: Rc::new(UnsafeCell::new(HashMap::new())),
        }
    }
}

enum WorkerDirectiveError {
    SiteLookupFailed,
    ServiceNotStaged,
    ServiceNotDeployed,
}

impl<S> WorkerManager<S> {
    // Lookup and clone service.
    fn get_svc(&self, name: &Arc<String>) -> Option<Rc<S>> {
        let sites = unsafe { &*self.sites.get() };
        sites.get(name).and_then(|s| s.get_svc())
    }

    // Set parpart slot with given S.
    fn stage_svc(&self, name: Arc<String>, svc: S) {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites
            .entry(name)
            .or_insert_with(ServiceDeploymentManager::new);
        let staged_slot = unsafe { &mut *sh.staged_service.get() };
        *staged_slot = Some(svc);
    }

    fn update_deployed_with_staged(&self, name: &Arc<String>) -> Result<(), WorkerDirectiveError> {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites
            .get_mut(name)
            .ok_or(WorkerDirectiveError::SiteLookupFailed)?;

        let hdr = sh
            .deployed_service
            .as_mut()
            .ok_or(WorkerDirectiveError::ServiceNotDeployed)?;
        let staged_slot = unsafe { &mut *sh.staged_service.get() };
        let staged = staged_slot
            .take()
            .ok_or(WorkerDirectiveError::ServiceNotStaged)?;

        hdr.slot.update_svc(Rc::new(staged));
        Ok(())
    }

    // Apply prepare to handler slot(must be empty).
    fn deploy_staged_service(
        &self,
        name: &Arc<String>,
    ) -> Result<(ServiceSlot<S>, OSender<()>), WorkerDirectiveError> {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites
            .get_mut(name)
            .ok_or(WorkerDirectiveError::SiteLookupFailed)?;
        let staged_slot = unsafe { &mut *sh.staged_service.get() };
        let staged = staged_slot
            .take()
            .ok_or(WorkerDirectiveError::ServiceNotStaged)?;

        let (new_site, stop) = ServiceManager::create(staged);
        let handler_slot = new_site.slot.clone();
        sh.deployed_service = Some(new_site);
        Ok((handler_slot, stop))
    }

    // Remove site.
    fn remove(&self, name: &Arc<String>) -> Result<(), WorkerDirectiveError> {
        let sites = unsafe { &mut *self.sites.get() };
        if sites.remove(name).is_none() {
            Err(WorkerDirectiveError::SiteLookupFailed)
        } else {
            Ok(())
        }
    }

    fn abort(&self, name: &Arc<String>) -> Result<(), WorkerDirectiveError> {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites
            .get_mut(name)
            .ok_or(WorkerDirectiveError::SiteLookupFailed)?;
        let staged_slot = unsafe { &mut *sh.staged_service.get() };
        *staged_slot = None;
        Ok(())
    }
}

/// Manages the deployment lifecycle of an individual service.
///
/// This struct handles both the currently deployed service and any staged service
/// waiting to be deployed. It supports the two-stage deployment process by maintaining
/// separate slots for the deployed and staged services.
///
/// # Type Parameters
///
/// * `S`: The type of the service being managed.
///
/// # Fields
///
/// * `deployed_service`: The currently deployed service, if any.
/// * `staged_service`: A service that has been prepared but not yet deployed.
pub struct ServiceDeploymentManager<S> {
    /// The currently deployed service, if any.
    deployed_service: Option<ServiceManager<S>>,
    /// A service that has been prepared but not yet deployed.
    staged_service: UnsafeCell<Option<S>>,
}

struct ServiceManager<S> {
    slot: ServiceSlot<S>,
    _stop: OReceiver<()>,
}

impl<S> ServiceDeploymentManager<S> {
    const fn new() -> Self {
        Self {
            deployed_service: None,
            staged_service: UnsafeCell::new(None),
        }
    }

    fn get_svc(&self) -> Option<Rc<S>> {
        self.deployed_service.as_ref().map(|h| h.slot.get_svc())
    }
}

impl<S> ServiceManager<S> {
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

/// Represents directives for managing service deployment in a worker.
///
/// This enum encapsulates the various operations that can be performed on services,
/// supporting both two-stage and one-stage deployment processes. It works in conjunction
/// with the [`WorkerManager`] to facilitate the lifecycle management of services.
///
/// The directives align with the concepts introduced in the `service_async` crate,
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
/// 1. [`StageService`](WorkerDirective::StageService): Prepare a service for deployment.
/// 2. Either [`UpdateDeployedWithStaged`](WorkerDirective::UpdateDeployedWithStaged) or
///    [`DeployNewFromStaged`](WorkerDirective::DeployNewFromStaged): Complete the deployment.
///
/// ## One-Stage Deployment
///
/// This model creates and deploys a service in a single step:
///
/// - [`CreateAndDeploy`](WorkerDirective::CreateAndDeploy): Directly create and deploy a service.
#[allow(dead_code)]
#[derive(Clone)]
pub enum WorkerDirective<F, LF> {
    /// Stages a service for deployment without actually deploying it.
    ///
    /// This is the first step in a two-stage deployment process. It leverages the
    /// `make_via_ref` method of [`AsyncMakeService`] to potentially transfer state from
    /// an existing service instance.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service.
    /// * `F` - The factory for creating the service, typically implementing [`AsyncMakeService`].
    StageService(Arc<String>, F),

    /// Updates an existing deployed service with the version that was previously staged.
    ///
    /// This is the second step in a two-stage deployment process for updating existing services.
    /// It allows for a seamless transition from the old service instance to the new one,
    /// potentially preserving state and resources.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service to update.
    UpdateDeployedWithStaged(Arc<String>),

    /// Deploys a previously staged service for the first time.
    ///
    /// This is the second step in a two-stage deployment process for new services.
    /// It's used when a new service has been staged and needs to be activated with
    /// its corresponding listener.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service to deploy.
    /// * `LF` - The listener factory for the service.
    DeployNewFromStaged(Arc<String>, LF),

    /// Creates and deploys a service in a single operation.
    ///
    /// This is used for the one-stage deployment process, suitable for initial deployments
    /// or when state preservation isn't necessary. It combines service creation and
    /// listener setup in one step.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service.
    /// * `F` - The factory for creating the service.
    /// * `LF` - The listener factory for the service.
    CreateAndDeploy(Arc<String>, F, LF),

    /// Aborts the staging process, removing any staged service that hasn't been deployed.
    ///
    /// This is useful for cleaning up staged services that are no longer needed or
    /// were prepared incorrectly.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the staged service to abort.
    AbortStaging(Arc<String>),

    /// Removes a deployed service entirely.
    ///
    /// This directive is used to completely remove a service from the system,
    /// cleaning up all associated resources.
    ///
    /// # Arguments
    /// * `Arc<String>` - The identifier for the service to remove.
    RemoveService(Arc<String>),
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

impl<SE, LE> From<WorkerDirectiveError> for CommandError<SE, LE> {
    fn from(value: WorkerDirectiveError) -> Self {
        match value {
            WorkerDirectiveError::SiteLookupFailed => Self::SiteNotExist,
            WorkerDirectiveError::ServiceNotStaged => Self::PreparationNotExist,
            WorkerDirectiveError::ServiceNotDeployed => Self::PreviousHandlerNotExist,
        }
    }
}

/// Represents a task encapsulating a worker directive and a channel for its execution result.
///
/// This struct combines a [`WorkerDirective`](WorkerDirective) with a mechanism to send back the
/// result of its execution. It's used to queue tasks for the worker thread to process and
/// allows for asynchronous communication of the task's outcome.
///
/// # Type Parameters
///
/// * `F`: The type of the service factory used in the [`WorkerDirective`](WorkerDirective).
/// * `LF`: The type of the listener factory used in the [`WorkerDirective`](WorkerDirective).
pub struct WorkerDirectiveTask<F, LF> {
    cmd: WorkerDirective<F, LF>,
    result: OSender<Result<(), AnyError>>,
}

impl<F, LF> WorkerDirectiveTask<F, LF> {
    pub fn new(cmd: WorkerDirective<F, LF>) -> (Self, OReceiver<Result<(), AnyError>>) {
        let (tx, rx) = ochannel();
        (Self { cmd, result: tx }, rx)
    }
}

pub trait Execute<A, S> {
    type Error: Into<AnyError>;
    fn execute(
        self,
        controller: &WorkerManager<S>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>>;
}

impl<F, LF, A, E, S> Execute<A, S> for WorkerDirective<F, LF>
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
    async fn execute(self, controller: &WorkerManager<S>) -> Result<(), Self::Error> {
        match self {
            WorkerDirective::StageService(name, factory) => {
                let current_svc = controller.get_svc(&name);
                let svc = factory
                    .make_via_ref(current_svc.as_deref())
                    .await
                    .map_err(CommandError::BuildService)?;
                controller.stage_svc(name, svc);
                Ok(())
            }
            WorkerDirective::UpdateDeployedWithStaged(name) => {
                controller.update_deployed_with_staged(&name)?;
                Ok(())
            }
            WorkerDirective::DeployNewFromStaged(name, listener_factory) => {
                let listener = listener_factory
                    .make()
                    .await
                    .map_err(CommandError::BuildListener)?;
                let (hdr, stop) = controller.deploy_staged_service(&name)?;
                monoio::spawn(serve(listener, hdr, stop));
                Ok(())
            }
            WorkerDirective::CreateAndDeploy(name, factory, listener_factory) => {
                let svc = factory.make().await.map_err(CommandError::BuildService)?;
                let listener = listener_factory
                    .make()
                    .await
                    .map_err(CommandError::BuildListener)?;
                controller.stage_svc(name.clone(), svc);
                let (hdr, stop) = controller.deploy_staged_service(&name)?;
                monoio::spawn(serve(listener, hdr, stop));
                Ok(())
            }
            WorkerDirective::AbortStaging(name) => {
                controller.abort(&name)?;
                Ok(())
            }
            WorkerDirective::RemoveService(name) => {
                controller.remove(&name)?;
                Ok(())
            }
        }
    }
}

impl<S> WorkerManager<S> {
    /// Runs the main control loop for the worker thread.
    ///
    /// This method continuously processes incoming [`WorkerDirective`]s and executes
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
    /// * `rx`: A receiver channel for `Update`s containing [`WorkerDirective`]s
    ///
    /// This method will run until the receiver channel is closed.
    pub async fn run_controller<F, LF, A>(&self, mut rx: Receiver<WorkerDirectiveTask<F, LF>>)
    where
        WorkerDirective<F, LF>: Execute<A, S>,
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
