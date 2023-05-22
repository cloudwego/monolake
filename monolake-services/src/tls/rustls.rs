use std::{future::Future, sync::Arc};

use anyhow::bail;
use monoio::io::{AsyncReadRent, AsyncWriteRent, Split};
use monoio_rustls::{ServerTlsStream, TlsAcceptor};
use monolake_core::{
    service::ServiceError,
    service::{Service, ServiceLayer},
    tls::CertificateResolver,
};
use rustls::ServerConfig;
use tower_layer::{layer_fn, Layer};

use crate::common::Accept;

type TlsAccept<Stream, SocketAddr> = (ServerTlsStream<Stream>, SocketAddr);

#[derive(Clone)]
pub struct RustlsService<T> {
    config: ServerConfig,
    inner: T,
}

impl<T, Stream, SocketAddr> Service<Accept<Stream, SocketAddr>> for RustlsService<T>
where
    T: Service<TlsAccept<Stream, SocketAddr>>,
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
            let acceptor = self.get_acceptor();
            match acceptor.accept(accept.0).await {
                Ok(stream) => match self.inner.call((stream, accept.1)).await {
                    Ok(resp) => Ok(resp),
                    Err(err) => bail!("{}", err),
                },
                Err(err) => bail!("TLS error: {:?}", err),
            }
        }
    }
}

impl<T> ServiceLayer<T> for RustlsService<T> {
    type Param = String;
    type Layer = impl Layer<T, Service = Self>;

    fn layer(param: Self::Param) -> Self::Layer {
        let config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(CertificateResolver::new(param)));
        layer_fn(move |inner: T| RustlsService {
            config: config.clone(),
            inner,
        })
    }
}

impl<S> RustlsService<S> {
    fn get_acceptor(&self) -> TlsAcceptor {
        TlsAcceptor::from(self.config.clone())
    }
}
