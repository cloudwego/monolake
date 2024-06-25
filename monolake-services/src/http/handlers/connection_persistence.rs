//! HTTP connection persistence and keep-alive management module.
//!
//! This module provides functionality to manage HTTP connection persistence (keep-alive)
//! across different HTTP versions. It handles the intricacies of connection reuse for
//! HTTP/1.0, HTTP/1.1, and HTTP/2, ensuring proper header management and version compatibility.
//! # Key Components
//!
//! - [`ConnectionPersistenceHandler`]: The main service component responsible for managing
//!   connection persistence and keep-alive behavior.
//!
//! # Features
//!
//! - Automatic detection and handling of keep-alive support for incoming requests
//! - Version-specific handling for HTTP/1.0, HTTP/1.1, and HTTP/2
//! - Modification of request and response headers to ensure proper keep-alive behavior
//! - Seamless integration with `service_async` for easy composition in service stacks
//! - Support for upgrading HTTP/1.0 connections to HTTP/1.1-like behavior
//!
//! # Usage
//!
//! This handler is typically used as part of a larger HTTP service stack. Here's a basic example:
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
//!
//! # Performance Considerations
//!
//! - Efficient header manipulation to minimize overhead
//! - Optimized handling for HTTP/2, which has built-in connection persistence
use http::{Request, Version};
use monolake_core::http::{HttpHandler, ResponseWithContinue};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Service,
};
use tracing::debug;

use crate::http::{CLOSE, CLOSE_VALUE, KEEPALIVE, KEEPALIVE_VALUE};

/// Handler for managing HTTP connection persistence and keep-alive behavior.
///
/// `ConnectionPersistenceHandler` is responsible for:
/// 1. Detecting whether an incoming request supports keep-alive.
/// 2. Modifying request and response headers to ensure proper keep-alive behavior.
/// 3. Handling version-specific connection persistence logic for HTTP/1.0, HTTP/1.1, and HTTP/2.
///
/// For implementation details and example usage, see the
/// [module level documentation](crate::http::handlers::connection_persistence).
#[derive(Clone)]
pub struct ConnectionPersistenceHandler<H> {
    inner: H,
}

impl<H, CX, B> Service<(Request<B>, CX)> for ConnectionPersistenceHandler<H>
where
    H: HttpHandler<CX, B>,
{
    type Response = ResponseWithContinue<H::Body>;
    type Error = H::Error;

    async fn call(
        &self,
        (mut request, ctx): (Request<B>, CX),
    ) -> Result<Self::Response, Self::Error> {
        let version = request.version();
        let keepalive = is_conn_keepalive(request.headers(), version);
        debug!("frontend keepalive {:?}", keepalive);

        match version {
            // for http 1.0, hack it to 1.1 like setting nginx `proxy_http_version` to 1.1
            Version::HTTP_10 => {
                // modify to 1.1 and remove connection header
                *request.version_mut() = Version::HTTP_11;
                let _ = request.headers_mut().remove(http::header::CONNECTION);

                // send
                let (mut response, mut cont) = self.inner.handle(request, ctx).await?;
                cont &= keepalive;

                // modify back and make sure reply keepalive if client want it and server
                // support it.
                let _ = response.headers_mut().remove(http::header::CONNECTION);
                if cont {
                    // insert keepalive header
                    response
                        .headers_mut()
                        .insert(http::header::CONNECTION, KEEPALIVE_VALUE);
                }
                *response.version_mut() = version;

                Ok((response, cont))
            }
            Version::HTTP_11 => {
                // remove connection header
                let _ = request.headers_mut().remove(http::header::CONNECTION);

                // send
                let (mut response, mut cont) = self.inner.handle(request, ctx).await?;
                cont &= keepalive;

                // modify back and make sure reply keepalive if client want it and server
                // support it.
                let _ = response.headers_mut().remove(http::header::CONNECTION);
                if !cont {
                    // insert close header
                    response
                        .headers_mut()
                        .insert(http::header::CONNECTION, CLOSE_VALUE);
                }
                Ok((response, cont))
            }
            Version::HTTP_2 => {
                let (response, _) = self.inner.handle(request, ctx).await?;
                Ok((response, true))
            }
            // for http 0.9 and other versions, just relay it
            _ => {
                let (response, _) = self.inner.handle(request, ctx).await?;
                Ok((response, false))
            }
        }
    }
}

// ConnReuseHandler is a Service and a MakeService.
impl<F: MakeService> MakeService for ConnectionPersistenceHandler<F> {
    type Service = ConnectionPersistenceHandler<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ConnectionPersistenceHandler {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
        })
    }
}

impl<F: AsyncMakeService> AsyncMakeService for ConnectionPersistenceHandler<F> {
    type Service = ConnectionPersistenceHandler<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(ConnectionPersistenceHandler {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner)).await?,
        })
    }
}

impl<F> ConnectionPersistenceHandler<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_: &C, inner| Self { inner })
    }
}

fn is_conn_keepalive(headers: &http::HeaderMap<http::HeaderValue>, version: Version) -> bool {
    match (version, headers.get(http::header::CONNECTION)) {
        (Version::HTTP_10, Some(header))
            if header.as_bytes().eq_ignore_ascii_case(KEEPALIVE.as_bytes()) =>
        {
            true
        }
        (Version::HTTP_11, None) => true,
        (Version::HTTP_11, Some(header))
            if !header.as_bytes().eq_ignore_ascii_case(CLOSE.as_bytes()) =>
        {
            true
        }
        _ => false,
    }
}
