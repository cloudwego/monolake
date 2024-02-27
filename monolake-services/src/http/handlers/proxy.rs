use std::{
    convert::Infallible,
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bytes::Bytes;
use http::{header, HeaderMap, HeaderValue, Request, StatusCode};
use monoio::net::TcpStream;
use monoio_http::common::body::HttpBody;
#[cfg(feature = "tls")]
use monoio_transports::connectors::{TlsConnector, TlsStream};
use monoio_transports::{
    connectors::{Connector, TcpConnector, TcpTlsAddr},
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
type HttpsConnector = H1Connector<TlsConnector<TcpConnector>, TcpTlsAddr, TlsStream<TcpStream>>;

#[derive(Clone)]
pub struct ProxyHandler {
    connector: HttpConnector,
    #[cfg(feature = "tls")]
    tls_connector: HttpsConnector,
}

impl Default for ProxyHandler {
    fn default() -> Self {
        Self {
            connector: HttpConnector::default().with_default_pool(),
            #[cfg(feature = "tls")]
            tls_connector: HttpsConnector::default().with_default_pool(),
        }
    }
}

impl ProxyHandler {
    #[cfg(not(feature = "tls"))]
    pub fn new(connector: HttpConnector) -> Self {
        ProxyHandler { connector }
    }

    #[cfg(feature = "tls")]
    pub fn new(connector: HttpConnector, tls_connector: HttpsConnector) -> Self {
        ProxyHandler {
            connector,
            tls_connector,
        }
    }

    pub const fn factory(timeout: Option<Duration>) -> ProxyHandlerFactory {
        ProxyHandlerFactory {
            http_timeout: timeout,
        }
    }
}

impl<CX> Service<(Request<HttpBody>, CX)> for ProxyHandler
where
    CX: ParamRef<PeerAddr> + ParamMaybeRef<Option<RemoteAddr>>,
{
    type Response = ResponseWithContinue;
    type Error = Infallible;

    async fn call(
        &self,
        (mut req, ctx): (Request<HttpBody>, CX),
    ) -> Result<Self::Response, Self::Error> {
        add_xff_header(req.headers_mut(), &ctx);
        #[cfg(feature = "tls")]
        if req.uri().scheme() == Some(&http::uri::Scheme::HTTPS) {
            return self.send_https_request(req).await;
        }
        self.send_http_request(req).await
    }
}

impl ProxyHandler {
    async fn send_http_request(
        &self,
        req: Request<HttpBody>,
    ) -> Result<ResponseWithContinue, Infallible> {
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
    async fn send_https_request(
        &self,
        req: Request<HttpBody>,
    ) -> Result<ResponseWithContinue, Infallible> {
        let key = match req.uri().try_into() {
            Ok(key) => key,
            Err(e) => {
                info!("convert invalid uri: {:?} with error: {:?}", req.uri(), e);
                return Ok((generate_response(StatusCode::BAD_REQUEST, true), true));
            }
        };
        debug!("key: {:?}", key);
        let mut conn = match self.tls_connector.connect(key).await {
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

pub struct ProxyHandlerFactory {
    http_timeout: Option<Duration>,
}

impl ProxyHandlerFactory {
    pub fn new(timeout: Option<Duration>) -> ProxyHandlerFactory {
        ProxyHandlerFactory {
            http_timeout: timeout,
        }
    }
}

// HttpCoreService is a Service and a MakeService.
impl MakeService for ProxyHandlerFactory {
    type Service = ProxyHandler;
    type Error = Infallible;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        let mut factory = Ok(ProxyHandler::default());
        factory.as_mut().unwrap().connector.read_timeout = self.http_timeout;
        factory
    }
}

impl AsyncMakeService for ProxyHandlerFactory {
    type Service = ProxyHandler;
    type Error = Infallible;

    async fn make_via_ref(
        &self,
        _old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        let mut factory = Ok(ProxyHandler::default());
        factory.as_mut().unwrap().connector.read_timeout = self.http_timeout;
        factory
    }
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
