mod rewrite;
use std::{future::Future, io::Cursor};

use http::{Request, Response};
use monoio::io::PrefixedReadIo;
use monoio_http::h1::payload::Payload;
pub use rewrite::Rewrite;
use service_async::Service;

use crate::{environments::Environments, sealed::Sealed};

pub type HttpError = anyhow::Error;

// Response and a bool indicating should continue processing the connection.
// Service does not need to add `Connection: close` itself.
pub type ResponseWithContinue = (Response<Payload>, bool);

pub type HttpAccept<Stream, SocketAddr> =
    (bool, PrefixedReadIo<Stream, Cursor<Vec<u8>>>, SocketAddr);

pub trait HttpHandler: Sealed {
    type Error;
    type Future<'a>: Future<Output = Result<ResponseWithContinue, Self::Error>>
    where
        Self: 'a;

    fn handle(&self, request: Request<Payload>, environments: Environments) -> Self::Future<'_>;
}

impl<T: Service<(Request<Payload>, Environments), Response = ResponseWithContinue>> Sealed for T {}

impl<T: Service<(Request<Payload>, Environments), Response = ResponseWithContinue>> HttpHandler
    for T
{
    type Error = T::Error;
    type Future<'a> = impl Future<Output = Result<ResponseWithContinue, Self::Error>> + 'a
    where
        Self: 'a;

    fn handle(&self, req: Request<Payload>, environments: Environments) -> Self::Future<'_> {
        async move { self.call((req, environments)).await }
    }
}
