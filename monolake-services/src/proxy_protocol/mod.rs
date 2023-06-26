use std::{fmt::Display, future::Future, net::SocketAddr};

use bytes::BytesMut;
use monoio::io::{AsyncReadRent, AsyncWriteRent, PrefixedReadIo};
use monolake_core::{context::keys::RemoteAddr, listener::AcceptedAddr, AnyError};
use proxy_protocol::{parse, version1, version2, ParseError, ProxyHeader};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, ParamSet, Service,
};

use crate::common::Accept;

pub struct ProxyProtocolService<T> {
    inner: T,
}

impl<S, T, CX> Service<(S, CX)> for ProxyProtocolService<T>
where
    S: AsyncReadRent + AsyncWriteRent,
    T: Service<Accept<PrefixedReadIo<S, std::io::Cursor<BytesMut>>, CX::Transformed>>,
    T::Error: Into<AnyError> + Display,
    CX: ParamSet<Option<RemoteAddr>>,
{
    type Response = T::Response;
    type Error = AnyError;
    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        Accept<S, CX>: 'cx;

    fn call(&self, (mut stream, ctx): Accept<S, CX>) -> Self::Future<'_> {
        async move {
            tracing::debug!("proxy protocol service!!!!");
            let buf = BytesMut::with_capacity(216);
            // TODO: we must check if io success
            let (_, mut buf) = stream.read(buf).await;
            let mut remote_addr = None;
            // TODO: This is a definitely wrong parsing.
            match parse(&mut buf) {
                Ok(header) => {
                    tracing::warn!("proxy-protocol header: {:?}", header);
                    remote_addr = match header {
                        ProxyHeader::Version1 {
                            addresses: version1::ProxyAddresses::Ipv4 { source, .. },
                        }
                        | ProxyHeader::Version2 {
                            addresses: version2::ProxyAddresses::Ipv4 { source, .. },
                            ..
                        } => Some(RemoteAddr(AcceptedAddr::from(SocketAddr::from(source)))),
                        ProxyHeader::Version1 {
                            addresses: version1::ProxyAddresses::Ipv6 { source, .. },
                        }
                        | ProxyHeader::Version2 {
                            addresses: version2::ProxyAddresses::Ipv6 { source, .. },
                            ..
                        } => Some(RemoteAddr(AcceptedAddr::from(SocketAddr::from(source)))),
                        _ => {
                            tracing::warn!("proxy protocol get source failed");
                            None
                        }
                    };
                }
                Err(ParseError::NotProxyHeader) => tracing::debug!("Not proxy protocol."),
                Err(ParseError::InvalidVersion { version }) => {
                    tracing::info!("Proxy protocol version {} is not supported", version)
                }
                Err(e) => tracing::error!("Proxy protocol process error: {:?}", e),
            }

            let ctx = ctx.param_set(remote_addr);
            let cursor = std::io::Cursor::new(buf);
            let prefix_io = PrefixedReadIo::new(stream, cursor);

            self.inner
                .call((prefix_io, ctx))
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
