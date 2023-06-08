use std::{convert::Infallible, future::Future};

use http::{Request, Response, StatusCode};
use monoio_http::h1::payload::Payload;
use monoio_http_client::Client;
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
    type Response = Response<Payload>;
    type Error = Infallible;
    type Future<'a> = impl Future<Output = Result<Response<Payload>, Self::Error>> + 'a
        where
            Self: 'a;

    fn call(&self, req: Request<Payload>) -> Self::Future<'_> {
        async move {
            match self.client.send(req).await {
                Ok(resp) => Ok(resp),
                // TODO(ihciah): Is it ok to return Ok even when Err?
                // TODO(ihciah): More accurate status code
                Err(_e) => Ok(generate_response(StatusCode::INTERNAL_SERVER_ERROR)),
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
