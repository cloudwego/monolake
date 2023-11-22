use std::{fmt::Display, sync::Arc};

use monoio::io::{AsyncReadRent, AsyncWriteRent};
use monoio_rustls::{ServerTlsStream, TlsAcceptor};
use monolake_core::AnyError;
use rustls::ServerConfig;
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};

use crate::tcp::Accept;

type RustlsAccept<Stream, SocketAddr> = (ServerTlsStream<Stream>, SocketAddr);

pub struct RustlsService<T> {
    acceptor: TlsAcceptor,
    inner: T,
}

impl<T, S, CX> Service<Accept<S, CX>> for RustlsService<T>
where
    T: Service<RustlsAccept<S, CX>>,
    T::Error: Into<AnyError> + Display,
    S: AsyncReadRent + AsyncWriteRent,
{
    type Response = T::Response;
    type Error = AnyError;

    async fn call(&self, (stream, cx): Accept<S, CX>) -> Result<Self::Response, Self::Error> {
        let stream = self.acceptor.accept(stream).await?;
        self.inner.call((stream, cx)).await.map_err(Into::into)
    }
}

pub struct RustlsServiceFactory<F> {
    config: Arc<ServerConfig>,
    inner: F,
}

impl<F> RustlsServiceFactory<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<ServerConfig>,
    {
        layer_fn(|c: &C, inner| RustlsServiceFactory {
            config: Arc::new(c.param()),
            inner,
        })
    }
}

impl<F> MakeService for RustlsServiceFactory<F>
where
    F: MakeService,
{
    type Service = RustlsService<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        let acceptor = TlsAcceptor::from(self.config.clone());
        Ok(RustlsService {
            acceptor,
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
        })
    }
}
