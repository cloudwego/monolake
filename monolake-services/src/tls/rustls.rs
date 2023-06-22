use std::{fmt::Display, future::Future, sync::Arc};

use monoio::io::{AsyncReadRent, AsyncWriteRent};
use monoio_rustls::{ServerTlsStream, TlsAcceptor};
use monolake_core::{
    environments::{Environments, ValueType, ALPN_PROTOCOL},
    AnyError,
};
use rustls::ServerConfig;
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};

use crate::common::Accept;

type RustlsAccept<Stream, Environments> = (ServerTlsStream<Stream>, Environments);

pub struct RustlsService<T> {
    acceptor: TlsAcceptor,
    inner: T,
}

impl<T, S> Service<Accept<S, Environments>> for RustlsService<T>
where
    T: Service<RustlsAccept<S, Environments>>,
    T::Error: Into<AnyError> + Display,
    S: AsyncReadRent + AsyncWriteRent,
{
    type Response = T::Response;

    type Error = AnyError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        Accept<S, Environments>: 'cx;

    fn call(&self, (stream, mut environments): Accept<S, Environments>) -> Self::Future<'_> {
        async move {
            let stream = self.acceptor.accept(stream).await?;

            match stream.alpn_protocol() {
                Some(alpn_protocol) => {
                    environments.insert(ALPN_PROTOCOL.to_string(), ValueType::String(alpn_protocol))
                }
                None => (),
            }

            self.inner
                .call((stream, environments))
                .await
                .map_err(Into::into)
        }
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
