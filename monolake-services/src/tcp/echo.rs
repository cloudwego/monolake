use std::{convert::Infallible, io};

use monoio::io::{AsyncReadRent, AsyncWriteRent, AsyncWriteRentExt};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, Service,
};

pub struct EchoService {
    buffer_size: usize,
}

impl<S> Service<S> for EchoService
where
    S: AsyncReadRent + AsyncWriteRent,
{
    type Response = ();
    type Error = io::Error;

    async fn call(&self, mut io: S) -> Result<Self::Response, Self::Error> {
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

impl MakeService for EchoService {
    type Service = Self;
    type Error = Infallible;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(EchoService {
            buffer_size: self.buffer_size,
        })
    }
}

impl AsyncMakeService for EchoService {
    type Service = Self;
    type Error = Infallible;

    async fn make_via_ref(
        &self,
        _old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(EchoService {
            buffer_size: self.buffer_size,
        })
    }
}

#[derive(Debug, Clone)]
pub struct EchoConfig {
    pub buffer_size: usize,
}

impl Default for EchoConfig {
    fn default() -> Self {
        Self { buffer_size: 4096 }
    }
}

impl EchoService {
    pub fn layer<C>(&self) -> impl FactoryLayer<C, (), Factory = Self>
    where
        C: Param<EchoConfig>,
    {
        layer_fn(|c: &C, ()| Self {
            buffer_size: c.param().buffer_size,
        })
    }
}

#[monoio::test_all]
async fn test_echo() {
    let s:EchoService = EchoService {
        buffer_size: 4096
    };
    let _ec:EchoConfig = EchoConfig::default();
    let d = 1;
    let _s2 = MakeService::make(&s).unwrap();
    let _s3 = AsyncMakeService::make(&s).await.unwrap();
    let _ = s.layer::<EchoConfig>();

    assert_eq!(d, 1);
}
