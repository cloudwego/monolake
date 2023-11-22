use std::fmt::Display;

use monoio::io::{AsyncReadRent, AsyncWriteRent};
use monoio_native_tls::{TlsAcceptor, TlsStream};
use monolake_core::AnyError;
use native_tls::Identity;
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};

use crate::tcp::Accept;

type NativeTlsAccept<Stream, SocketAddr> = (TlsStream<Stream>, SocketAddr);

#[derive(Clone)]
pub struct NativeTlsService<T> {
    acceptor: TlsAcceptor,
    inner: T,
}

impl<T, S, CX> Service<Accept<S, CX>> for NativeTlsService<T>
where
    T: Service<NativeTlsAccept<S, CX>>,
    T::Error: Into<AnyError> + Display,
    S: AsyncReadRent + AsyncWriteRent,
{
    type Response = T::Response;
    type Error = AnyError;

    async fn call(&self, (stream, addr): Accept<S, CX>) -> Result<Self::Response, Self::Error> {
        let stream = self.acceptor.accept(stream).await?;
        self.inner.call((stream, addr)).await.map_err(Into::into)
    }
}

pub struct NativeTlsServiceFactory<F> {
    identity: Identity,
    inner: F,
}

impl<F> NativeTlsServiceFactory<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Identity>,
    {
        layer_fn(|c: &C, inner| NativeTlsServiceFactory {
            identity: c.param(),
            inner,
        })
    }
}

impl<F> MakeService for NativeTlsServiceFactory<F>
where
    F: MakeService,
    F::Error: Into<AnyError>,
{
    type Service = NativeTlsService<F::Service>;
    type Error = AnyError;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        let builder = native_tls::TlsAcceptor::builder(self.identity.clone());
        let acceptor = TlsAcceptor::from(builder.build().map_err(AnyError::from)?);
        Ok(NativeTlsService {
            acceptor,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
        })
    }
}
