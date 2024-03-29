use std::future::Future;

use http::{Request, Response};
use service_async::Service;

use crate::sealed::SealedT;

// Response and a bool indicating should continue processing the connection.
// Service does not need to add `Connection: close` itself.
pub type ResponseWithContinue<B> = (Response<B>, bool);

// use_h2, io, addr
pub type HttpAccept<Stream, CX> = (bool, Stream, CX);

pub trait HttpHandler<CX, B>: SealedT<(CX, B)> {
    type Body;
    type Error;

    fn handle(
        &self,
        request: Request<B>,
        ctx: CX,
    ) -> impl Future<Output = Result<ResponseWithContinue<Self::Body>, Self::Error>>;
}

impl<T, CX, IB, OB> SealedT<(CX, IB)> for T where
    T: Service<(Request<IB>, CX), Response = ResponseWithContinue<OB>>
{
}

impl<T, CX, IB, OB> HttpHandler<CX, IB> for T
where
    T: Service<(Request<IB>, CX), Response = ResponseWithContinue<OB>>,
{
    type Body = OB;
    type Error = T::Error;

    async fn handle(
        &self,
        req: Request<IB>,
        ctx: CX,
    ) -> Result<ResponseWithContinue<OB>, Self::Error> {
        self.call((req, ctx)).await
    }
}
