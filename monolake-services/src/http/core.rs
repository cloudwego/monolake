use std::convert::Infallible;
use std::time::{Duration, Instant};
use std::{fmt::Debug, future::Future, rc::Rc};

use crate::http::util::MaybeDoubleFuture;
use crate::http::{COUNTER_HEADER_NAME, TIMER_HEADER_NAME};
use crate::{common::Accept, http::is_conn_reuse};
use async_channel::Receiver;
use http::{HeaderName, HeaderValue, Request, Response, StatusCode};
use monoio::io::{
    sink::{Sink, SinkExt},
    stream::Stream,
    AsyncReadRent, AsyncWriteRent, Split, Splitable,
};
use monoio_http::h1::{
    codec::{decoder::RequestDecoder, encoder::GenericEncoder},
    payload::Payload,
};
use monolake_core::http::HttpError;
use monolake_core::service::layer::{layer_fn, FactoryLayer};
use monolake_core::service::{MakeService, Param};
use monolake_core::{
    config::{KeepaliveConfig, DEFAULT_TIMEOUT},
    http::HttpHandler,
    service::Service,
};
use tracing::{debug, info, warn};

use super::generate_response;

#[derive(Clone)]
pub struct HttpCoreService<H> {
    handler_chain: H,
    timeout: Duration,
}

impl<H> HttpCoreService<H> {
    pub fn new(handler_chain: H, keepalive_config: Option<KeepaliveConfig>) -> Self {
        let timeout = match keepalive_config {
            Some(config) => Duration::from_secs(config.keepalive_timeout as u64),
            None => Duration::from_secs(DEFAULT_TIMEOUT as u64),
        };
        HttpCoreService {
            handler_chain,
            timeout,
        }
    }
}

impl<H> HttpCoreService<H>
where
    H: HttpHandler,
    H::Error: Into<HttpError>,
{
    #[inline]
    async fn handle(&self, request: Request<Payload>) -> anyhow::Result<Response<Payload>> {
        self.handler_chain.handle(request).await.map_err(Into::into)
    }

    #[inline]
    async fn close_conn<O>(&self, encoder: &mut GenericEncoder<O>)
    where
        O: AsyncWriteRent,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        let _ = encoder.close().await;
    }

    #[inline]
    async fn send_error<O>(&self, encoder: &mut GenericEncoder<O>, status: StatusCode)
    where
        O: AsyncWriteRent,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        let _ = encoder.send_and_flush(generate_response(status)).await;

        let _ = self.close_conn(encoder).await;
    }

    #[inline]
    async fn process_response<O>(
        &self,
        response: Response<Payload>,
        encoder: &mut GenericEncoder<O>,
        rx: Receiver<()>,
    ) where
        O: AsyncWriteRent,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        let should_close_conn = !is_conn_reuse(response.headers(), response.version());

        monoio::select! {
            _ = encoder.send_and_flush(response) => {
                if should_close_conn {
                    self.close_conn(encoder).await;
                }
            }
            _ = rx.recv() => {
                self.send_error(encoder, StatusCode::INTERNAL_SERVER_ERROR).await;
            }
        };
    }

    #[inline]
    async fn process_request<O>(
        &self,
        request: Request<Payload>,
        encoder: &mut GenericEncoder<O>,
        rx: Receiver<()>,
    ) where
        O: AsyncWriteRent,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        match self.handle(request).await {
            Ok(response) => self.process_response(response, encoder, rx).await,
            Err(e) => {
                debug!("send request with error:  {:?}", e);
                self.send_error(encoder, StatusCode::INTERNAL_SERVER_ERROR)
                    .await;
            }
        }
    }
}

impl<H, Stream, SocketAddr> Service<Accept<Stream, SocketAddr>> for HttpCoreService<H>
where
    Stream: Split + AsyncReadRent + AsyncWriteRent,
    SocketAddr: Debug,
    H: HttpHandler,
    H::Error: Into<HttpError>,
{
    type Response = ();
    type Error = Infallible;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a, Accept<Stream, SocketAddr>: 'a;

    // TODO(ihciah): remove counter and timer
    fn call(&self, incoming_stream: Accept<Stream, SocketAddr>) -> Self::Future<'_> {
        let (stream, addr) = incoming_stream;
        let (reader, writer) = stream.into_split();
        let service = Rc::new(self.to_owned());
        let (tx, rx) = async_channel::bounded(1);
        let mut decoder = RequestDecoder::new(reader);
        let mut encoder = GenericEncoder::new(writer);

        let mut counter: usize = 0;
        let starting_time = Instant::now();

        async move {
            let mut maybe_processing = None;
            loop {
                counter += 1;

                let next_future = MaybeDoubleFuture::new(decoder.next(), maybe_processing);

                // Pending refactor due to timeout function have double meaning:
                // 1) keepalive idle conn timeout. 2) accept request timeout.
                match monoio::time::timeout(self.timeout, next_future).await {
                    Ok(Some(Ok(mut request))) => {
                        let counter_header_value =
                            HeaderValue::from_bytes(counter.to_string().as_bytes()).unwrap();
                        request
                            .headers_mut()
                            .insert(COUNTER_HEADER_NAME, counter_header_value);
                        let elapsed_time: u64 = (Instant::now() - starting_time).as_secs();
                        let timer_header_value =
                            HeaderValue::from_str(&format!("{}", elapsed_time)).unwrap();
                        request.headers_mut().insert(
                            HeaderName::from_static(TIMER_HEADER_NAME),
                            timer_header_value,
                        );
                        let processing = service.process_request(request, &mut encoder, rx.clone());
                        maybe_processing = Some(processing);
                    }
                    Ok(Some(Err(err))) => {
                        warn!("{}", err);
                        break;
                    }
                    _ => {
                        info!("Connection {:?} timed out", addr);
                        break;
                    }
                }
            }
            info!("http client {:?} closed", addr);
            // notify disconnect from endpoints
            rx.close();
            let _ = tx.send(()).await;
            Ok(())
        }
    }
}

