use std::{future::Future, rc::Rc};

use http::{Request, Response};
use hyper::body::{Body, Incoming};
use hyper_util::server::conn::auto::Builder;
use monoio::io::{
    poll_io::{AsyncRead, AsyncWrite},
    IntoPollIo,
};
use monoio_compat::hyper::{MonoioExecutor, MonoioIo};
use monolake_core::{http::HttpHandler, AnyError};
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

impl<H, Stream, CX> Service<Accept<Stream, CX>> for HyperCoreService<H>
where
    Stream: IntoPollIo,
    Stream::PollIo: AsyncRead + AsyncWrite + Unpin + 'static,
    H: HttpHandler<CX, Incoming> + 'static,
    H::Error: Into<AnyError>,
    H::Body: Body,
    <H::Body as Body>::Error: Into<AnyError>,
    CX: Clone + 'static,
{
    type Response = ();
    type Error = AnyError;

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

impl<F: MakeService> MakeService for HyperCoreService<F> {
    type Service = HyperCoreService<F::Service>;
    type Error = F::Error;
    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        let handler_chain = self
            .handler_chain
            .make_via_ref(old.map(|o| o.handler_chain.as_ref()))?;
        Ok(HyperCoreService::new(handler_chain))
    }
}

impl<F: AsyncMakeService> AsyncMakeService for HyperCoreService<F> {
    type Service = HyperCoreService<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        let handler_chain = self
            .handler_chain
            .make_via_ref(old.map(|o| o.handler_chain.as_ref()))
            .await?;
        Ok(HyperCoreService::new(handler_chain))
    }
}

impl<F> HyperCoreService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_c: &C, inner| Self::new(inner))
    }
}
