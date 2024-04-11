use std::future::Future;

use monoio_thrift::codec::ttheader::TTHeaderPayload;
use service_async::Service;

use crate::sealed::SealedT;

// TODO: support discontinuous memory
pub type ThriftBody = bytes::Bytes;
pub type ThriftRequest<T> = TTHeaderPayload<T>;
pub type ThriftResponse<T> = TTHeaderPayload<T>;

struct ThriftSeal;

#[allow(private_bounds)]
pub trait ThriftHandler<CX>: SealedT<ThriftSeal, CX> {
    type Error;

    fn handle(
        &self,
        request: ThriftRequest<ThriftBody>,
        ctx: CX,
    ) -> impl Future<Output = Result<ThriftResponse<ThriftBody>, Self::Error>>;
}

impl<T, CX> SealedT<ThriftSeal, CX> for T where
    T: Service<(ThriftRequest<ThriftBody>, CX), Response = ThriftResponse<ThriftBody>>
{
}

impl<T, CX> ThriftHandler<CX> for T
where
    T: Service<(ThriftRequest<ThriftBody>, CX), Response = ThriftResponse<ThriftBody>>,
{
    type Error = T::Error;

    async fn handle(
        &self,
        req: ThriftRequest<ThriftBody>,
        ctx: CX,
    ) -> Result<ThriftResponse<ThriftBody>, Self::Error> {
        self.call((req, ctx)).await
    }
}
