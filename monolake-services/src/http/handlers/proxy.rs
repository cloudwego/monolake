use std::{convert::Infallible, future::Future};

use http::{Request, StatusCode};
use monoio_http::h1::payload::Payload;
use monoio_http_client::Client;
use monolake_core::http::ResponseWithContinue;
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
}

impl Service<Request<Payload>> for ProxyHandler {
    type Response = ResponseWithContinue;
    type Error = Infallible;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a;

    fn call(&self, req: Request<Payload>) -> Self::Future<'_> {
        async move {
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
