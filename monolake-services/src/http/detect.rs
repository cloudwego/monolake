//! HTTP version detection and handling module.
//!
//! This module provides functionality to detect the HTTP version (HTTP/1.x or HTTP/2)
//! of incoming connections and route them accordingly. It is designed to work seamlessly
//! with monoio's asynchronous runtime and the service_async framework.
//!
//! # Key Components
//!
//! - [`H2Detect`]: The main service component responsible for HTTP version detection.
//! - [`H2DetectError`]: Error type for version detection operations.
//!
//! # Features
//!
//! - Automatic detection of HTTP/2 connections based on the client preface
//! - Seamless handling of both HTTP/1.x and HTTP/2 connections
//! - Integration with `service_async` for easy composition in service stacks
//! - Efficient I/O handling using monoio's asynchronous primitives
//!
//! # Usage
//!
//! This service is typically used as part of a larger service stack, placed before
//! the main HTTP handling logic. Here's a basic example:
//!
//! ```ignore
//! use service_async::{layer::FactoryLayer, stack::FactoryStack};
//!
//! let config = Config { /* ... */ };
//! let stack = FactoryStack::new(config)
//!     .push(HttpCoreService::layer())
//!     .push(H2Detect::layer())
//!     // ... other layers ...
//!     ;
//!
//! let service = stack.make_async().await.unwrap();
//! // Use the service to handle incoming connections
//! ```
//!
//! # Performance Considerations
//!
//! - Uses efficient buffering to minimize I/O operations during version detection
//! - Implements zero-copy techniques where possible to reduce memory overhead

use service_async::{
    AsyncMakeService, MakeService,
    layer::{FactoryLayer, layer_fn},
};

use crate::common::{DetectService, PrefixDetector};

const PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// Service for detecting HTTP version and routing connections accordingly.
///
/// `H2Detect` examines the initial bytes of an incoming connection to
/// determine whether it's an HTTP/2 connection (by checking for the HTTP/2 preface)
/// or an HTTP/1.x connection. It then forwards the connection to the inner service
/// with appropriate version information.
/// For implementation details and example usage, see the
/// [module level documentation](crate::http::detect).
#[derive(Clone)]
pub struct H2Detect<T> {
    inner: T,
}

#[derive(thiserror::Error, Debug)]
pub enum H2DetectError<E> {
    #[error("inner error: {0:?}")]
    Inner(E),
    #[error("io error: {0:?}")]
    Io(std::io::Error),
}

impl<F: MakeService> MakeService for H2Detect<F> {
    type Service = DetectService<PrefixDetector, F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(DetectService {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
            detector: PrefixDetector(PREFACE),
        })
    }
}

impl<F: AsyncMakeService> AsyncMakeService for H2Detect<F> {
    type Service = DetectService<PrefixDetector, F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(DetectService {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner)).await?,
            detector: PrefixDetector(PREFACE),
        })
    }
}

impl<F> H2Detect<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_: &C, inner| H2Detect { inner })
    }
}
