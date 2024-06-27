//! Upstream proxy handling and request forwarding module.
//!
//! This module provides components for proxying HTTP and HTTPS requests to upstream servers,
//! leveraging high-performance HTTP client implementations optimized for use with monoio's
//! asynchronous runtime and io_uring.
//!
//! # Key Components
//!
//! - [`UpstreamHandler`]: The main service component responsible for proxying requests. It utilizes
//!   the `HttpConnector` for efficient connection management and request handling.
//! - [`UpstreamHandlerFactory`]: A factory for creating and updating `UpstreamHandler` instances.
//! - [`HttpUpstreamTimeout`]: Configuration for various timeout settings in upstream communication.
//!
//! # Features
//!
//! - HTTP and HTTPS request proxying using optimized connectors
//! - Connection pooling for efficient resource usage, provided by `HttpConnector`
//! - Support for both HTTP/1.1 and HTTP/2 protocols
//! - Configurable timeout settings
//! - TLS support (enabled with the `tls` feature flag)
//! - X-Forwarded-For header management
//! - Leverages monoio's native IO traits built on top of io_uring for high performance
//!
//! # HTTP Connector Usage
//!
//! The `UpstreamHandler` utilizes `HttpConnector`, which provides:
//!
//! - Unified interface for HTTP/1.1 and HTTP/2 connections
//! - Built-in connection pooling for efficient reuse of established connections
//! - Optimized for monoio's asynchronous runtime and io_uring
//! - TLS support for secure HTTPS connections
//!
//! # Error Handling
//!
//! - Connection errors result in 502 Bad Gateway responses
//! - Invalid URIs or unresolvable hosts result in 400 Bad Request responses
//! - Timeouts are handled gracefully, returning appropriate error responses
//!
//! # Performance Considerations
//!
//! - Utilizes `HttpConnector`'s connection pooling to reduce the overhead of creating new
//!   connections
//! - Employs efficient async I/O operations leveraging io_uring for improved performance
//! - Supports both HTTP/1.1 and HTTP/2, allowing for protocol-specific optimizations
//!
//! # Feature Flags
//!
//! - `tls`: Enables TLS support for HTTPS connections to upstream servers
use std::{
    convert::Infallible,
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bytes::Bytes;
use http::{header, HeaderMap, HeaderValue, Request, StatusCode};
use monoio::net::TcpStream;
use monoio_http::common::{
    body::{Body, HttpBody},
    error::HttpError,
};
#[cfg(feature = "tls")]
use monoio_transports::connectors::{TlsConnector, TlsStream};
use monoio_transports::{
    connectors::{Connector, TcpConnector},
    http::H1Connector,
};
use monolake_core::{
    context::{PeerAddr, RemoteAddr},
    http::ResponseWithContinue,
    listener::AcceptedAddr,
};
use service_async::{AsyncMakeService, MakeService, ParamMaybeRef, ParamRef, Service};
use tracing::{debug, info};

use crate::http::generate_response;

type HttpConnector = H1Connector<TcpConnector, SocketAddr, TcpStream>;
#[cfg(feature = "tls")]
type HttpsConnector = H1Connector<
    TlsConnector<TcpConnector>,
    monoio_transports::connectors::TcpTlsAddr,
    TlsStream<TcpStream>,
>;

/// Handles proxying of HTTP and HTTPS requests to upstream servers.
///
/// `UpstreamHandler` is responsible for forwarding incoming requests to appropriate
/// upstream servers, handling both HTTP and HTTPS protocols. It manages connection
/// pooling, timeout settings, and error handling.
///
/// For implementation details and example usage, see the
/// [module level documentation](crate::http::handlers::upstream).
#[derive(Clone)]
pub struct UpstreamHandler {
    connector: HttpConnector,
    #[cfg(feature = "tls")]
    tls_connector: HttpsConnector,
    pub http_upstream_timeout: HttpUpstreamTimeout,
}

impl Default for UpstreamHandler {
    fn default() -> Self {
        Self {
            connector: HttpConnector::default().with_default_pool(),
            #[cfg(feature = "tls")]
            tls_connector: HttpsConnector::default().with_default_pool(),
            http_upstream_timeout: Default::default(),
        }
    }
}

impl UpstreamHandler {
    #[cfg(not(feature = "tls"))]
    pub fn new(connector: HttpConnector) -> Self {
        UpstreamHandler {
            connector,
            http_upstream_timeout: Default::default(),
        }
    }

    #[cfg(feature = "tls")]
    pub fn new(connector: HttpConnector, tls_connector: HttpsConnector) -> Self {
        UpstreamHandler {
            connector,
            tls_connector,
            http_upstream_timeout: Default::default(),
        }
    }

    pub const fn factory(http_upstream_timeout: HttpUpstreamTimeout) -> UpstreamHandlerFactory {
        UpstreamHandlerFactory {
            http_upstream_timeout,
        }
    }
}

impl<CX, B> Service<(Request<B>, CX)> for UpstreamHandler
where
    CX: ParamRef<PeerAddr> + ParamMaybeRef<Option<RemoteAddr>>,
    B: Body,
    HttpError: From<B::Error>,
{
    type Response = ResponseWithContinue<HttpBody>;
    type Error = Infallible;

    async fn call(&self, (mut req, ctx): (Request<B>, CX)) -> Result<Self::Response, Self::Error> {
        add_xff_header(req.headers_mut(), &ctx);
        #[cfg(feature = "tls")]
        if req.uri().scheme() == Some(&http::uri::Scheme::HTTPS) {
            return self.send_https_request(req).await;
        }
        self.send_http_request(req).await
    }
}

impl UpstreamHandler {
    async fn send_http_request<B>(
        &self,
        req: Request<B>,
    ) -> Result<ResponseWithContinue<HttpBody>, Infallible>
    where
        B: Body,
        HttpError: From<B::Error>,
    {
        let Some(host) = req.uri().host() else {
            info!("invalid uri which does not contain host: {:?}", req.uri());
            return Ok((generate_response(StatusCode::BAD_REQUEST, true), true));
        };
        let port = req.uri().port_u16().unwrap_or(80);
        let mut iter = match (host, port).to_socket_addrs() {
            Ok(iter) => iter,
            Err(e) => {
                info!("convert invalid uri: {:?} with error: {:?}", req.uri(), e);
                return Ok((generate_response(StatusCode::BAD_REQUEST, true), true));
            }
        };
        let Some(key) = iter.next() else {
            info!("unable to resolve host: {host}");
            return Ok((generate_response(StatusCode::BAD_REQUEST, true), true));
        };
        debug!("key: {:?}", key);
        let mut conn = match self.connector.connect(key).await {
            Ok(conn) => conn,
            Err(e) => {
                info!("connect upstream error: {:?}", e);
                return Ok((generate_response(StatusCode::BAD_GATEWAY, true), true));
            }
        };

        match conn.send_request(req).await {
            (Ok(resp), _) => Ok((resp, true)),
            // Bad gateway should not affect inbound connection.
            // It should still be keepalive.
            (Err(_e), _) => Ok((generate_response(StatusCode::BAD_GATEWAY, false), true)),
        }
    }

    #[cfg(feature = "tls")]
    async fn send_https_request<B>(
        &self,
        req: Request<B>,
    ) -> Result<ResponseWithContinue<HttpBody>, Infallible>
    where
        B: Body,
        HttpError: From<B::Error>,
    {
        let key = match req.uri().try_into() {
            Ok(key) => key,
            Err(e) => {
                info!("convert invalid uri: {:?} with error: {:?}", req.uri(), e);
                return Ok((generate_response(StatusCode::BAD_REQUEST, true), true));
            }
        };
        debug!("key: {:?}", key);
        let connect = match self.http_upstream_timeout.connect_timeout {
            Some(connect_timeout) => {
                match monoio::time::timeout(connect_timeout, self.tls_connector.connect(key)).await
                {
                    Ok(x) => x,
                    Err(_) => {
                        info!("connect upstream timeout");
                        return Ok((generate_response(StatusCode::BAD_GATEWAY, true), true));
                    }
                }
            }
            None => self.tls_connector.connect(key).await,
        };

        let mut conn = match connect {
            Ok(conn) => conn,
            Err(e) => {
                info!("connect upstream error: {:?}", e);
                return Ok((generate_response(StatusCode::BAD_GATEWAY, true), true));
            }
        };

        match conn.send_request(req).await {
            (Ok(resp), _) => Ok((resp, true)),
            // Bad gateway should not affect inbound connection.
            // It should still be keepalive.
            (Err(_e), _) => Ok((generate_response(StatusCode::BAD_GATEWAY, false), true)),
        }
    }
}

pub struct UpstreamHandlerFactory {
    http_upstream_timeout: HttpUpstreamTimeout,
}

impl UpstreamHandlerFactory {
    pub fn new(http_upstream_timeout: HttpUpstreamTimeout) -> UpstreamHandlerFactory {
        UpstreamHandlerFactory {
            http_upstream_timeout,
        }
    }
}

// HttpCoreService is a Service and a MakeService.
impl MakeService for UpstreamHandlerFactory {
    type Service = UpstreamHandler;
    type Error = Infallible;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        let http_connector = HttpConnector::default().with_default_pool();
        Ok(UpstreamHandler {
            connector: http_connector,
            #[cfg(feature = "tls")]
            tls_connector: HttpsConnector::default().with_default_pool(),
            http_upstream_timeout: self.http_upstream_timeout,
        })
    }
}

