use std::{convert::Infallible, future::Future, io};

use monoio::io::{AsyncReadRent, AsyncWriteRent, AsyncWriteRentExt};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};

#[derive(Debug, Clone)]
pub struct EchoReplaceConfig {
    pub replace: u8,
}

pub struct EchoReplaceService {
    replace: u8,
}

impl<S> Service<S> for EchoReplaceService
where
    S: AsyncReadRent + AsyncWriteRent,
{
    type Response = ();

    type Error = io::Error;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        S: 'cx;

    fn call(&self, mut io: S) -> Self::Future<'_> {
        async move {
            let mut buffer = Vec::with_capacity(1024);
            loop {
                let (mut r, mut buf) = io.read(buffer).await;
                if r? == 0 {
                    break;
                }
                for b in buf.iter_mut() {
                    *b = self.replace;
                }
                (r, buffer) = io.write_all(buf).await;
                r?;
            }
            tracing::info!("tcp relay finished successfully");
            Ok(())
        }
    }
}

impl MakeService for EchoReplaceService {
    type Service = Self;

    type Error = Infallible;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(Self {
            replace: self.replace,
        })
    }
}

impl EchoReplaceService {
    pub fn layer<C>() -> impl FactoryLayer<C, (), Factory = Self>
    where
        C: Param<EchoReplaceConfig>,
    {
        layer_fn(|c: &C, ()| Self {
            replace: c.param().replace,
        })
    }
}
