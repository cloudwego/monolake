#![feature(let_chains)]
#![feature(impl_trait_in_assoc_type)]
//! # Monolake Services
//!
//! `monolake-services` is a crate that provides a collection of services
//! for building high-performance, modular HTTP servers and Thrift services. It offers a range of
//! components that can be easily combined with custom user-created services to create robust and
//! flexible server applications.
//!
//! ## Key Components
//!
//! ### HTTP Services
//!
//! #### Connection Handlers
//!
//! - [`HttpCoreService`](http::core): The main service for handling HTTP/1.1 and HTTP/2
//!   connections.
//! - [`HttpVersionDetect`](http::detect): Automatic detection of HTTP protocol versions.
//!   #[cfg_attr(feature = "hyper", doc = "- [`HyperCoreService`](hyper::HyperCoreService): A
//!   high-performance HTTP service built on top of the Hyper library.")]
//!
//! #### Request Handlers
//!
//! - [`ConnectionReuseHandler`](http::handlers::connection_persistence): Manages HTTP connection
//!   persistence and keep-alive behavior. It ensures proper handling of connection lifecycles
//!   across different HTTP versions.
//!
//! - [`ContentHandler`](http::handlers::content_handler): Handles content encoding and decoding for
//!   requests and responses. It supports various compression methods and ensures efficient data
//!   transfer.
//!
//! - [`RewriteAndRouteHandler`](http::handlers::route): Directs requests to appropriate handlers
//!   based on predefined rules. It allows for flexible URL-based routing and request dispatching.
//!
//! - [`UpstreamHandler`](http::handlers::upstream): Manages proxying of requests to upstream
//!   servers. It supports load balancing, connection pooling, and error handling for backend
//!   services.
//!
//! - [`OpenIdHandler`](crate::http::handlers::OpenIdHandler): Provides OpenID Connect
//!   authentication (optional feature). It enables secure user authentication using OpenID Connect
//!   protocols.
//!
//! ### Thrift Services
//!
//! - [`TtheaderCoreService`](thrift::ttheader): Core service for handling Thrift THeader protocol
//!   connections.
//! - [`ProxyHandler`](thrift::handlers::proxy): Proxy service for routing Thrift requests to
//!   upstream servers.
//!
//! The Thrift module provides components for handling Thrift protocol communications, including
//! core services for processing Thrift requests and proxy handlers for routing requests
//! to upstream Thrift servers. It supports the THeader protocol, connection pooling, and
//! integrates seamlessly with the `service_async` framework.
//!
//! ### Common Services
//!
//! - [`CatchPanicService`](common::CatchPanicService): Catches panics in inner services and
//!   converts them to errors. It enhances system stability by preventing panics from crashing the
//!   entire server.
//!
//! - [`ContextService`](common::ContextService): Inserts context information into the request
//!   processing pipeline. It works with `certain_map` for flexible and type-safe context
//!   management.
//!
//! - [`TimeoutService`](common::TimeoutService) Adds configurable timeout functionality to any
//!   inner service. It ensures that long-running operations don't block the server indefinitely.
//!
//! ### TLS Service
//!
//! - [`UnifiedTlsService`](crate::tls): Provides a unified interface for different TLS
//!   implementations (Rustls and Native TLS). It allows for flexible TLS configuration and seamless
//!   switching between TLS backends.
//!
//! ### Proxy Protocol Service
//!
//! - [`ProxyProtocolService`](crate::proxy_protocol::ProxyProtocolService): Handles PROXY protocol
//!   headers in incoming connections. It preserves client IP information when operating behind load
//!   balancers or proxies.
//!
//! ## Service Trait
//!
//! All services in this crate implement the `Service` trait, which is defined as follows:
//!
//! ```ignore
//! pub trait Service<Request> {
//!     type Response;
//!     type Error;
//!
//!     fn call(&self, req: Request) -> impl Future<Output = Result<Self::Response, Self::Error>>;
//! }
//! ```
//!
//! This trait allows for efficient and flexible composition of services, enabling
//! the creation of complex processing pipelines.
//!
//! ## Features
//!
//! - Modular design allowing easy composition of services
//! - Support for HTTP/1.x and HTTP/2 protocols
//! - Support for Thrift THeader protocol
//! - Flexible routing and request processing capabilities
//! - TLS support with multiple backend options
//! - PROXY protocol support for preserving client IP information
//!
//! ## Usage Example
//!
//! Here's a basic example of how to compose these services:
//!
//! ```ignore
//! use monolake_services::{
//!     HttpCoreService, HttpVersionDetect, ConnectionReuseHandler,
//!     ContentHandler, RewriteAndRouteHandler, UpstreamHandler, UnifiedTlsService,
//!     ProxyProtocolService, HyperCoreService
//! };
//! use service_async::{layer::FactoryLayer, stack::FactoryStack};
//!
//! let config = ServerConfig {
//!     // ... configuration options ...
//! };
//!
//! let stack = FactoryStack::new(config)
//!     .push(UpstreamHandler::layer())
//!     .push(RewriteAndRouteHandler::layer())
//!     .push(ContentHandler::layer())
//!     .push(ConnectionReuseHandler::layer())
//!     .push(HyperCoreService::layer());
//!     .push(HttpVersionDetect::layer())
//!     .push(UnifiedTlsService::layer())
//!     .push(ContextService::layer());
//!
//!
//! let service = stack.make_async().await.unwrap();
//! // Use the service to handle incoming connections
//! ```
//!
//! ## Performance Considerations
//!
//! - Efficient async I/O operations using the `monoio` runtime
//! - Connection pooling and keep-alive support for improved resource utilization
//! - Optimized routing and request handling
//! - Support for HTTP/2 multiplexing
//! - Efficient Thrift request processing and proxying
//!
//! ## Customization
//!
//! The modular nature of the services allows for easy extension and customization.
//! Users can implement their own services that conform to the `Service` trait
//! and integrate them seamlessly into the processing pipeline.
//!
//! ## Additional Resources
//!
//! For more detailed information on each component, please refer to the documentation
//! of individual modules and the examples directory in the crate's repository.
pub mod common;
pub mod http;
pub mod tcp;
pub mod thrift;

#[cfg(feature = "proxy-protocol")]
pub mod proxy_protocol;

#[cfg(feature = "tls")]
pub mod tls;

#[cfg(feature = "hyper")]
pub mod hyper;
