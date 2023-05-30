use std::{fmt::Display, future::Future};

use anyhow::bail;
use monoio::io::{AsyncReadRent, AsyncWriteRent};
use monoio_native_tls::{TlsAcceptor, TlsStream};
use monolake_core::service::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};
use native_tls::Identity;

use crate::{common::Accept, AnyError};

type NativeTlsAccept<Stream, SocketAddr> = (TlsStream<Stream>, SocketAddr);

#[derive(Clone)]
pub struct NativeTlsService<T> {
    acceptor: TlsAcceptor,
    inner: T,
}

impl<T, S, A> Service<Accept<S, A>> for NativeTlsService<T>
where
    T: Service<NativeTlsAccept<S, A>>,
    T::Error: Into<AnyError> + Display,
    S: AsyncReadRent + AsyncWriteRent,
{
    type Response = T::Response;

    type Error = AnyError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        Accept<S, A>: 'cx;

    fn call(&self, (stream, addr): Accept<S, A>) -> Self::Future<'_> {
        async move {
            let stream = self.acceptor.accept(stream).await?;
            self.inner.call((stream, addr)).await.map_err(Into::into)
        }
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
        layer_fn::<C, _, _, _>(|c, inner| NativeTlsServiceFactory {
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
        let acceptor = match builder.build() {
            Ok(acceptor) => TlsAcceptor::from(acceptor),
            Err(e) => bail!("Tls acceptor configure error: {:?}", e),
        };
        Ok(NativeTlsService {
            acceptor,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
        })
    }
}
