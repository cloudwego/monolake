use std::future::Future;

use http::{Request, Version};
use monoio_http::h1::payload::Payload;
use monolake_core::http::{HttpHandler, ResponseWithContinue};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Service,
};
use tracing::debug;

use crate::http::{CLOSE, CLOSE_VALUE, KEEPALIVE, KEEPALIVE_VALUE};

/// ConnReuseHandler judges whether the request supports keepalive,
/// and adds relevant headers to the response.
#[derive(Clone)]
pub struct ConnReuseHandler<H> {
    inner: H,
}

impl<H> Service<Request<Payload>> for ConnReuseHandler<H>
where
    H: HttpHandler,
{
    type Response = ResponseWithContinue;
    type Error = H::Error;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a, Request<Payload>: 'a;

    fn call(&self, mut request: Request<Payload>) -> Self::Future<'_> {
        async move {
            let version = request.version();
            let keepalive = is_conn_keepalive(request.headers(), version);
            debug!("frontend keepalive {:?}", keepalive);

            match version {
                // for http 1.0, hack it to 1.1 like setting nginx `proxy_http_version` to 1.1
                Version::HTTP_10 => {
                    // modify to 1.1 and remove connection header
                    *request.version_mut() = Version::HTTP_11;
                    let _ = request.headers_mut().remove(http::header::CONNECTION);

                    // send
                    let (mut response, mut cont) = self.inner.handle(request).await?;
                    cont &= keepalive;

                    // modify back and make sure reply keepalive if client want it and server
                    // support it.
                    let _ = response.headers_mut().remove(http::header::CONNECTION);
                    if cont {
                        // insert keepalive header
                        response
                            .headers_mut()
                            .insert(http::header::CONNECTION, KEEPALIVE_VALUE);
                    }
                    *response.version_mut() = version;

                    Ok((response, cont))
                }
                Version::HTTP_11 => {
                    // remove connection header
                    let _ = request.headers_mut().remove(http::header::CONNECTION);

                    // send
                    let (mut response, mut cont) = self.inner.handle(request).await?;
                    cont &= keepalive;

                    // modify back and make sure reply keepalive if client want it and server
                    // support it.
                    let _ = response.headers_mut().remove(http::header::CONNECTION);
                    if !cont {
                        // insert close header
                        response
                            .headers_mut()
                            .insert(http::header::CONNECTION, CLOSE_VALUE);
                    }
                    Ok((response, cont))
                }
                // for http 0.9 and other versions, just relay it
                _ => {
                    let (response, _) = self.inner.handle(request).await?;
                    Ok((response, false))
                }
            }
        }
    }
}

// ConnReuseHandler is a Service and a MakeService.
impl<F> MakeService for ConnReuseHandler<F>
where
    F: MakeService,
{
    type Service = ConnReuseHandler<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ConnReuseHandler {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
        })
    }
}

impl<F> ConnReuseHandler<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_: &C, inner| Self { inner })
    }
}

fn is_conn_keepalive(headers: &http::HeaderMap<http::HeaderValue>, version: Version) -> bool {
    match (version, headers.get(http::header::CONNECTION)) {
        (Version::HTTP_10, Some(header))
            if header.as_bytes().eq_ignore_ascii_case(KEEPALIVE.as_bytes()) =>
        {
            true
        }
        (Version::HTTP_11, None) => true,
        (Version::HTTP_11, Some(header))
            if !header.as_bytes().eq_ignore_ascii_case(CLOSE.as_bytes()) =>
        {
            true
        }
        _ => false,
    }
}
