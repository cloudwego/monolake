use std::{fmt::Display, sync::Arc};

use monoio::io::{AsyncReadRent, AsyncWriteRent};
use monoio_rustls::{ServerTlsStream, TlsAcceptor};
use monolake_core::AnyError;
use rustls::ServerConfig;
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, Service,
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

impl<F: MakeService> MakeService for RustlsServiceFactory<F> {
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

impl<F: AsyncMakeService> AsyncMakeService for RustlsServiceFactory<F> {
    type Service = RustlsService<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        let acceptor = TlsAcceptor::from(self.config.clone());
        Ok(RustlsService {
            acceptor,
            inner: self.inner.make_via_ref(old.map(|o| &o.inner)).await?,
        })
    }
}

#[monoio::test_all(timer_enabled = true)]
async fn test_rustls() {
    use crate::common::delay::DummyService;

    let d = 1;
    let es:DummyService = DummyService{};

    let fname = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/certs/cert.pem");
    let cert_file = &mut std::io::BufReader::new(std::fs::File::open(fname).unwrap());
    let key_file = &mut std::io::BufReader::new(std::fs::File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/certs/key.pem")).unwrap());
    let certs = rustls_pemfile::certs(cert_file).unwrap();
    let cert_vec:Vec<rustls::Certificate> = certs.into_iter().map(rustls::Certificate).collect();
    let mut private_key = rustls_pemfile::pkcs8_private_keys(key_file).unwrap();
    let key: rustls::PrivateKey = rustls::PrivateKey(private_key.remove(0));
    
    let server_config = ServerConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(cert_vec, key)
        .expect("bad certificate/key");
    let s:RustlsServiceFactory<DummyService> = RustlsServiceFactory {
        config: Arc::new(server_config),
        inner: es,
    };
    let _s2 = MakeService::make(&s).unwrap();
    let _s3 = AsyncMakeService::make(&s).await.unwrap();

    assert_eq!(d, 1);
}
