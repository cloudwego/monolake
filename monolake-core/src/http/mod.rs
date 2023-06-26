mod rewrite;
use std::future::Future;

use http::{Request, Response};
use monoio_http::h1::payload::Payload;
pub use rewrite::Rewrite;
use service_async::Service;

use crate::sealed::SealedT;

pub type HttpError = anyhow::Error;

// Response and a bool indicating should continue processing the connection.
// Service does not need to add `Connection: close` itself.
pub type ResponseWithContinue = (Response<Payload>, bool);

// use_h2, io, addr
pub type HttpAccept<Stream, Addr> = (bool, Stream, Addr);

pub trait HttpHandler<CX>: SealedT<CX> {
    type Error;
    type Future<'a>: Future<Output = Result<ResponseWithContinue, Self::Error>>
    where
        Self: 'a,
        CX: 'a;

    fn handle(&self, request: Request<Payload>, ctx: CX) -> Self::Future<'_>;
}

impl<T, CX> SealedT<CX> for T where
    T: Service<(Request<Payload>, CX), Response = ResponseWithContinue>
{
}

impl<T, CX> HttpHandler<CX> for T
where
    T: Service<(Request<Payload>, CX), Response = ResponseWithContinue>,
{
    type Error = T::Error;
    type Future<'a> = impl Future<Output = Result<ResponseWithContinue, Self::Error>> + 'a
    where
        Self: 'a, CX: 'a;

    fn handle(&self, req: Request<Payload>, ctx: CX) -> Self::Future<'_> {
        self.call((req, ctx))
    }
}
