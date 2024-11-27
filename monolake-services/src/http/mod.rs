//! HTTP protocol handling and services module.
//!
//! This module provides a comprehensive set of components for handling HTTP connections,
//! processing requests, and managing responses. It includes core services, various handlers,
//! protocol detection, and utility functions for working with HTTP.
//!
//! # Key Components
//!
//! ## Submodules
//!
//! - [`core`]: Contains the core HTTP service implementation, including `HttpCoreService`.
//! - [`handlers`]: Provides various HTTP request handlers for different aspects of request
//!   processing.
//! - [`detect`]: Implements HTTP version detection functionality.
//!
//! ## Structs and Types
//!
//! - [`HttpCoreService`]: The main service for handling HTTP/1.1 and HTTP/2 connections.
//! - [`HttpServerTimeout`]: Configuration for various HTTP server timeout settings.
//!
//! # Features
//!
//! - Support for both HTTP/1.1 and HTTP/2 protocols
//! - Modular design with separate handlers for different aspects of HTTP processing
//! - HTTP version detection capabilities
//! - Configurable timeout settings for various stages of request handling
//! - Utility functions and constants for common HTTP operations
//!
//! # Performance Considerations
//!
//! - The core service and handlers are designed for efficient processing of HTTP requests
//! - Connection keep-alive and HTTP/2 multiplexing are supported for improved performance
//! - Version detection allows for optimized handling based on the HTTP version
//!
//! # Error Handling
//!
//! - Each component implements its own error handling strategy
//! - The core service provides high-level error handling for the entire request lifecycle
//!
//! # Customization
//!
//! - The modular design allows for easy extension and customization of HTTP handling behavior
//! - Custom handlers can be implemented and integrated into the `HttpCoreService`
use http::HeaderValue;
use serde::{Deserialize, Serialize};

pub use self::core::{HttpCoreService, HttpServerTimeout};
pub mod handlers;

pub mod core;
pub mod detect;
pub mod util;

pub(crate) const CLOSE: &str = "close";
pub(crate) const KEEPALIVE: &str = "Keep-Alive";
#[allow(clippy::declare_interior_mutable_const)]
pub(crate) const CLOSE_VALUE: HeaderValue = HeaderValue::from_static(CLOSE);
#[allow(clippy::declare_interior_mutable_const)]
pub(crate) const KEEPALIVE_VALUE: HeaderValue = HeaderValue::from_static(KEEPALIVE);
pub(crate) use util::generate_response;

#[derive(Debug, Copy, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HttpVersion {
    Http2,
    Http11,
    #[default]
    Auto,
}
