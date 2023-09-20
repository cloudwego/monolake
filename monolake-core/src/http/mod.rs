use std::future::Future;

use http::{Request, Response};
use monoio_http::common::body::HttpBody;
use service_async::Service;

use crate::sealed::SealedT;

// Response and a bool indicating should continue processing the connection.
// Service does not need to add `Connection: close` itself.
pub type ResponseWithContinue = (Response<HttpBody>, bool);

// use_h2, io, addr
pub type HttpAccept<Stream, CX> = (bool, Stream, CX);

pub trait HttpHandler<CX>: SealedT<CX> {
    type Error;
    type Future<'a>: Future<Output = Result<ResponseWithContinue, Self::Error>>
    where
        Self: 'a,
        CX: 'a;

    fn handle(&self, request: Request<HttpBody>, ctx: CX) -> Self::Future<'_>;
}

impl<T, CX> SealedT<CX> for T where
    T: Service<(Request<HttpBody>, CX), Response = ResponseWithContinue>
{
}

impl<T, CX> HttpHandler<CX> for T
where
    T: Service<(Request<HttpBody>, CX), Response = ResponseWithContinue>,
{
    type Error = T::Error;
    type Future<'a> = impl Future<Output = Result<ResponseWithContinue, Self::Error>> + 'a
    where
        Self: 'a, CX: 'a;

    fn handle(&self, req: Request<HttpBody>, ctx: CX) -> Self::Future<'_> {
        self.call((req, ctx))
    }
}
