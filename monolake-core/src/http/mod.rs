// pub mod handler;
mod rewrite;
use std::future::Future;

use http::{Request, Response};
use monoio_http::h1::payload::Payload;
pub use rewrite::Rewrite;

use crate::{sealed::Sealed, service::Service};

pub type HttpError = anyhow::Error;

pub trait HttpHandler: Sealed {
    type Error;
    type Future<'a>: Future<Output = Result<Response<Payload>, Self::Error>>
    where
        Self: 'a;

    fn handle(&self, request: Request<Payload>) -> Self::Future<'_>;
}

impl<T: Service<Request<Payload>, Response = Response<Payload>>> Sealed for T {}

impl<T: Service<Request<Payload>, Response = Response<Payload>>> HttpHandler for T {
    type Error = T::Error;
    type Future<'a> = impl Future<Output = Result<Response<Payload>, Self::Error>> + 'a
    where
        Self: 'a;

    fn handle(&self, req: Request<Payload>) -> Self::Future<'_> {
        async move { self.call(req).await }
    }
}
