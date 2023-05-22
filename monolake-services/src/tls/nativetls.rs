use std::future::Future;

use anyhow::bail;
use monoio::io::{AsyncReadRent, AsyncWriteRent, Split};
use monoio_native_tls::{TlsAcceptor, TlsStream};
use monolake_core::{
    service::ServiceError,
    service::{Service, ServiceLayer},
    tls::IDENTITY_MAP,
};
use native_tls::Identity;
use tower_layer::{layer_fn, Layer};

use crate::common::Accept;

type NativeTlsAccept<Stream, SocketAddr> = (TlsStream<Stream>, SocketAddr);

#[derive(Clone)]
pub struct NativeTlsService<T> {
    identity: Identity,
    inner: T,
}

impl<T, Stream, SocketAddr> Service<Accept<Stream, SocketAddr>> for NativeTlsService<T>
where
    T: Service<NativeTlsAccept<Stream, SocketAddr>>,
    Stream: Split + AsyncReadRent + AsyncWriteRent + 'static,
    SocketAddr: 'static,
{
    type Response = T::Response;

    type Error = ServiceError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx;

    fn call(&self, accept: Accept<Stream, SocketAddr>) -> Self::Future<'_> {
        async move {
            let acceptor = self.get_acceptor()?;
            match acceptor.accept(accept.0).await {
                Ok(stream) => match self.inner.call((stream, accept.1)).await {
                    Ok(resp) => Ok(resp),
                    Err(err) => {
                        bail!("{}", err)
                    }
                },
                Err(e) => bail!("tls error: {:?}", e.to_string()),
            }
        }
    }
}

impl<T> ServiceLayer<T> for NativeTlsService<T> {
    type Param = String;
    type Layer = impl Layer<T, Service = Self>;

    fn layer(param: Self::Param) -> Self::Layer {
        let identity = IDENTITY_MAP.read().unwrap().get(&param).unwrap().clone();
        layer_fn(move |inner| NativeTlsService {
            inner,
            identity: identity.clone(),
        })
    }
}

impl<T> NativeTlsService<T> {
    fn get_acceptor(&self) -> anyhow::Result<TlsAcceptor> {
        let builder = native_tls::TlsAcceptor::builder(self.identity.clone());
        match builder.build() {
            Ok(acceptor) => Ok(TlsAcceptor::from(acceptor)),
            Err(e) => bail!("Tls acceptor configure error: {:?}", e),
        }
    }
}
