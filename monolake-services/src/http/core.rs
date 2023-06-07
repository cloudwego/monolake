use futures::future::FutureExt;
use std::convert::Infallible;
use std::future::poll_fn;
use std::time::{Duration, Instant};
use std::{fmt::Debug, future::Future};

use super::generate_response;
use crate::http::util::MaybeDoubleFuture;
use crate::http::{COUNTER_HEADER_NAME, TIMER_HEADER_NAME};
use crate::{common::Accept, http::is_conn_reuse};
use async_channel::Receiver;
use bytes::Bytes;
use http::{HeaderName, HeaderValue, Request, Response, StatusCode, Version};
use monoio::io::{
    sink::{Sink, SinkExt},
    stream::Stream,
    AsyncReadRent, AsyncReadRentExt, AsyncWriteRent, Split, Splitable,
};
use monoio_http::h1::{
    codec::{decoder::RequestDecoder, encoder::GenericEncoder},
    payload::Payload,
};
use monoio_http::h2::server::{Builder, SendResponse};
use monolake_core::{
    config::{KeepaliveConfig, DEFAULT_TIMEOUT},
    http::{HttpAccept, HttpError, HttpHandler},
};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};
use tracing::{debug, info, warn};

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
    async fn handle(&self, request: Request<Payload>) -> anyhow::Result<Response<Payload>> {
        self.handler_chain.handle(request).await.map_err(Into::into)
    }

    async fn h1_close_conn<O>(&self, encoder: &mut GenericEncoder<O>)
    where
        O: AsyncWriteRent,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        let _ = encoder.close().await;
    }

    async fn h1_send_error<O>(&self, encoder: &mut GenericEncoder<O>, status: StatusCode)
    where
        O: AsyncWriteRent,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        let _ = encoder.send_and_flush(generate_response(status)).await;

        let _ = self.h1_close_conn(encoder).await;
    }

    async fn h1_process_response<O>(
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
                    self.h1_close_conn(encoder).await;
                }
            }
            _ = rx.recv() => {
                self.h1_send_error(encoder, StatusCode::INTERNAL_SERVER_ERROR).await;
            }
        };
    }

    async fn h1_process_request<O>(
        &self,
        request: Request<Payload>,
        encoder: &mut GenericEncoder<O>,
        rx: Receiver<()>,
    ) where
        O: AsyncWriteRent,
        GenericEncoder<O>: monoio::io::sink::Sink<Response<Payload>>,
    {
        match self.handle(request).await {
            Ok(response) => self.h1_process_response(response, encoder, rx).await,
            Err(e) => {
                debug!("send request with error:  {:?}", e);
                self.h1_send_error(encoder, StatusCode::INTERNAL_SERVER_ERROR)
                    .await;
            }
        }
    }

    async fn h1_svc_call<S>(&self, stream: S) -> Result<(), HttpError>
    where
        S: Split + AsyncReadRent + AsyncWriteRent + 'static,
    {
        let (tx, rx) = async_channel::bounded(1);
        let (reader, writer) = stream.into_split();
        let mut decoder = RequestDecoder::new(reader);
        let mut encoder = GenericEncoder::new(writer);

        let mut counter: usize = 0;
        let starting_time = Instant::now();
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
                    let processing = self.h1_process_request(request, &mut encoder, rx.clone());
                    maybe_processing = Some(processing);
                }
                Ok(Some(Err(err))) => {
                    warn!("{}", err);
                    break;
                }
                _ => {
                    info!("Connection timed out");
                    break;
                }
            }
        }

        // notify disconnect from endpoints
        rx.close();
        let _ = tx.send(()).await;
        Ok(())
    }

    async fn h2_send_error(&self, mut response_handle: SendResponse<Bytes>, status: StatusCode) {
        let (parts, _) = generate_response(status).into_parts();
        let response = http::Response::from_parts(parts, ());

        let _ = response_handle.send_response(response, true);
    }

    async fn h2_process_response(
        &self,
        response: Response<Payload>,
        mut response_handle: SendResponse<Bytes>,
        rx: Receiver<()>,
    ) {
        let (mut parts, payload) = response.into_parts();
        parts.headers.remove("connection");
        let response = http::Response::from_parts(parts, ());

        monoio::select! {
            _ = async {} => {
                // Don't bother doing any further processing if client has closed
                // this particular stream
                let stream_reset = poll_fn(|cx| response_handle.poll_reset(cx));
                if stream_reset.now_or_never().is_some() {
                    debug!("Stream reset by client");
                    return;
                }
                match payload {
                    Payload::None => {
                        let _ = response_handle.send_response(response, true);
                    }
                    Payload::Fixed(p) => {
                        let mut send_stream = match response_handle.send_response(response, false) {
                            Ok(send_stream) => send_stream,
                            Err(_) => { return; }
                        };

                        let data = p.get().await.unwrap();
                        send_stream.send_data(data, true).expect("H2 resp body streaming failed");
                    }
                    Payload::Stream(mut p) => {
                        let mut send_stream = match response_handle.send_response(response, false) {
                            Ok(send_stream) => send_stream,
                            Err(_) => { return; }
                        };

                        while let Some(data_result) = p.next().await {
                            let data = data_result.unwrap();
                            send_stream.send_data(data, false).expect("H2 resp body streaming failed");
                        }
                        send_stream.send_data(Bytes::new(), true).expect("H2 resp body streaming failed");
                    }
                    Payload::H2BodyStream(_) => {
                        // H2 client to be implemented
                        unreachable!()
                    }
                }
            }
            _ = rx.recv() => {
                self.h2_send_error(response_handle, StatusCode::INTERNAL_SERVER_ERROR).await;
            }
        };
    }

    async fn h2_process_request(
        &self,
        request: Request<Payload>,
        response_handle: SendResponse<Bytes>,
        rx: Receiver<()>,
    ) {
        match self.handle(request).await {
            Ok(response) => {
                self.h2_process_response(response, response_handle, rx)
                    .await;
            }
            Err(e) => {
                debug!("send request with error:  {:?}", e);
                self.h2_send_error(response_handle, StatusCode::INTERNAL_SERVER_ERROR)
                    .await;
            }
        }
    }

    async fn h2_svc_call<S>(&self, stream: S) -> Result<(), HttpError>
    where
        S: AsyncReadRent + AsyncWriteRent + Unpin + 'static,
    {
        let (tx, rx) = async_channel::bounded(1);
        let mut connection = Builder::new()
            .initial_window_size(1_000_000)
            .max_concurrent_streams(1000)
            .handshake(stream)
            .await?;

        info!("H2 handshake complete ");

        let mut counter: usize = 0;
        let starting_time = Instant::now();
        let mut maybe_processing = None;

        loop {
            counter += 1;
            let next_future = MaybeDoubleFuture::new(connection.accept(), maybe_processing);

            match monoio::time::timeout(self.timeout, next_future).await {
                Ok(Some(Ok((mut request, response_handle)))) => {
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
                    let (parts, body_stream) = request.into_parts();
                    let request = http::Request::from_parts(
                        parts,
                        monoio_http::h1::payload::Payload::H2BodyStream(body_stream),
                    );
                    let processing = self.h2_process_request(request, response_handle, rx.clone());
                    maybe_processing = Some(processing);
                }
                Ok(Some(Err(err))) => {
                    warn!("{}", err);
                    break;
                }
                _ => {
                    info!("Connection timed out");
                    break;
                }
            }
        }

        rx.close();
        let _ = tx.send(()).await;

        Ok(())
    }
}

