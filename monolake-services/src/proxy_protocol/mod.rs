use std::{fmt::Display, future::Future, net::SocketAddr};

use monoio::{
    buf::IoBufMut,
    io::{AsyncReadRent, AsyncWriteRent, PrefixedReadIo},
};
use monolake_core::{context::RemoteAddr, listener::AcceptedAddr, AnyError};
use proxy_protocol::{parse, version1, version2, ParseError, ProxyHeader};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, ParamSet, Service,
};

use crate::tcp::Accept;

// Ref: https://www.haproxy.org/download/1.8/doc/proxy-protocol.txt
// V1 max length is 107-byte.
const V1HEADER: &[u8; 6] = &[b'P', b'R', b'O', b'X', b'Y', b' '];
// V2 max length is 14+216 = 230 bytes.
const V2HEADER: &[u8; 12] = &[
    0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A,
];

pub struct ProxyProtocolService<T> {
    inner: T,
}

impl<S, T, CX> Service<(S, CX)> for ProxyProtocolService<T>
where
    S: AsyncReadRent + AsyncWriteRent,
    T: Service<Accept<PrefixedReadIo<S, std::io::Cursor<Vec<u8>>>, CX::Transformed>>,
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
            const MAX_HEADER_SIZE: usize = 230;
            let mut buffer = Vec::with_capacity(MAX_HEADER_SIZE);
            let mut pos = 0;

            // read at-least 1 byte
            let (res, buf) = stream
                .read(unsafe { buffer.slice_mut_unchecked(0..MAX_HEADER_SIZE) })
                .await;
            buffer = buf.into_inner();
            pos += res.map_err(AnyError::from)?;
            // match version magic header
            let parsed = if let Some(target_header) = match buffer[0] {
                b'P' => {
                    let end = pos.min(V1HEADER.len());
                    if buffer[1..end] == V1HEADER[1..end] {
                        Some(&V1HEADER[..])
                    } else {
                        tracing::warn!("proxy-protocol: v1 magic only partly matched");
                        None
                    }
                }
                0x0D => {
                    let end = pos.min(V2HEADER.len());
                    if buffer[1..end] == V2HEADER[1..end] {
                        Some(&V2HEADER[..])
                    } else {
                        tracing::warn!("proxy-protocol: v2 magic only partly matched");
                        None
                    }
                }
                _ => None,
            } {
                // loop {parse; read; check_full;}
                let header = loop {
                    let mut cursor = std::io::Cursor::new(&buffer);
                    let e = match parse(&mut cursor) {
                        Ok(header) => break Ok((header, cursor.position())),
                        // data is not enough to parse version, we should read again
                        Err(
                            e @ ParseError::NotProxyHeader
                            | e @ ParseError::Version1 {
                                source: version1::ParseError::UnexpectedEof,
                            }
                            | e @ ParseError::Version2 {
                                source: version2::ParseError::UnexpectedEof,
                            },
                        ) => e,
                        Err(e) => break Err(e),
                    };

                    let buf = unsafe { buffer.slice_mut_unchecked(pos..MAX_HEADER_SIZE) };
                    let (res, buf) = stream.read(buf).await;
                    buffer = buf.into_inner();
                    let read = res.map_err(AnyError::from)?;
                    // if we are reading magic header, we have to check if the magic header matches
                    // because ParseError::NotProxyHeader does not always mean data is not enough
                    if pos < target_header.len() {
                        let end = target_header.len().min(pos + read);
                        if buffer[pos..end] != target_header[pos..end] {
                            break Err(e);
                        }
                    }
                    pos += read;
                    if pos == MAX_HEADER_SIZE {
                        return Err(ParseError::NotProxyHeader.into());
                    }
                };
                Some(header)
            } else {
                tracing::debug!("proxy-protocol: not proxy protocol at first glance");
                None
            };

            let mut cursor = std::io::Cursor::new(buffer);
            let remote_addr = match parsed {
                Some(Ok((header, idx))) => {
                    // advance proxy-protocol length on success parsing
                    cursor.set_position(idx);
                    match header {
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
                    }
                }
                _ => None,
            };

            let ctx = ctx.param_set(remote_addr);
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
