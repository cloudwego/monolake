use std::{fmt::Display, future::Future, sync::Arc};

use monoio::io::{AsyncReadRent, AsyncWriteRent};
use monoio_rustls::{ServerTlsStream, TlsAcceptor};
use monolake_core::service::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};
use rustls::ServerConfig;

use crate::{common::Accept, AnyError};

type RustlsAccept<Stream, SocketAddr> = (ServerTlsStream<Stream>, SocketAddr);

pub struct RustlsService<T> {
    acceptor: TlsAcceptor,
    inner: T,
}

impl<T, S, A> Service<Accept<S, A>> for RustlsService<T>
where
    T: Service<RustlsAccept<S, A>>,
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

pub struct RustlsServiceFactory<F> {
    config: Arc<ServerConfig>,
    inner: F,
}

impl<F> RustlsServiceFactory<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<ServerConfig>,
    {
        layer_fn::<C, _, _, _>(|c, inner| RustlsServiceFactory {
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