impl<H, Stream, SocketAddr> Service<HttpAccept<Stream, SocketAddr>> for HttpCoreService<H>
where
    Stream: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
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
    fn call(&self, incoming_stream: HttpAccept<Stream, SocketAddr>) -> Self::Future<'_> {
        let (version, stream, _addr) = incoming_stream;
        async move {
            match version {
                Version::HTTP_11 => {
                    let _ = self.h1_svc_call(stream).await;
                }
                Version::HTTP_2 => {
                    let _ = self.h2_svc_call(stream).await;
                }
                _ => {
                    unreachable!()
                }
            };
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
        layer_fn(|c: &C, inner| Self::new(inner, c.param()))
    }
}

#[derive(Clone)]
pub struct HttpVersionDetect<T> {
    inner: T,
}

impl<F> MakeService for HttpVersionDetect<F>
where
    F: MakeService,
{
    type Service = HttpVersionDetect<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(HttpVersionDetect {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
        })
    }
}

impl<F> HttpVersionDetect<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<()>,
    {
        layer_fn(|_c: &C, inner| HttpVersionDetect { inner })
    }
}

impl<T, Stream, SocketAddr> Service<Accept<Stream, SocketAddr>> for HttpVersionDetect<T>
where
    Stream: AsyncReadRent + AsyncWriteRent + 'static,
    SocketAddr: 'static,
    T: Service<HttpAccept<Stream, SocketAddr>>,
{
    type Response = ();

    type Error = HttpError;

    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a;

    fn call(&self, incoming_stream: Accept<Stream, SocketAddr>) -> Self::Future<'_> {
        let buf = vec![0_u8; 24];

        async move {
            let (mut stream, addr) = incoming_stream;
            let preface: [u8; 24] = *b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

            let (sz, buf) = stream.read_exact(buf).await;
            let sz = sz.unwrap();

            if sz != preface.len() {
                panic!()
            }

            let version = if buf[..] == preface[..] {
                http::Version::HTTP_2
            } else {
                http::Version::HTTP_11
            };

            let preface_buf = std::io::Cursor::new(buf);
            let rewind_io = monoio::io::PrefixedReadIo::new(stream, preface_buf);

            let _ = self.inner.call((version, rewind_io, addr)).await;

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{convert::Infallible, future::Future};

    use http::{HeaderValue, Request, Response};
    use monoio_http::h1::payload::Payload;
    use monolake_core::http::HttpHandler;
    use service_async::Service;
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
