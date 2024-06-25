//! Worker and service lifecycle management for thread-per-core network services.
//!
//! This module provides the core functionality for managing workers and services
//! in a thread-per-core architecture. It implements a flexible and efficient system
//! for deploying, updating, and managing services across multiple worker threads.
//!
//! # Key Components
//!
//! - [`WorkerFleetOrchestrator`]: Manages the entire fleet of worker threads.
//! - [`WorkerManager`]: Handles service lifecycle within a single worker thread.
//! - [`ServiceDeploymentManager`]: Manages the deployment and updates of individual services.
//! - [`WorkerDirective`]: Represents actions to be performed on services.
//! - [`ResultGroup`]: Aggregates results from operations across multiple workers.
//!
//! # Deployment Models
//!
//! This module supports two deployment models:
//!
//! 1. Two-Stage Deployment: For updating services with state preservation.
//!    - Stage a service using [`WorkerDirective::StageService`].
//!    - Update or deploy using [`WorkerDirective::UpdateDeployedWithStaged`] or
//!      [`WorkerDirective::DeployNewFromStaged`].
//!
//! 2. Single-Stage Deployment: For initial deployments or stateless updates.
//!    - Deploy in one step using [`WorkerDirective::CreateAndDeploy`].
//!
//! # Service Lifecycle
//!
//! Services can be dynamically updated while the system is running:
//! - Existing connections continue using the current service version.
//! - New connections use the latest deployed version.
//!
//! This module is designed to work seamlessly with the `service_async` crate,
//! leveraging its [`Service`] and [`AsyncMakeService`](service_async::AsyncMakeService)
//! traits for efficient service creation and management.
use std::fmt::Debug;

use futures_channel::oneshot::Sender as OSender;
use monoio::io::stream::Stream;
use service_async::Service;
use tracing::{debug, error, info, warn};

use self::runtime::RuntimeWrapper;

mod runtime;
mod worker_fleet_orchestrator;
mod worker_manager;

pub use worker_fleet_orchestrator::{JoinHandlesWithOutput, WorkerFleetOrchestrator};
pub use worker_manager::{
    Execute, ServiceDeploymentManager, ServiceSlot, WorkerDirective, WorkerDirectiveTask,
    WorkerManager,
};

/// A collection of results from multiple worker operations.
///
/// [`ResultGroup`] is typically used to aggregate the results of dispatching
/// a [`WorkerDirective`] to multiple workers in a [`WorkerFleetOrchestrator`].
/// It provides a convenient way to handle and process multiple results as a single unit.
pub struct ResultGroup<T, E>(Vec<Result<T, E>>);

impl<T, E> From<Vec<Result<T, E>>> for ResultGroup<T, E> {
    fn from(value: Vec<Result<T, E>>) -> Self {
        Self(value)
    }
}

impl<T, E> From<ResultGroup<T, E>> for Vec<Result<T, E>> {
    fn from(value: ResultGroup<T, E>) -> Self {
        value.0
    }
}

impl<E> ResultGroup<(), E> {
    pub fn err(self) -> Result<(), E> {
        for r in self.0.into_iter() {
            r?;
        }
        Ok(())
    }
}

pub async fn serve<S, Svc, A, E>(mut listener: S, handler: ServiceSlot<Svc>, mut stop: OSender<()>)
where
    S: Stream<Item = Result<A, E>> + 'static,
    E: Debug,
    Svc: Service<A> + 'static,
    Svc::Error: Debug,
    A: 'static,
{
    let mut cancellation = stop.cancellation();
    loop {
        monoio::select! {
            _ = &mut cancellation => {
                info!("server is notified to stop");
                break;
            }
            accept_opt = listener.next() => {
                let accept = match accept_opt {
                    Some(accept) => accept,
                    None => {
                        info!("listener is closed, serve stopped");
                        return;
                    }
                };
                match accept {
                    Ok(accept) => {
                        let svc = handler.get_svc();
                        monoio::spawn(async move {
                            match svc.call(accept).await {
                                Ok(_) => {
                                    debug!("Connection complete");
                                }
                                Err(e) => {
                                    error!("Connection error: {e:?}");
                                }
                            }
                        });
                    }
                    Err(e) => warn!("Accept connection failed: {e:?}"),
                }
            }
        }
    }
}
