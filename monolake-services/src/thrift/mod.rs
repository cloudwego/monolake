//! Thrift protocol support and services for building high-performance Thrift servers and proxies.
//!
//! This module provides components for handling Thrift protocol communications, including
//! core services for processing Thrift requests and proxy handlers for routing requests
//! to upstream Thrift servers. It is designed to work with monoio's asynchronous runtime
//! and the `service_async` framework for efficient and composable service architectures.
//!
//! # Submodules
//!
//! - [`handlers`]: Contains handlers for processing Thrift requests, including proxy functionality.
//! - [`ttheader`]: Implements core services for the Thrift THeader protocol.
//!
//! # Key Components
//!
//! - [`TtheaderCoreService`](ttheader::TtheaderCoreService): Core service for handling Thrift
//!   THeader protocol connections.
//! - [`ProxyHandler`](handlers::ProxyHandler): Proxy service for routing Thrift requests to
//!   upstream servers.
//!
//! # Features
//!
//! - Support for Thrift THeader protocol
//! - High-performance request processing and proxying
//! - Configurable timeout settings for various stages of request handling
//! - Connection pooling for efficient management of upstream connections
//! - Integration with `service_async` for easy composition in service stacks
//!
//! # Usage
//!
//! Components from this module can be used to build Thrift servers or proxies. Here's a basic
//! example:
//!
//! ```ignore
//! use service_async::{layer::FactoryLayer, stack::FactoryStack};
//!
//! use crate::thrift::{handlers::ProxyHandler, TtheaderCoreService};
//!
//! let config = Config { /* ... */ };
//! let routes = vec![RouteConfig { /* ... */ }];
//!
//! let stack = FactoryStack::new(config)
//!     .push(ProxyHandler::factory(routes))
//!     .push(TtheaderCoreService::layer());
//!
//! let service = stack.make_async().await.unwrap();
//! // Use the service to handle incoming Thrift connections
//! ```
//!
//! # Performance Considerations
//!
//! - Utilizes monoio's efficient async I/O operations
//! - Implements connection pooling to reduce connection establishment overhead
//! - Optimized for the Thrift THeader protocol
//!
//! For more detailed information on specific components, please refer to the documentation
//! of individual submodules and structs.
pub mod handlers;
pub mod ttheader;

pub use handlers::proxy::{Endpoint, RouteConfig, Upstream};
