use std::future::Future;

use http::{HeaderMap, HeaderValue, Response, StatusCode, Version};
use tracing::debug;
use monoio_http::h1::payload::Payload;
use monolake_core::{
    config::KeepaliveConfig,
    http::{HttpError, HttpHandler},
};
use tower_layer::{layer_fn, Layer};

use crate::http::{generate_response, is_conn_reuse, CONN_CLOSE, CONN_KEEP_ALIVE, COUNTER_HEADER_NAME, TIMER_HEADER_NAME};

#[derive(Clone)]
pub struct ConnReuseHandler<H> {
    inner: H,
    keepalive_config: Option<KeepaliveConfig>,
}

impl<H> ConnReuseHandler<H> {
    pub fn layer(keepalive_config: Option<KeepaliveConfig>) -> impl Layer<H, Service = ConnReuseHandler<H>> {
        layer_fn(move |inner| ConnReuseHandler {
            inner,
            keepalive_config: keepalive_config.clone(),
        })
    }

    fn should_close_conn(&self, headers: &HeaderMap<HeaderValue>) -> bool {
        match &self.keepalive_config {
            Some(config) => {
                let cnt_str = headers.get(COUNTER_HEADER_NAME).unwrap();
                let counter: usize = cnt_str.to_str().unwrap().parse().unwrap();
                if counter >= config.keepalive_requests {
                    return true;
                }

                let time_str = headers.get(TIMER_HEADER_NAME).unwrap();
                let elapsed_time : u64 = time_str.to_str().unwrap().parse().unwrap();
                if elapsed_time >= config.keepalive_time as u64 {
                    return true;
                }

                return false;
            },
            None => {
                return true;
            }
        }
    }
}

impl<H> HttpHandler for ConnReuseHandler<H>
where
    H: HttpHandler<Body = Payload> + 'static,
{
    type Body = Payload;
    type Future<'a> = impl Future<Output = Result<Response<Self::Body>, HttpError>> + 'a;
    fn handle(&self, mut request: http::Request<Self::Body>) -> Self::Future<'_> {
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
                Err(_e) => Ok(generate_response(StatusCode::INTERNAL_SERVER_ERROR)),
            }
        }
    }
}
