use std::{error::Error, future::Future, rc::Rc};

use http::{Request, Response};
use hyper::body::{Body, Incoming};
use hyper_util::server::conn::auto::Builder;
use monoio::io::{
    poll_io::{AsyncRead, AsyncWrite},
    IntoPollIo,
};
use monoio_compat::hyper::{MonoioExecutor, MonoioIo};
use monolake_core::http::HttpHandler;
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Service,
};

use crate::tcp::Accept;

pub struct HyperCoreService<H> {
    handler_chain: Rc<H>,
    builder: Builder<MonoioExecutor>,
}

impl<H> HyperCoreService<H> {
    pub fn new(handler_chain: H) -> Self {
        Self {
            handler_chain: Rc::new(handler_chain),
            builder: Builder::new(MonoioExecutor),
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

pub struct HyperCoreFactory<F> {
    factory_chain: F,
}

impl<F: MakeService> MakeService for HyperCoreFactory<F> {
    type Service = HyperCoreService<F::Service>;
    type Error = F::Error;
    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        let handler_chain = self
            .factory_chain
            .make_via_ref(old.map(|o| o.handler_chain.as_ref()))?;
        Ok(HyperCoreService::new(handler_chain))
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
        Ok(HyperCoreService::new(handler_chain))
    }
}

impl<F> HyperCoreService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = HyperCoreFactory<F>> {
        layer_fn(|_c: &C, inner| HyperCoreFactory {
            factory_chain: inner,
        })
    }
}
