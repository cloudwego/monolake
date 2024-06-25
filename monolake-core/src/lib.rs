//! # monolake-core
//!
//! `monolake-core` is a foundational crate for building high-performance, thread-per-core network
//! services. It provides a robust framework for worker orchestration, service deployment, and
//! lifecycle management, supporting protocols such as HTTP and Thrift. This crate builds upon
//! concepts from the `service_async` crate to implement a thread-per-core worker system with
//! advanced service management capabilities.
//!
//! ## Key Features
//!
//! - **Network Service Foundation**: Core building blocks for creating efficient network services.
//! - **Thread-per-Core Architecture**: Maximizes performance on multi-core processors.
//! - **Service Lifecycle Management**: Seamless updates and deployments of service chains.
//! - **Flexible Deployment Models**: Support for both single-stage and two-stage service deployment
//!   processes.
//! - **State Transfer**: Facilitate updates with state preservation between service versions.
//! - **Protocol Support**: Built-in support for HTTP and Thrift protocols.
//! - **Asynchronous Design**: Leverages Rust's async capabilities for efficient, non-blocking
//!   operations.
//!
//! ## Service and Service Factory Concepts
//!
//! This crate builds upon the `service_async` crate, providing:
//!
//! - A refined [`Service`](service_async::Service) trait that leverages `impl Trait` for improved
//!   performance and flexibility.
//! - The [`MakeService`](service_async::MakeService) trait for efficient creation and updating of
//!   services, particularly useful for managing stateful resources across service updates.
//!
//! `monolake-core` extends these concepts to provide a comprehensive system for managing network
//! services in a thread-per-core architecture.
//!
//! ## Pre-built Services
//!
//! While `monolake-core` provides the foundation, you can find pre-built services for common
//! protocols in the `monolake-services` crate. This includes ready-to-use implementations for:
//!
//! - HTTP services
//! - Thrift services
//!
//! These pre-built services can be easily integrated into your `monolake-core` based applications,
//! speeding up development for standard network service scenarios.
//!
//! ## Worker-Service Lifecycle Management
//!
//! The core of this crate is the worker-service lifecycle management system, implemented in the
//! [`worker_service_lifecycle`] module. Key components include:
//!
//! - [`WorkerFleetOrchestrator`](worker_service_lifecycle::WorkerFleetOrchestrator): Manages
//!   multiple worker threads, each running on a dedicated CPU core.
//! - [`WorkerManager`](worker_service_lifecycle::WorkerManager): Handles the lifecycle of services
//!   within a single worker thread.
//! - [`ServiceDeploymentManager`](worker_service_lifecycle::ServiceDeploymentManager) Manages
//!   individual service instances, including staging and deployment.
//! - [`WorkerDirective`](worker_service_lifecycle::WorkerDirective): Represents actions to be
//!   performed on services, such as staging, updating, or removing.
//!
//! This system supports dynamic updating of deployed services:
//!
//! - You can update a currently deployed service with a new service chain.
//! - Existing connections continue to use the old service chain.
//! - New connections automatically use the latest service chain.
//!
//! This approach ensures smooth transitions during updates with minimal disruption to ongoing
//! operations.
//!
//! ## Deployment Models
//!
//! The system supports two deployment models:
//!
//! 1. **Two-Stage Deployment**: Ideal for updating services while preserving state.
//!    - Stage a service
//!      [`WorkerDirective::StageService`](worker_service_lifecycle::WorkerDirective::StageService)
//!    - Update [`WorkerDirective::UpdateDeployedWithStaged`](worker_service_lifecycle::WorkerDirective::UpdateDeployedWithStaged)
//!      or deploy [`WorkerDirective::DeployNewFromStaged`](worker_service_lifecycle::WorkerDirective::DeployNewFromStaged)
//!
//! 2. **Single-Stage Deployment**: Suitable for initial deployments or when state preservation
//!    isn't necessary.
//!    - Create and deploy in one step
//!      [`WorkerDirective::CreateAndDeploy`](worker_service_lifecycle::WorkerDirective::CreateAndDeploy)
//!
//! ## Protocol Handlers
//!
//! ### HTTP Handler
//!
//! The [`http`] module provides the [`HttpHandler`](http::HttpHandler) trait for
//! implementing HTTP request handlers. It supports context-aware handling and connection
//! management.
//!
//! ### Thrift Handler
//!
//! The [`thrift`] module offers the [`ThriftHandler`](thrift::ThriftHandler) trait for
//! implementing Thrift request handlers.
//!
//! Both handler traits are automatically implemented for types that implement the
//! [`Service`](service_async::Service) trait with appropriate request and response types.
//!
//! ## Usage Example
//!
//! ```ignore
//!    let mut manager = WorkerFleetOrchestrator::new(config.runtime);
//!    let join_handlers = manager.spawn_workers_async();
//!    for (name, ServiceConfig { listener, server }) in config.servers.into_iter() {
//!         let lis_fac = ListenerBuilder::try_from(listener).expect("build listener failed");
//!         let svc_fac = l7_factory(server);
//!         manager
//!             .apply(WorkerDirective::CreateAndDeploy(
//!                    Arc::new(name),
//!                    AsyncMakeServiceWrapper(svc_fac),
//!                    AsyncMakeServiceWrapper(Arc::new(lis_fac)),
//!              ))
//!              .await
//!              .err()
//!              .expect("apply init config failed");
//!    }  
//! ```
//!
//! ## Modules
//!
//! - [`worker_service_lifecycle`]: Core functionality for worker management and service deployment.
//! - [`http`]: HTTP-specific implementations and utilities.
//! - [`thrift`]: Thrift protocol support and related functionalities.
//! - [`config`]: Configuration structures and utilities for the system.
//! - [`context`]: Context management for request processing.
//! - [`listener`]: Network listener implementations and abstractions.
//! - [`util`]: Various utility functions and helpers.
//!
//! ## Error Handling
//!
//! This crate uses [`AnyError`] as a type alias for `anyhow::Error`, providing flexible error
//! handling. The [`AnyResult`] type alias offers a convenient way to return results that can
//! contain any error type.
#[macro_use]
mod error;
pub use error::{AnyError, AnyResult};

pub mod config;
pub mod context;
pub mod http;
pub mod listener;
pub mod thrift;
pub mod util;
pub mod worker_service_lifecycle;

pub(crate) mod sealed {
    #[allow(dead_code)]
    pub trait Sealed {}
    #[allow(dead_code)]
    pub trait SealedT<T1, T2> {}
}