// HttpCoreService is a Service and a MakeService.
impl<F> MakeService for HttpCoreService<F>
where
    F: MakeService,
{
    type Service = HttpCoreService<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(HttpCoreService {
            handler_chain: self
                .handler_chain
                .make_via_ref(old.map(|o| &o.handler_chain))?,
            timeout: self.timeout,
        })
    }
}

impl<F> HttpCoreService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Option<KeepaliveConfig>>,
    {
        layer_fn::<C, _, _, _>(|c, inner| Self::new(inner, c.param()))
    }
}

#[cfg(test)]
mod tests {
    use std::{convert::Infallible, future::Future};

    use http::{HeaderValue, Request, Response};
    use monoio_http::h1::payload::Payload;
    use monolake_core::{http::HttpHandler, service::Service};
    use tower_layer::{layer_fn, Layer};

    use crate::http::core::HttpCoreService;

    struct IntermediateHttpHandler1<H> {
        inner: H,
    }
    impl<H> Service<Request<Payload>> for IntermediateHttpHandler1<H>
    where
        H: HttpHandler,
    {
        type Response = Response<Payload>;
        type Error = H::Error;
        type Future<'a> = impl Future<Output = Result<Response<Payload>, Self::Error>> + 'a
        where
            Self: 'a;

        fn call(&self, mut req: Request<Payload>) -> Self::Future<'_> {
            async move {
                let headers = req.headers_mut();
                headers.append("IntermediateHttpHandler1", HeaderValue::from_static("Ok"));
                let mut res = self.inner.handle(req).await?;
                let headers = res.headers_mut();
                headers.append("IntermediateHttpHandler1", HeaderValue::from_static("Ok"));
                Ok(res)
            }
        }
    }

    impl<H> IntermediateHttpHandler1<H> {
        fn layer() -> impl Layer<H, Service = IntermediateHttpHandler1<H>> {
            layer_fn(move |inner| IntermediateHttpHandler1 { inner })
        }
    }

    struct IntermediateHttpHandler2<H> {
        inner: H,
    }
    impl<H> Service<Request<Payload>> for IntermediateHttpHandler2<H>
    where
        H: HttpHandler,
    {
        type Response = Response<Payload>;
        type Error = H::Error;
        type Future<'a> = impl Future<Output = Result<Response<Payload>, Self::Error>> + 'a
        where
            Self: 'a;

        fn call(&self, req: Request<Payload>) -> Self::Future<'_> {
            async move {
                let mut res = self.inner.handle(req).await?;
                let headers = res.headers_mut();
                headers.append("IntermediateHttpHandler2", HeaderValue::from_static("Ok"));
                Ok(res)
            }
        }
    }

    impl<H> IntermediateHttpHandler2<H> {
        fn layer() -> impl Layer<H, Service = IntermediateHttpHandler2<H>> {
            layer_fn(move |inner| IntermediateHttpHandler2 { inner })
        }
    }

    struct LeafHttpHandler;
    impl Service<Request<Payload>> for LeafHttpHandler {
        type Response = Response<Payload>;
        type Error = Infallible;
        type Future<'a> = impl Future<Output = Result<Response<Payload>, Self::Error>> + 'a
        where
            Self: 'a;

        fn call(&self, _req: Request<Payload>) -> Self::Future<'_> {
            async move { Ok(Response::builder().status(200).body(Payload::None).unwrap()) }
        }
    }

    #[monoio::test]
    async fn test_handler_chains() {
        let handler = (
            IntermediateHttpHandler1::layer(),
            IntermediateHttpHandler2::layer(),
        )
            .layer(LeafHttpHandler);
        let service = HttpCoreService::new(handler, None);
        let request = Request::builder()
            .method("GET")
            .uri("https://www.rust-lang.org/")
            .header("X-Custom-Foo", "Bar")
            .body(Payload::None)
            .unwrap();
        let response = service.handle(request).await.unwrap();
        let headers = response.headers();
        assert_eq!(200, response.status());
        assert!(headers.contains_key("IntermediateHttpHandler1"));
        assert!(headers.contains_key("IntermediateHttpHandler2"));
    }
}