impl AsyncMakeService for UpstreamHandlerFactory {
    type Service = UpstreamHandler;
    type Error = Infallible;

    async fn make_via_ref(
        &self,
        _old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        let http_connector = HttpConnector::default().with_default_pool();
        Ok(UpstreamHandler {
            connector: http_connector,
            #[cfg(feature = "tls")]
            tls_connector: HttpsConnector::default().with_default_pool(),
            http_upstream_timeout: self.http_upstream_timeout,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct HttpUpstreamTimeout {
    // Connect timeout
    // Link Nginx `proxy_connect_timeout`
    pub connect_timeout: Option<Duration>,
    // Read full http header.
    pub read_header_timeout: Option<Duration>,
    // Receiving full body timeout.
    pub read_body_timeout: Option<Duration>,
}

fn add_xff_header<CX>(headers: &mut HeaderMap, ctx: &CX)
where
    CX: ParamRef<PeerAddr> + ParamMaybeRef<Option<RemoteAddr>>,
{
    let peer_addr = ParamRef::<PeerAddr>::param_ref(ctx);
    let remote_addr = ParamMaybeRef::<Option<RemoteAddr>>::param_maybe_ref(ctx);
    let addr = remote_addr
        .and_then(|addr| addr.as_ref().map(|x| &x.0))
        .unwrap_or(&peer_addr.0);

    match addr {
        AcceptedAddr::Tcp(addr) => {
            if let Ok(value) = HeaderValue::from_maybe_shared(Bytes::from(addr.ip().to_string())) {
                headers.insert(header::FORWARDED, value);
            }
        }
        AcceptedAddr::Unix(addr) => {
            if let Some(path) = addr.as_pathname().and_then(|s| s.to_str()) {
                if let Ok(value) = HeaderValue::from_str(path) {
                    headers.insert(header::FORWARDED, value);
                }
            }
        }
    }
}
