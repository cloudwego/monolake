use std::{convert::Infallible, future::Future};

use http::{header, HeaderMap, HeaderValue, Request, StatusCode};
use monoio_http::h1::payload::Payload;
use monoio_http_client::Client;
use monolake_core::{
    environments::{Environments, ValueType, PEER_ADDR, REMOTE_ADDR},
    http::ResponseWithContinue,
};
use service_async::{MakeService, Service};

use crate::http::generate_response;

#[derive(Clone)]
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

    pub fn add_xff_header(
        &self,
        headers: &mut HeaderMap,
        remote_addr: Option<&ValueType>,
        peer_addr: Option<&ValueType>,
    ) {
        let header_value = match remote_addr.clone() {
            Some(ValueType::SocketAddr(socket_addr)) => {
                HeaderValue::from_maybe_shared(socket_addr.ip().to_string()).ok()
            }
            Some(ValueType::Path(path)) => match path.to_str() {
                Some(path) => HeaderValue::from_str(path).ok(),
                None => None,
            },
            _ => match peer_addr.clone() {
                Some(ValueType::SocketAddr(socket_addr)) => {
                    HeaderValue::from_maybe_shared(socket_addr.ip().to_string()).ok()
                }
                Some(ValueType::Path(path)) => match path.to_str() {
                    Some(path) => HeaderValue::from_str(path).ok(),
                    None => None,
                },
                _ => None,
            },
        };
        match header_value {
            Some(value) => {
                headers.insert(header::FORWARDED, value);
            }
            None => (),
        }
    }
}

impl Service<(Request<Payload>, Environments)> for ProxyHandler {
    type Response = ResponseWithContinue;
    type Error = Infallible;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a;

    fn call(&self, (mut req, environments): (Request<Payload>, Environments)) -> Self::Future<'_> {
        async move {
            self.add_xff_header(
                req.headers_mut(),
                environments.get(&REMOTE_ADDR.to_string()),
                environments.get(&PEER_ADDR.to_string()),
            );
            match self.client.send(req).await {
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
        Ok(ProxyHandler {
            client: Default::default(),
        })
    }
}
