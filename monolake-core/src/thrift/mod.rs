use std::future::Future;

use monoio_thrift::codec::ttheader::TTHeaderPayload;
use service_async::Service;

// TODO: support uncontiguous memory
pub type ThriftBody = bytes::Bytes;
pub type ThriftRequest<T> = TTHeaderPayload<T>;
pub type ThriftResponse<T> = TTHeaderPayload<T>;

pub trait ThriftHandler<CX> {
    type Error;

    fn handle(
        &self,
        request: ThriftRequest<ThriftBody>,
        ctx: CX,
    ) -> impl Future<Output = Result<ThriftResponse<ThriftBody>, Self::Error>>;
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
