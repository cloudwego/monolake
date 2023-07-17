use std::{convert::Infallible, future::Future};

use bytes::Bytes;
use http::{header, HeaderMap, HeaderValue, Request, StatusCode};
use monoio_http::common::body::HttpBody;
use monoio_http_client::Client;
use monolake_core::{
    context::{PeerAddr, RemoteAddr},
    http::ResponseWithContinue,
    listener::AcceptedAddr,
};
use service_async::{MakeService, ParamMaybeRef, ParamRef, Service};

use crate::http::generate_response;

#[derive(Clone, Default)]
pub struct ProxyHandler {
    client: Client,
}

impl ProxyHandler {
    pub fn new(client: Client) -> Self {
        ProxyHandler { client }
    }

    pub fn factory() -> ProxyHandlerFactory {
        ProxyHandlerFactory
    }
}

impl<CX> Service<(Request<HttpBody>, CX)> for ProxyHandler
where
    CX: ParamRef<PeerAddr> + ParamMaybeRef<Option<RemoteAddr>>,
{
    type Response = ResponseWithContinue;
    type Error = Infallible;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a, CX: 'a;

    fn call(&self, (mut req, ctx): (Request<HttpBody>, CX)) -> Self::Future<'_> {
        add_xff_header(req.headers_mut(), &ctx);
        async move {
            match self.client.send_request(req).await {
                Ok(resp) => Ok((resp, true)),
                // Bad gateway should not affect inbound connection.
                // It should still be keepalive.
                Err(_e) => Ok((generate_response(StatusCode::BAD_GATEWAY, false), true)),
            }
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
        #[cfg(target_os = "linux")]
        AcceptedAddr::Unix(addr) => {
            if let Some(path) = addr.as_pathname().and_then(|s| s.to_str()) {
                if let Ok(value) = HeaderValue::from_str(path) {
                    headers.insert(header::FORWARDED, value);
                }
            }
        }
    }
}
