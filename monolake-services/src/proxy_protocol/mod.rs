use std::{fmt::Display, future::Future, path::PathBuf};

use bytes::BytesMut;
use monoio::io::{AsyncReadRent, AsyncWriteRent, PrefixedReadIo};
use monolake_core::{
    environments::{Environments, ValueType, REMOTE_ADDR},
    AnyError,
};
use proxy_protocol::{parse, version1, version2, ParseError, ProxyHeader};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Service,
};

use crate::common::Accept;

pub struct ProxyProtocolService<T> {
    inner: T,
}

impl<S, T> Service<(S, Environments)> for ProxyProtocolService<T>
where
    S: AsyncReadRent + AsyncWriteRent,
    T: Service<Accept<PrefixedReadIo<S, std::io::Cursor<BytesMut>>, Environments>>,
    T::Error: Into<AnyError> + Display,
{
    type Response = T::Response;

    type Error = AnyError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        Accept<S, Environments>: 'cx;

    fn call(&self, (stream, environments): Accept<S, Environments>) -> Self::Future<'_> {
        async move {
            tracing::debug!("proxy protocol service!!!!");
            let buf = BytesMut::with_capacity(216);
            let (mut stream, mut environments): (S, Environments) = (stream, environments);
            let (_, mut buf) = stream.read(buf).await;
            match parse(&mut buf) {
                Ok(header) => {
                    tracing::warn!("header: {:?}", header);
                    // environments.insert(REMOTE_ADDR, ValueType::String(header.));
                    match header {
                        ProxyHeader::Version1 { addresses } => match addresses {
                            version1::ProxyAddresses::Ipv4 { source, .. } => environments.insert(
                                REMOTE_ADDR.to_string(),
                                ValueType::SocketAddr(source.into()),
                            ),
                            version1::ProxyAddresses::Ipv6 { source, .. } => environments.insert(
                                REMOTE_ADDR.to_string(),
                                ValueType::SocketAddr(source.into()),
                            ),
                            version1::ProxyAddresses::Unknown => {
                                tracing::warn!(
                                    "Unknown remote address from proxy protocol v1 header"
                                );
                            }
                        },
                        ProxyHeader::Version2 { addresses, .. } => match addresses {
                            version2::ProxyAddresses::Ipv4 { source, .. } => environments.insert(
                                REMOTE_ADDR.to_string(),
                                ValueType::SocketAddr(source.into()),
                            ),
                            version2::ProxyAddresses::Ipv6 { source, .. } => environments.insert(
                                REMOTE_ADDR.to_string(),
                                ValueType::SocketAddr(source.into()),
                            ),
                            version2::ProxyAddresses::Unix { source, .. } => {
                                let remote_addr = String::from_utf8(source.to_vec());
                                match remote_addr {
                                    Ok(remote_addr) => environments.insert(
                                        REMOTE_ADDR.to_string(),
                                        ValueType::Path(PathBuf::from(remote_addr)),
                                    ),
                                    Err(e) => {
                                        tracing::error!("Parse unix remote address error: {:?}", e)
                                    }
                                }
                            }
                            version2::ProxyAddresses::Unspec => {
                                tracing::warn!(
                                    "Unknown remote address from proxy protocol v2 header"
                                );
                            }
                        },
                        _ => {
                            tracing::warn!("Unknown version of proxy protocol header");
                        }
                    }
                }
                Err(ParseError::NotProxyHeader) => tracing::debug!("Not proxy protocol."),
                Err(ParseError::InvalidVersion { version }) => {
                    tracing::info!("Proxy protocol version {} is not supported", version)
                }
                Err(e) => tracing::error!("Proxy protocol process error: {:?}", e),
            }

            let cursor = std::io::Cursor::new(buf);
            let prefix_io = PrefixedReadIo::new(stream, cursor);

            self.inner
                .call((prefix_io, environments))
                .await
                .map_err(|e| e.into())
        }
    }
}

pub struct ProxyProtocolServiceFactory<F> {
    inner: F,
}

impl<F> ProxyProtocolServiceFactory<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_: &C, inner| ProxyProtocolServiceFactory { inner })
    }
}

impl<F> MakeService for ProxyProtocolServiceFactory<F>
where
    F: MakeService,
{
    type Service = ProxyProtocolService<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ProxyProtocolService {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
        })
    }
}
