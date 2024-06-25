//! HTTP request handling and processing module.
//!
//! This module provides a collection of handlers and services that work together
//! to process HTTP requests in a modular and efficient manner. Each handler implements
//! the `HttpHandler` trait, allowing for flexible composition and nesting within the
//! [`HttpCoreService`](crate::http::HttpCoreService).
//!
//! # Key Components
//!
//! - [`HttpCoreService`](crate::http::HttpCoreService): Core service responsible for handling
//!   HTTP/1.1 and HTTP/2 connections, decoding requests, and encoding responses.
//! - [`ConnectionPersistenceHandler`]: Manages HTTP connection persistence and keep-alive behavior
//!   across different HTTP versions.
//! - [`ContentHandler`]: Handles content encoding and decoding for both requests and responses.
//! - [`UpstreamHandler`]: Manages proxying of requests to upstream servers, including load
//!   balancing and error handling.
//! - [`RoutingHandler`]: Handles request routing based on predefined rules, directing requests to
//!   appropriate handlers or upstream servers.
//!
//! # Optional Components
//!
//! - [`OpenIdHandler`]: Provides OpenID Connect authentication functionality (available with the
//!   "openid" feature).
//!
//! # HttpHandler Trait
//!
//! All handlers in this module implement the `HttpHandler` trait, which defines a common
//! interface for processing HTTP requests:
//!
//! ```ignore
//! pub trait HttpHandler<CX, B>: SealedT<HttpSeal, (CX, B)> {
//!     type Body;
//!     type Error;
//!     fn handle(
//!         &self,
//!         request: Request<B>,
//!         ctx: CX,
//!     ) -> impl Future<Output = Result<ResponseWithContinue<Self::Body>, Self::Error>>;
//! }
//! ```
//!
//! This trait allows handlers to be easily composed and nested within the `HttpCoreService`.
//!
//! # Features
//!
//! - Modular design allowing easy composition of handlers in a service stack
//! - Support for HTTP/1.1 and HTTP/2 protocols through `HttpCoreService`
//! - Efficient connection management and keep-alive handling
//! - Content encoding and decoding support
//! - Flexible routing capabilities
//! - Upstream proxying with load balancing
//! - Optional OpenID Connect authentication
//!
//! # Usage
//!
//! Handlers in this module can be composed and nested within the `HttpCoreService`.
//! Here's a basic example:
//!
//! ```rust
//! use monolake_services::{
//!     common::ContextService,
//!     http::{
//!         core::HttpCoreService,
//!         detect::HttpVersionDetect,
//!         handlers::{
//!             route::RouteConfig, ConnectionPersistenceHandler, ContentHandler, RoutingHandler,
//!             UpstreamHandler,
//!         },
//!         HttpServerTimeout,
//!     },
//! };
//! use service_async::{layer::FactoryLayer, stack::FactoryStack, Param};
//!
//! // Dummy struct to satisfy Param trait requirements
//! struct DummyConfig;
//!
//! // Implement Param for DummyConfig to return Vec<RouteConfig>
//! impl Param<Vec<RouteConfig>> for DummyConfig {
//!     fn param(&self) -> Vec<RouteConfig> {
//!         vec![]
//!     }
//! }
//! impl Param<HttpServerTimeout> for DummyConfig {
//!     fn param(&self) -> HttpServerTimeout {
//!         HttpServerTimeout::default()
//!     }
//! }
//!
//! let config = DummyConfig;
//! let stacks = FactoryStack::new(config)
//!     .replace(UpstreamHandler::factory(Default::default()))
//!     .push(ContentHandler::layer())
//!     .push(RoutingHandler::layer())
//!     .push(ConnectionPersistenceHandler::layer())
//!     .push(HttpCoreService::layer())
//!     .push(HttpVersionDetect::layer());
//!
//! // Use the service to handle HTTP requests
//! ```
//! # Performance Considerations
//!
//! - Each handler is designed to be efficient and add minimal overhead
//! - The modular design allows for fine-grained control over request processing, enabling
//!   optimizations based on specific use cases
//! - `HttpCoreService` efficiently handles both HTTP/1.1 and HTTP/2 protocols
//!
//! # Error Handling
//!
//! - Each handler implements its own error handling strategy
//! - The `RoutingFactoryError` type is exposed for handling routing-specific errors
//! - `HttpCoreService` provides high-level error handling for the entire request lifecycle
//!
//! # Feature Flags
//!
//! - `openid`: Enables the OpenID Connect authentication functionality
pub mod connection_persistence;
pub mod content_handler;
#[cfg(feature = "openid")]
pub mod openid;
pub mod route;
pub mod upstream;

pub use connection_persistence::ConnectionPersistenceHandler;
pub use content_handler::ContentHandler;
#[cfg(feature = "openid")]
pub use openid::OpenIdHandler;
pub use route::{RoutingFactoryError, RoutingHandler};
pub use upstream::UpstreamHandler;
