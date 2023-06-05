use std::future::Future;

use http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode, Version};
use monoio_http::h1::payload::Payload;
use monolake_core::{
    config::KeepaliveConfig,
    http::HttpHandler,
    service::{layer::{FactoryLayer, layer_fn}, MakeService, Param, Service},
};
use tracing::debug;

use crate::http::{
    generate_response, is_conn_reuse, CONN_CLOSE, CONN_KEEP_ALIVE, COUNTER_HEADER_NAME,
    TIMER_HEADER_NAME,
};

#[derive(Clone)]
pub struct ConnReuseHandler<H> {
    inner: H,
    keepalive_config: Option<KeepaliveConfig>,
}

impl<H> ConnReuseHandler<H> {
    fn should_close_conn(&self, headers: &HeaderMap<HeaderValue>) -> bool {
        // TODO(ihciah): remove keepalive according to request count and timer.
        // Does nginx have this machanism?
        match &self.keepalive_config {
            Some(config) => {
                let cnt_str = headers.get(COUNTER_HEADER_NAME).unwrap();
                let counter: usize = cnt_str.to_str().unwrap().parse().unwrap();
                if counter >= config.keepalive_requests {
                    return true;
                }

                let time_str = headers
                    .get(HeaderName::from_static(TIMER_HEADER_NAME))
                    .unwrap();
                let elapsed_time: u64 = time_str.to_str().unwrap().parse().unwrap();
                elapsed_time >= config.keepalive_time
            }
            None => true,
        }
    }
}

impl<H> Service<Request<Payload>> for ConnReuseHandler<H>
where
    H: HttpHandler,
{
    type Response = Response<Payload>;
    type Error = H::Error;
    type Future<'a> = impl Future<Output = Result<Response<Payload>, Self::Error>> + 'a
    where
        Self: 'a;

    fn call(&self, mut request: Request<Payload>) -> Self::Future<'_> {
        async move {
            let version = request.version();
            let reuse_conn = is_conn_reuse(request.headers(), version);
            let should_close = self.should_close_conn(request.headers());
            debug!("frontend conn reuse {:?}", reuse_conn);
            // update the http request to make sure the backend conn reuse
            *request.version_mut() = Version::HTTP_11;
            let _ = request.headers_mut().remove(http::header::CONNECTION);

            match self.inner.handle(request).await {
                Ok(mut response) => {
                    let header_value = if reuse_conn && !should_close {
                        unsafe { HeaderValue::from_maybe_shared_unchecked(CONN_KEEP_ALIVE) }
                    } else {
                        unsafe { HeaderValue::from_maybe_shared_unchecked(CONN_CLOSE) }
                    };
                    response
                        .headers_mut()
                        .insert(http::header::CONNECTION, header_value);
                    *response.version_mut() = version;
                    Ok(response)
                }
                // TODO(ihciah): Is it ok to return Ok even when Err?
                Err(_e) => Ok(generate_response(StatusCode::INTERNAL_SERVER_ERROR)),
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
            keepalive_config: self.keepalive_config,
        })
    }
}

impl<F> ConnReuseHandler<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Option<KeepaliveConfig>>,
    {
        layer_fn::<C, _, _, _>(|c, inner| Self {
            keepalive_config: c.param(),
            inner,
        })
    }
}
