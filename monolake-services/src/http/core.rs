use std::time::{Duration, Instant};
use std::{cell::UnsafeCell, fmt::Debug, future::Future, rc::Rc};

use crate::http::{COUNTER_HEADER_NAME, TIMER_HEADER_NAME};
use crate::{common::Accept, http::is_conn_reuse};
use async_channel::Receiver;
use http::{HeaderValue, Request, Response, StatusCode};
use log::{debug, info, warn};
use monoio::io::{
    sink::{Sink, SinkExt},
    stream::Stream,
    AsyncReadRent, AsyncWriteRent, Split, Splitable,
};
use monoio_http::h1::{
    codec::{decoder::RequestDecoder, encoder::GenericEncoder},
    payload::Payload,
};
use monolake_core::{
    config::{KeepaliveConfig, DEFAULT_TIMEOUT},
    http::{HttpError, HttpHandler},
    service::Service,
};

use super::generate_response;

#[derive(Clone)]
pub struct HttpCoreService<H: Clone> {
    handler_chain: H,
    timeout: Duration,
}

impl<H> HttpCoreService<H>
where
    H: HttpHandler<Body = Payload> + 'static,
{
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

    #[inline]
    async fn handle(&self, request: Request<Payload>) -> anyhow::Result<Response<Payload>> {
        self.handler_chain.handle(request).await
    }

    #[inline]
    async fn close_conn<O>(&self, encoder: Rc<UnsafeCell<GenericEncoder<O>>>)
    where
        O: AsyncWriteRent + 'static,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        let _ = unsafe { &mut *encoder.get() }.close().await;
    }

    #[inline]
    async fn send_error<O>(&self, encoder: Rc<UnsafeCell<GenericEncoder<O>>>, status: StatusCode)
    where
        O: AsyncWriteRent + 'static,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        let _ = unsafe { &mut *encoder.get() }
            .send_and_flush(generate_response(status))
            .await;

        let _ = self.close_conn(encoder);
    }

    #[inline]
    async fn process_response<O>(
        self: Rc<Self>,
        response: Response<Payload>,
        encoder: Rc<UnsafeCell<GenericEncoder<O>>>,
        rx: Receiver<()>,
    ) where
        O: AsyncWriteRent + 'static,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        let should_close_conn = !is_conn_reuse(response.headers(), response.version());

        monoio::select! {
            _ = unsafe { &mut *encoder.get() }.send_and_flush(response) => {
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
        self: Rc<Self>,
        request: Request<Payload>,
        encoder: Rc<UnsafeCell<GenericEncoder<O>>>,
        rx: Receiver<()>,
    ) where
        O: AsyncWriteRent + 'static,
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
    Stream: Split + AsyncReadRent + AsyncWriteRent + 'static,
    H: HttpHandler<Body = Payload> + 'static,
    SocketAddr: Debug + 'static,
{
    type Response = ();

    type Error = HttpError;

    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a;

    fn call(&self, incoming_stream: Accept<Stream, SocketAddr>) -> Self::Future<'_> {
        let (stream, addr) = incoming_stream;
        let (reader, writer) = stream.into_split();
        let service = Rc::new(self.to_owned());
        let (tx, rx) = async_channel::bounded(1);
        let mut decoder = RequestDecoder::new(reader);
        let encoder = Rc::new(UnsafeCell::new(GenericEncoder::new(writer)));

        let mut counter: usize = 0;
        let starting_time = Instant::now();

        async move {
            loop {
                counter += 1;

                // Pending refactor due to timeout function have double meaning:
                // 1) keepalive idle conn timeout. 2) accept request timeout.
                match monoio::time::timeout(self.timeout, decoder.next()).await {
                    Ok(Some(Ok(mut request))) => {
                        let counter_header_value =
                            HeaderValue::from_bytes(counter.to_string().as_bytes()).unwrap();
                        request
                            .headers_mut()
                            .insert(COUNTER_HEADER_NAME, counter_header_value);
                        let elapsed_time: u64 = (Instant::now() - starting_time).as_secs();
                        let timer_header_value =
                            HeaderValue::from_str(&format!("{}", elapsed_time)).unwrap();
                        request
                            .headers_mut()
                            .insert(TIMER_HEADER_NAME, timer_header_value);
                        monoio::spawn(service.clone().process_request(
                            request,
                            encoder.clone(),
                            rx.clone(),
                        ));
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

#[cfg(test)]
mod tests {
    use std::future::Future;

    use http::{HeaderValue, Request, Response};
    use monoio_http::h1::payload::Payload;
    use monolake_core::http::{HttpError, HttpHandler};
    use tower_layer::{layer_fn, Layer};

    use crate::http::core::HttpCoreService;

    #[derive(Clone, Default)]
    struct IntermediateHttpHandler1<H> {
        inner: H,
    }
    impl<H> HttpHandler for IntermediateHttpHandler1<H>
    where
        H: HttpHandler<Body = Payload> + 'static,
    {
        type Body = Payload;
        type Future<'a> = impl Future<Output = Result<Response<Self::Body>, HttpError>> + 'a;
        fn handle(&self, mut req: http::Request<Self::Body>) -> Self::Future<'_> {
            async {
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

    #[derive(Clone, Default)]
    struct IntermediateHttpHandler2<H> {
        inner: H,
    }

    impl<H> HttpHandler for IntermediateHttpHandler2<H>
    where
        H: HttpHandler<Body = Payload> + 'static,
    {
        type Body = Payload;
        type Future<'a> = impl Future<Output = Result<Response<Self::Body>, HttpError>> + 'a;
        fn handle(&self, req: http::Request<Self::Body>) -> Self::Future<'_> {
            async {
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

    #[derive(Clone, Default)]
    struct LeafHttpHandler;

    impl HttpHandler for LeafHttpHandler {
        type Body = Payload;
        type Future<'a> = impl Future<Output = Result<Response<Self::Body>, HttpError>> + 'a;
        fn handle(&self, _req: http::Request<Self::Body>) -> Self::Future<'_> {
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
