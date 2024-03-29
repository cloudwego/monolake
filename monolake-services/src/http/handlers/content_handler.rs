use std::fmt::Debug;

use http::{Request, StatusCode};
use monoio_http::common::{
    body::{BodyEncodeExt, FixedBody},
    response::Response,
};
use monolake_core::http::{HttpHandler, ResponseWithContinue};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Service,
};

use crate::http::generate_response;

#[derive(Clone)]
pub struct ContentHandler<H> {
    inner: H,
}

impl<H, CX, B> Service<(Request<B>, CX)> for ContentHandler<H>
where
    H: HttpHandler<CX, B>,
    B: BodyEncodeExt + FixedBody,
    H::Body: BodyEncodeExt + FixedBody,
    B::EncodeDecodeError: Debug,
    <H::Body as BodyEncodeExt>::EncodeDecodeError: Debug,
{
    type Response = ResponseWithContinue<H::Body>;
    type Error = H::Error;

    async fn call(&self, (request, ctx): (Request<B>, CX)) -> Result<Self::Response, Self::Error> {
        let content_encoding = request
            .headers()
            .get(http::header::CONTENT_ENCODING)
            .and_then(|value: &http::HeaderValue| value.to_str().ok())
            .unwrap_or("identity")
            .to_string();

        let accept_encoding = request
            .headers()
            .get(http::header::ACCEPT_ENCODING)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("identity")
            .to_string();

        let content_length = request
            .headers()
            .get(http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.parse::<usize>().unwrap_or_default())
            .unwrap_or_default();

        if content_length == 0 || content_encoding == "identity" {
            let (response, _) = self.inner.handle(request, ctx).await?;
            return Ok((response, true));
        }

        let (parts, body) = request.into_parts();
        match body.decode_content(content_encoding).await {
            Ok(decodec_data) => {
                let req = Request::from_parts(parts, B::fixed_body(Some(decodec_data)));
                let (mut response, _) = self.inner.handle(req, ctx).await?;
                if accept_encoding != "identity" {
                    let (parts, body) = response.into_parts();
                    match body.encode_content(accept_encoding).await {
                        Ok(encoded_data) => {
                            response =
                                Response::from_parts(parts, H::Body::fixed_body(Some(encoded_data)))
                        }
                        Err(e) => {
                            tracing::error!("Response content encoding failed {e:?}");
                            return Ok((
                                generate_response(StatusCode::INTERNAL_SERVER_ERROR, false),
                                true,
                            ));
                        }
                    }
                }
                Ok((response, true))
            }
            Err(e) => {
                tracing::error!("Request content decode failed {e:?}");
                Ok((generate_response(StatusCode::BAD_REQUEST, false), true))
            }
        }
    }
}

// ContentHandler is a Service and a MakeService.
impl<F> MakeService for ContentHandler<F>
where
    F: MakeService,
{
    type Service = ContentHandler<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ContentHandler {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
        })
    }
}

impl<F: AsyncMakeService> AsyncMakeService for ContentHandler<F> {
    type Service = ContentHandler<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(ContentHandler {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner)).await?,
        })
    }
}

impl<F> ContentHandler<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_: &C, inner| Self { inner })
    }
}
