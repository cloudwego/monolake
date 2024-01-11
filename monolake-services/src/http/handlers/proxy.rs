use std::convert::Infallible;

use bytes::Bytes;
use http::{header, uri::Scheme, HeaderMap, HeaderValue, Request, StatusCode};
use monoio::net::TcpStream;
use monoio_http::common::body::HttpBody;
use monoio_transports::{
    connectors::{Connector, TcpConnector, TlsConnector, TlsStream},
    http::HttpConnector,
    key::Key,
    pooled::connector::PooledConnector,
};
use monolake_core::{
    context::{PeerAddr, RemoteAddr},
    http::ResponseWithContinue,
    listener::AcceptedAddr,
};
use service_async::{MakeService, ParamMaybeRef, ParamRef, Service};
use tracing::{debug, info};

use crate::http::generate_response;

type PoolHttpConnector = HttpConnector<PooledConnector<TcpConnector, Key, TcpStream, ()>>;
type PoolHttpsConnector =
    HttpConnector<PooledConnector<TlsConnector<TcpConnector>, Key, TlsStream<TcpStream>, ()>>;

#[derive(Clone, Default)]
pub struct ProxyHandler {
    connector: PoolHttpConnector,
    tls_connector: PoolHttpsConnector,
}

impl ProxyHandler {
    pub fn new(connector: PoolHttpConnector, tls_connector: PoolHttpsConnector) -> Self {
        ProxyHandler {
            connector,
            tls_connector,
        }
    }

    pub const fn factory() -> ProxyHandlerFactory {
        ProxyHandlerFactory
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
        if req.uri().scheme() == Some(&Scheme::HTTPS) {
            self.send_https_request(req).await
        } else {
            self.send_http_request(req).await
        }
    }
}

impl ProxyHandler {
    async fn send_http_request(
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

pub struct ProxyHandlerFactory;

// HttpCoreService is a Service and a MakeService.
impl MakeService for ProxyHandlerFactory {
    type Service = ProxyHandler;
    type Error = Infallible;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ProxyHandler::default())
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
