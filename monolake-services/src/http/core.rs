use std::{convert::Infallible, future::Future, pin::Pin, time::Duration};

use bytes::Bytes;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use http::StatusCode;
use monoio::io::{sink::SinkExt, stream::Stream, AsyncReadRent, AsyncWriteRent, Split, Splitable};
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
    context::PeerAddr,
    http::{HttpAccept, HttpError, HttpHandler},
};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, ParamRef, Service,
};
use tracing::{error, info, warn};

use super::{generate_response, util::AccompanyPair};
use crate::tcp::Accept;

#[derive(Clone)]
pub struct HttpCoreService<H> {
    handler_chain: H,
    keepalive_timeout: Duration,
}

impl<H> HttpCoreService<H> {
    pub fn new(handler_chain: H, keepalive_config: Keepalive) -> Self {
        HttpCoreService {
            handler_chain,
            keepalive_timeout: keepalive_config.0,
        }
    }

    async fn h1_svc<S, CX>(&self, stream: S, ctx: CX)
    where
        S: Split + AsyncReadRent + AsyncWriteRent,
        H: HttpHandler<CX>,
        H::Error: Into<HttpError>,
        CX: ParamRef<PeerAddr> + Clone,
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
                        ParamRef::<PeerAddr>::param_ref(&ctx),
                    );
                    break;
                }
                Err(_) => {
                    // timeout
                    info!(
                        "Connection {:?} keepalive timed out",
                        ParamRef::<PeerAddr>::param_ref(&ctx),
                    );
                    break;
                }
            };

            // Check if we should keepalive

            // handle request and reply response
            // 1. do these things simultaneously: read body and send + handle request
            let mut acc_fut = AccompanyPair::new(
                self.handler_chain.handle(req, ctx.clone()),
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
                        error!("Error processing H1 fixed body {e:?}");
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
                            error!("Error processing H1 chunked body {e:?}");
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

    async fn h2_svc<S, CX>(&self, stream: S, ctx: CX)
    where
        S: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
        H: HttpHandler<CX>,
        H::Error: Into<HttpError>,
        CX: ParamRef<PeerAddr> + Clone,
    {
        let mut connection = match monoio_http::h2::server::Builder::new()
            .initial_window_size(1_000_000)
            .max_concurrent_streams(1000)
            .handshake::<S, Bytes>(stream)
            .await
        {
            Ok(c) => {
                info!(
                    "H2 handshake complete for {:?}",
                    ParamRef::<PeerAddr>::param_ref(&ctx),
                );
                c
            }
            Err(e) => {
                error!("h2 server build failed: {e:?}");
                return;
            }
        };

        let (tx, mut rx) = local_sync::mpsc::unbounded::channel();
        let mut backend_resp_stream = FuturesUnordered::new();
        let mut frontend_resp_stream = FuturesUnordered::new();

        monoio::spawn(async move {
            let tx = tx.clone();
            while let Some(result) = connection.accept().await {
                match tx.send(result) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Frontend Req send failed {e:?}");
                        break;
                    }
                }
            }
        });

        loop {
            let ctx = ctx.clone();
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
                                (self.handler_chain.handle(request, ctx).await, response_handle)
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

impl<H, Stream, CX> Service<HttpAccept<Stream, CX>> for HttpCoreService<H>
where
    Stream: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
    H: HttpHandler<CX>,
    H::Error: Into<HttpError>,
    CX: ParamRef<PeerAddr> + Clone,
{
    type Response = ();
    type Error = Infallible;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a, Accept<Stream, CX>: 'a;

    fn call(&self, incoming_stream: HttpAccept<Stream, CX>) -> Self::Future<'_> {
        async move {
            let (use_h2, stream, ctx) = incoming_stream;
            if use_h2 {
                self.h2_svc(stream, ctx).await
            } else {
                self.h1_svc(stream, ctx).await
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

#[derive(Debug, Copy, Clone)]
pub struct Keepalive(pub Duration);

impl Default for Keepalive {
    fn default() -> Self {
        const DEFAULT_KEEPALIVE_SEC: u64 = 75;
        Self(Duration::from_secs(DEFAULT_KEEPALIVE_SEC))
    }
}

impl<F> HttpCoreService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Keepalive>,
    {
        layer_fn(|c: &C, inner| Self::new(inner, c.param()))
    }
}
