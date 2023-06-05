use std::{convert::Infallible, future::Future, io};

use monoio::io::{AsyncReadRent, AsyncWriteRent, AsyncWriteRentExt};
use monolake_core::service::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};

#[derive(Debug, Clone)]
pub struct EchoConfig {
    pub buffer_size: usize,
}

pub struct EchoService {
    buffer_size: usize,
}

impl<S> Service<S> for EchoService
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
            let mut buffer = Vec::with_capacity(self.buffer_size);
            loop {
                let (mut r, buf) = io.read(buffer).await;
                if r? == 0 {
                    break;
                }
                (r, buffer) = io.write_all(buf).await;
                r?;
            }
            tracing::info!("tcp relay finished successfully");
            Ok(())
        }
    }
}

impl MakeService for EchoService {
    type Service = Self;

    type Error = Infallible;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(EchoService {
            buffer_size: self.buffer_size,
        })
    }
}

impl EchoService {
    pub fn layer<C>() -> impl FactoryLayer<C, (), Factory = Self>
    where
        C: Param<EchoConfig>,
    {
        layer_fn::<C, (), _, _>(|c, ()| Self {
            buffer_size: c.param().buffer_size,
        })
    }
}
