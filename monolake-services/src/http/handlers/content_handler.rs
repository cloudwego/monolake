//! Content encoding and decoding handler for HTTP requests and responses.
//!
//! This module provides a `ContentHandler` that manages content encoding and decoding
//! for both incoming requests and outgoing responses in an HTTP service stack. It supports
//! various content encodings and can be easily integrated into a service pipeline.
//!
//! # Key Components
//!
//! - [`ContentHandler`]: The main service component responsible for content encoding/decoding.
//!
//! # Features
//!
//! - Transparent content decoding for incoming requests
//! - Content encoding for outgoing responses based on client preferences
//! - Support for various content encodings (e.g., gzip, deflate)
//! - Integration with service-async framework for easy composition
//! - Error handling for decoding and encoding failures
//!
//! # Usage
//!
//! This handler is typically used as part of a larger service stack. Here's a basic example:
//!
//! ```rust
//! use monolake_services::{
//!     common::ContextService,
//!     http::{
//!         core::HttpCoreService,
//!         detect::HttpVersionDetect,
//!         handlers::{
//!             route::RouteConfig, ConnectionReuseHandler, ContentHandler, RewriteAndRouteHandler,
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
//!     .replace(UpstreamHandler::factory(
//!         Default::default(),
//!         Default::default(),
//!     ))
//!     .push(ContentHandler::layer())
//!     .push(RewriteAndRouteHandler::layer())
//!     .push(ConnectionReuseHandler::layer())
//!     .push(HttpCoreService::layer())
//!     .push(HttpVersionDetect::layer());
//!
//! // Use the service to handle HTTP requests
//! ```
//! # Error Handling
//!
//! - Decoding errors for incoming requests result in 400 Bad Request responses
//! - Encoding errors for outgoing responses result in 500 Internal Server Error responses
//!
//! # Performance Considerations
//!
//! - Content encoding/decoding is only performed when necessary (i.e., non-identity encoding)
//! - The handler avoids unnecessary allocations and copies where possible
use std::fmt::Debug;

use http::{Request, StatusCode};
use monoio_http::common::{
    body::{BodyEncodeExt, FixedBody},
    response::Response,
};
use monolake_core::http::{HttpHandler, ResponseWithContinue};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Service,
};

use crate::http::generate_response;

/// Handles content encoding and decoding for HTTP requests and responses.
///
/// `ContentHandler` is responsible for:
/// 1. Decoding the content of incoming requests based on their Content-Encoding header.
/// 2. Encoding the content of outgoing responses based on the client's Accept-Encoding preferences.
///
/// It wraps an inner handler and preprocesses requests before passing them to the inner handler,
/// as well as postprocessing responses from the inner handler. For implementation details and
/// example usage, see the [module level documentation](crate::http::handlers::content_handler).
#[derive(Clone)]
pub struct ContentHandler<H> {
    inner: H,
}

impl<H, CX, B> Service<(Request<B>, CX)> for ContentHandler<H>
where
    H: HttpHandler<CX, B>,
    B: BodyEncodeExt + FixedBody,
    H::Body: BodyEncodeExt + FixedBody,
    B::EncodeDecodeError: Debug,
    <H::Body as BodyEncodeExt>::EncodeDecodeError: Debug,
{
    type Response = ResponseWithContinue<H::Body>;
    type Error = H::Error;

    async fn call(&self, (request, ctx): (Request<B>, CX)) -> Result<Self::Response, Self::Error> {
        let content_encoding = request
            .headers()
            .get(http::header::CONTENT_ENCODING)
            .and_then(|value: &http::HeaderValue| value.to_str().ok())
            .unwrap_or("identity")
            .to_string();

        let accept_encoding = request
            .headers()
            .get(http::header::ACCEPT_ENCODING)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("identity")
            .to_string();

        let content_length = request
            .headers()
            .get(http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.parse::<usize>().unwrap_or_default())
            .unwrap_or_default();

        if content_length == 0 || content_encoding == "identity" {
            let (response, _) = self.inner.handle(request, ctx).await?;
            return Ok((response, true));
        }

        let (parts, body) = request.into_parts();
        match body.decode_content(content_encoding).await {
            Ok(decodec_data) => {
                let req = Request::from_parts(parts, B::fixed_body(Some(decodec_data)));
                let (mut response, _) = self.inner.handle(req, ctx).await?;
                if accept_encoding != "identity" {
                    let (parts, body) = response.into_parts();
                    match body.encode_content(accept_encoding).await {
                        Ok(encoded_data) => {
                            response =
                                Response::from_parts(parts, H::Body::fixed_body(Some(encoded_data)))
                        }
                        Err(e) => {
                            tracing::error!("Response content encoding failed {e:?}");
                            return Ok((
                                generate_response(StatusCode::INTERNAL_SERVER_ERROR, false),
                                true,
                            ));
                        }
                    }
                }
                Ok((response, true))
            }
            Err(e) => {
                tracing::error!("Request content decode failed {e:?}");
                Ok((generate_response(StatusCode::BAD_REQUEST, false), true))
            }
        }
    }
}

// ContentHandler is a Service and a MakeService.
impl<F> MakeService for ContentHandler<F>
where
    F: MakeService,
{
    type Service = ContentHandler<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ContentHandler {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
        })
    }
}

impl<F: AsyncMakeService> AsyncMakeService for ContentHandler<F> {
    type Service = ContentHandler<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(ContentHandler {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner)).await?,
        })
    }
}

impl<F> ContentHandler<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_: &C, inner| Self { inner })
    }
}
