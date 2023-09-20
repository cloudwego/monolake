use bytes::Bytes;
pub use dyn_thrift::thrift::service::H2TConfig;
use dyn_thrift::thrift::service::{H2TFactory, H2TService};
use futures::Future;
use http::Request;
use matchit::Router;
use monoio_http::common::body::{BodyExt, HttpBody};
use monolake_core::{
    http::{HttpHandler, ResponseWithContinue},
    AnyError,
};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};

#[derive(Clone)]
pub struct H2THandler<H> {
    h2t: H2TService,
    inner: H,
}

impl<H, CX> Service<(Request<HttpBody>, CX)> for H2THandler<H>
where
    H: HttpHandler<CX>,
    H::Error: Into<AnyError>,
{
    type Response = ResponseWithContinue;
    type Error = AnyError;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a, Request<HttpBody>: 'a, CX: 'a;

    fn call(&self, (request, ctx): (Request<HttpBody>, CX)) -> Self::Future<'_> {
        async move {
            if request.headers().get("h2t").is_some() {
                // simulate route match
                let mut router = Box::new(Router::new());
                router.insert("/:path", "test path").unwrap();
                let params = router.at("/path").unwrap().params;

                let (header, body) = request.into_parts();
                let body: Bytes = body.bytes().await?;
                let req = http::Request::from_parts(header, body);
                let resp = self.h2t.call((&req, params, "GetItem")).await?;
                let (header, body) = resp.into_parts();
                let resp = http::Response::from_parts(header, HttpBody::Ready(Some(body)));
                return Ok((resp, true));
            }
            self.inner.handle(request, ctx).await.map_err(Into::into)
        }
    }
}

#[derive(Clone)]
pub struct H2THandlerFactory<F> {
    fac: H2TFactory,
    inner: F,
}

impl<F> MakeService for H2THandlerFactory<F>
where
    F: MakeService,
    F::Error: Into<AnyError>,
{
    type Service = H2THandler<F::Service>;
    type Error = AnyError;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(H2THandler {
            h2t: self.fac.make()?,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
        })
    }
}

impl<F> H2THandler<F> {
    pub fn layer<C: Param<H2TConfig>>() -> impl FactoryLayer<C, F, Factory = H2THandlerFactory<F>> {
        layer_fn(|cfg: &C, inner| H2THandlerFactory {
            fac: H2TService::layer().layer(cfg, ()),
            inner,
        })
    }
}
