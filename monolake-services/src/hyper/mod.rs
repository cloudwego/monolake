//! Hyper-based HTTP core service for handling client connections.
//!
//! This module provides a high-performance, asynchronous HTTP service built on top of
//! the Hyper library. It's designed to work with monoio's asynchronous runtime and
//! supports flexible handler composition through the `HttpHandler` trait.
//!
//! # Key Components
//!
//! - [`HyperCoreService`](HyperCoreService): The main service component responsible for handling
//!   HTTP connections using Hyper. It can be composed of handlers implementing the `HttpHandler`
//!   trait.
//! - [`HyperCoreFactory`](HyperCoreFactory): Factory for creating `HyperCoreService` instances.
//! - [`HyperCoreError`](HyperCoreError): Error type for `HyperCoreService` operations.
//!
//! # Features
//!
//! - Built on Hyper for robust HTTP protocol support
//! - Integration with monoio's asynchronous runtime
//! - Composable design allowing a stack of `HttpHandler` implementations
//! - Configurable through Hyper's `Builder`
//!
//! # Usage
//!
//! `HyperCoreService` is typically used as part of a larger service stack. Here's a basic example:
//!
//! ```ignore
//! use service_async::{layer::FactoryLayer, stack::FactoryStack};
//!
//! use crate::http::HyperCoreService;
//!
//! let config = Config { /* ... */ };
//! let stack = FactoryStack::new(config)
//!     .push(HyperCoreService::layer())
//!     // ... other handlers implementing HttpHandler ...
//!     ;
//!
//! let service = stack.make_async().await.unwrap();
//! // Use the service to handle incoming HTTP connections
//! ```
//!
//! # Handler Composition
//!
//! `HyperCoreService` can be composed of multiple handlers implementing the `HttpHandler` trait.
//! This allows for a flexible and modular approach to request processing. Handlers can be
//! chained together to form a processing pipeline, each handling a specific aspect of the
//! HTTP request/response cycle.
//!
//! # Performance Considerations
//!
//! - Leverages Hyper's efficient HTTP implementation
//! - Uses monoio's async I/O operations for improved performance
//! - Supports connection keep-alive and pipelining through Hyper
use std::{error::Error, future::Future, rc::Rc};

use http::{Request, Response};
use hyper::body::{Body, Incoming};
use hyper_util::server::conn::auto::Builder;
use monoio::io::{
    IntoPollIo,
    poll_io::{AsyncRead, AsyncWrite},
};
pub use monoio_compat::hyper::{MonoioExecutor, MonoioIo};
use monolake_core::http::HttpHandler;
use service_async::{
    AsyncMakeService, MakeService, Service,
    layer::{FactoryLayer, layer_fn},
};

use crate::tcp::Accept;

pub struct HyperCoreService<H> {
    handler_chain: Rc<H>,
    builder: Builder<MonoioExecutor>,
}

/// Hyper-based HTTP core service supporting handler composition.
///
/// `HyperCoreService` is responsible for handling HTTP connections using Hyper,
/// and can be composed of a chain of handlers implementing the `HttpHandler` trait.
impl<H> HyperCoreService<H> {
    #[inline]
    pub fn new(handler_chain: H, builder: Builder<MonoioExecutor>) -> Self {
        Self {
            handler_chain: Rc::new(handler_chain),
            builder,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum HyperCoreError {
    #[error("io error: {0:?}")]
    Io(#[from] std::io::Error),
    #[error("hyper error: {0:?}")]
    Hyper(#[from] Box<dyn Error + Send + Sync>),
}

impl<H, Stream, CX> Service<Accept<Stream, CX>> for HyperCoreService<H>
where
    Stream: IntoPollIo,
    Stream::PollIo: AsyncRead + AsyncWrite + Unpin + 'static,
    H: HttpHandler<CX, Incoming> + 'static,
    H::Error: Into<Box<dyn Error + Send + Sync>>,
    H::Body: Body,
    <H::Body as Body>::Error: Into<Box<dyn Error + Send + Sync>>,
    CX: Clone + 'static,
{
    type Response = ();
    type Error = HyperCoreError;

    async fn call(&self, (io, cx): Accept<Stream, CX>) -> Result<Self::Response, Self::Error> {
        tracing::trace!("hyper core handling io");
        let poll_io = io.into_poll_io()?;
        let io = MonoioIo::new(poll_io);
        let service = HyperServiceWrapper {
            cx,
            handler_chain: self.handler_chain.clone(),
        };
        self.builder
            .serve_connection(io, service)
            .await
            .map_err(Into::into)
    }
}

struct HyperServiceWrapper<CX, H> {
    cx: CX,
    handler_chain: Rc<H>,
}

impl<CX, H> hyper::service::Service<Request<Incoming>> for HyperServiceWrapper<CX, H>
where
    H: HttpHandler<CX, Incoming> + 'static,
    CX: Clone + 'static,
{
    type Response = Response<H::Body>;
    type Error = H::Error;
    type Future = impl Future<Output = Result<Self::Response, Self::Error>> + 'static;

    #[inline]
    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let chain = self.handler_chain.clone();
        let cx = self.cx.clone();
        async move { chain.handle(req, cx).await.map(|r| r.0) }
    }
}

/// Factory for creating `HyperCoreService` instances.
pub struct HyperCoreFactory<F> {
    factory_chain: F,
    builder: Builder<MonoioExecutor>,
}

impl<F: MakeService> MakeService for HyperCoreFactory<F> {
    type Service = HyperCoreService<F::Service>;
    type Error = F::Error;
    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        let handler_chain = self
            .factory_chain
            .make_via_ref(old.map(|o| o.handler_chain.as_ref()))?;
        Ok(HyperCoreService::new(handler_chain, self.builder.clone()))
    }
}

impl<F: AsyncMakeService> AsyncMakeService for HyperCoreFactory<F> {
    type Service = HyperCoreService<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        let handler_chain = self
            .factory_chain
            .make_via_ref(old.map(|o| o.handler_chain.as_ref()))
            .await?;
        Ok(HyperCoreService::new(handler_chain, self.builder.clone()))
    }
}

impl<F> HyperCoreService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = HyperCoreFactory<F>> {
        layer_fn(|_c: &C, inner| HyperCoreFactory {
            factory_chain: inner,
            builder: Builder::new(MonoioExecutor),
        })
    }

    pub fn layer_with_builder<C>(
        builder: Builder<MonoioExecutor>,
    ) -> impl FactoryLayer<C, F, Factory = HyperCoreFactory<F>> {
        layer_fn(move |_c: &C, inner| HyperCoreFactory {
            factory_chain: inner,
            builder: builder.clone(),
        })
    }

    #[inline]
    pub fn builder(&mut self) -> &mut Builder<MonoioExecutor> {
        &mut self.builder
    }
}
