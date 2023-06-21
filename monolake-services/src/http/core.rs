use std::{convert::Infallible, future::Future, pin::Pin, time::Duration};

use bytes::Bytes;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use http::StatusCode;
use monoio::{
    buf::SliceMut,
    io::{sink::SinkExt, stream::Stream, AsyncReadRent, AsyncWriteRent, Split, Splitable},
};
use monoio_http::{
    common::response::Response,
    h1::{
        codec::{
            decoder::{FillPayload, RequestDecoder},
            encoder::GenericEncoder,
        },
        payload::Payload,
    },
    h2::server::SendResponse,
};
use monolake_core::{
    config::{KeepaliveConfig, DEFAULT_TIMEOUT},
    environments::{Environments, PEER_ADDR},
    http::{HttpAccept, HttpError, HttpHandler},
};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};
use tracing::{error, info, warn};

use super::{generate_response, util::AccompanyPair};
use crate::common::Accept;

const PREFACE: [u8; 24] = *b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

#[derive(Clone)]
pub struct HttpCoreService<H> {
    handler_chain: H,
    keepalive_timeout: Duration,
}

impl<H> HttpCoreService<H> {
    pub fn new(handler_chain: H, keepalive_config: Option<KeepaliveConfig>) -> Self {
        let timeout = match keepalive_config {
            Some(config) => Duration::from_secs(config.keepalive_timeout as u64),
            None => Duration::from_secs(DEFAULT_TIMEOUT as u64),
        };
        HttpCoreService {
            handler_chain,
            keepalive_timeout: timeout,
        }
    }

    async fn h1_svc<S>(&self, stream: S, environments: Environments)
    where
        S: Split + AsyncReadRent + AsyncWriteRent,
        H: HttpHandler,
        H::Error: Into<HttpError>,
    {
        let (reader, writer) = stream.into_split();
        let mut decoder = RequestDecoder::new(reader);
        let mut encoder = GenericEncoder::new(writer);

        loop {
            // decode request with keepalive timeout
            let req = match monoio::time::timeout(self.keepalive_timeout, decoder.next()).await {
                Ok(Some(Ok(req))) => req,
                Ok(Some(Err(err))) => {
                    // decode error
                    warn!("decode request header failed: {err}");
                    break;
                }
                Ok(None) => {
                    // EOF
                    info!(
                        "Connection {:?} closed",
                        environments.get(&PEER_ADDR.to_string())
                    );
                    break;
                }
                Err(_) => {
                    // timeout
                    info!(
                        "Connection {:?} keepalive timed out",
                        environments.get(&PEER_ADDR.to_string())
                    );
                    break;
                }
            };

            // Check if we should keepalive

            // handle request and reply response
            // 1. do these things simultaneously: read body and send + handle request
            let mut acc_fut = AccompanyPair::new(
                self.handler_chain.handle(req, environments.clone()),
                decoder.fill_payload(),
            );
            let res = unsafe { Pin::new_unchecked(&mut acc_fut) }.await;
            match res {
                Ok((resp, should_cont)) => {
                    // 2. do these things simultaneously: read body and send + handle response
                    let mut f = acc_fut.replace(encoder.send_and_flush(resp));
                    if let Err(e) = unsafe { Pin::new_unchecked(&mut f) }.await {
                        warn!("error when encode and write response: {e}");
                        break;
                    }
                    if !should_cont {
                        break;
                    }
                    if let Err(e) = f.into_accompany().await {
                        warn!("error when decode request body: {e}");
                        break;
                    }
                }
                Err(e) => {
                    // something error when process request(not a biz error)
                    error!("error when processing request: {}", e.into());
                    if let Err(e) = encoder
                        .send_and_flush(generate_response(StatusCode::INTERNAL_SERVER_ERROR, true))
                        .await
                    {
                        warn!("error when reply client: {e}");
                    }
                    break;
                }
            }
        }
    }

    async fn h2_process_response(
        response: Response<Payload>,
        mut response_handle: SendResponse<Bytes>,
    ) {
        let (mut parts, payload) = response.into_parts();
        parts.headers.remove("connection");
        let response = http::Response::from_parts(parts, ());

        match payload {
            Payload::None => {
                let _ = response_handle.send_response(response, true);
            }
            Payload::Fixed(p) => {
                let mut send_stream = match response_handle.send_response(response, false) {
                    Ok(send_stream) => send_stream,
                    Err(_) => {
                        return;
                    }
                };

                match p.get().await {
                    Ok(data) => {
                        let _ = send_stream.send_data(data, true);
                    }

                    Err(e) => {
                        error!("Error processing H1 fixed body {:?}", e);
                    }
                }
            }
            Payload::Stream(mut p) => {
                let mut send_stream = match response_handle.send_response(response, false) {
                    Ok(send_stream) => send_stream,
                    Err(_) => {
                        return;
                    }
                };

                while let Some(data_result) = p.next().await {
                    match data_result {
                        Ok(data) => {
                            let _ = send_stream.send_data(data, false);
                        }
                        Err(e) => {
                            error!("Error processing H1 chunked body {:?}", e);
                        }
                    }
                }
                let _ = send_stream.send_data(Bytes::new(), true);
            }
            Payload::H2BodyStream(_) => {
                // H2 client to be implemented
                unreachable!()
            }
        }
    }

    async fn h2_svc<S>(&self, stream: S, environments: Environments)
    where
        S: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
        H: HttpHandler,
        H::Error: Into<HttpError>,
    {
        let mut connection = monoio_http::h2::server::Builder::new()
            .initial_window_size(1_000_000)
            .max_concurrent_streams(1000)
            .handshake::<S, Bytes>(stream)
            .await
            .unwrap();

        info!(
            "H2 handshake complete for {:?}",
            environments.get(&PEER_ADDR.to_string())
        );

        let (tx, mut rx) = local_sync::mpsc::unbounded::channel();
        let mut backend_resp_stream = FuturesUnordered::new();
        let mut frontend_resp_stream = FuturesUnordered::new();

        monoio::spawn(async move {
            let tx = tx.clone();
            while let Some(result) = connection.accept().await {
                match tx.send(result) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Frontend Req send failed {:?}", e);
                        break;
                    }
                }
            }
        });

        loop {
            let environments = environments.clone();
            futures::select! {
                result = rx.recv().fuse() => {
                    match result {
                        Some(Ok((request, response_handle)))  => {
                            let (parts, body_stream) = request.into_parts();
                            let request = http::Request::from_parts(
                                parts,
                                monoio_http::h1::payload::Payload::H2BodyStream(body_stream),
                            );

                            backend_resp_stream.push( async move {
                                (self.handler_chain.handle(request, environments).await, response_handle)
                            });
                        },
                        Some(Err(e)) => {
                            error!("H2 connection error {:?}", e);
                            break;
                        },
                        None => {}
                    }
                },
                result = backend_resp_stream.next() => {
                    match result {
                        Some((Ok((response, _)), response_handle)) => {
                            frontend_resp_stream.push(Self::h2_process_response(response, response_handle));
                        }
                        Some((Err(e), mut response_handle)) => {
                            error!("Handler chain returned error : {}", e.into());
                            let (parts, _) = generate_response(StatusCode::INTERNAL_SERVER_ERROR, false).into_parts();
                            let response = http::Response::from_parts(parts, ());
                            let _ = response_handle.send_response(response, true);
                        }
                        None => {}
                    }
                },
                _ = frontend_resp_stream.next() => {},
                complete => {}
            }
        }
    }
}

impl<H, Stream> Service<HttpAccept<Stream, Environments>> for HttpCoreService<H>
where
    Stream: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
    H: HttpHandler,
    H::Error: Into<HttpError>,
{
    type Response = ();
    type Error = Infallible;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a, Accept<Stream, Environments>: 'a;

    fn call(&self, incoming_stream: HttpAccept<Stream, Environments>) -> Self::Future<'_> {
        let (is_h2, stream, environments) = incoming_stream;
        async move {
            match is_h2 {
                false => self.h1_svc(stream, environments).await,
                true => self.h2_svc(stream, environments).await,
            }
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
            keepalive_timeout: self.keepalive_timeout,
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

    type Error = std::io::Error;

    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a;

    fn call(&self, incoming_stream: Accept<Stream, SocketAddr>) -> Self::Future<'_> {
        async move {
            let (mut stream, addr) = incoming_stream;
            let mut buf = vec![0; 24];
            let mut pos = 0;
            let mut h2_detect = false;
            let len = buf.len();

            loop {
                let buf_slice = unsafe { SliceMut::new_unchecked(buf, pos, len) };
                let (result, buf_slice) = stream.read(buf_slice).await;
                buf = buf_slice.into_inner();
                match result {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        if PREFACE[pos..pos + n] != buf[pos..pos + n] {
                            break;
                        }
                        pos += n;
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }

                if pos == PREFACE.len() {
                    h2_detect = true;
                    break;
                }
            }

            let preface_buf = std::io::Cursor::new(buf);
            let rewind_io = monoio::io::PrefixedReadIo::new(stream, preface_buf);

            let _ = self.inner.call((h2_detect, rewind_io, addr)).await;

            Ok(())
        }
    }
}
