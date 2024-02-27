use std::{convert::Infallible, fmt::Debug, pin::Pin, time::Duration};

use bytes::Bytes;
use futures::{stream::FuturesUnordered, StreamExt};
use http::StatusCode;
use monoio::io::{sink::SinkExt, stream::Stream, AsyncReadRent, AsyncWriteRent, Split, Splitable};
use monoio_http::{
    common::{
        body::{Body, HttpBody, StreamHint},
        response::Response,
    },
    h1::codec::{
        decoder::{FillPayload, RequestDecoder},
        encoder::GenericEncoder,
    },
    h2::server::SendResponse,
};
use monolake_core::{
    context::PeerAddr,
    http::{HttpAccept, HttpHandler},
    AnyError,
};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, ParamRef, Service,
};
use tracing::{error, info, warn};

use super::{generate_response, util::AccompanyPair};

#[derive(Clone)]
pub struct HttpCoreService<H> {
    handler_chain: H,
    keepalive_timeout: Duration,
    http_timeout: Option<Duration>,
}

impl<H> HttpCoreService<H> {
    pub fn new(handler_chain: H, keepalive_config: Keepalive, timeout_config: HttpTimeout) -> Self {
        HttpCoreService {
            handler_chain,
            keepalive_timeout: keepalive_config.0,
            http_timeout: timeout_config.0,
        }
    }

    async fn h1_svc<S, CX>(&self, stream: S, ctx: CX)
    where
        S: Split + AsyncReadRent + AsyncWriteRent,
        H: HttpHandler<CX>,
        H::Error: Into<AnyError> + Debug,
        CX: ParamRef<PeerAddr> + Clone,
    {
        let (reader, writer) = stream.into_split();
        let mut decoder = RequestDecoder::new(reader);
        let mut encoder = GenericEncoder::new(writer);

        loop {
            // decode request with keepalive timeout
            let req = match monoio::time::timeout(self.keepalive_timeout, decoder.next()).await {
                Ok(Some(Ok(req))) => HttpBody::request(req),
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
                    error!("error when processing request: {e:?}");
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
        response: Response<HttpBody>,
        mut response_handle: SendResponse<Bytes>,
    ) {
        let (mut parts, mut body) = response.into_parts();
        parts.headers.remove("connection");
        let response = http::Response::from_parts(parts, ());

        match body.stream_hint() {
            StreamHint::None => {
                if let Err(e) = response_handle.send_response(response, true) {
                    error!("H2 frontend response send fail {:?}", e);
                }
            }
            StreamHint::Fixed => {
                let mut send_stream = match response_handle.send_response(response, false) {
                    Ok(s) => s,
                    Err(e) => {
                        error!("H2 frontend response send fail {:?}", e);
                        return;
                    }
                };

                if let Some(Ok(data)) = body.next_data().await {
                    let _ = send_stream.send_data(data, true);
                }
            }
            StreamHint::Stream => {
                let mut send_stream = match response_handle.send_response(response, false) {
                    Ok(s) => s,
                    Err(e) => {
                        error!("H2 frontend response send fail {:?}", e);
                        return;
                    }
                };

                while let Some(Ok(data)) = body.next_data().await {
                    let _ = send_stream.send_data(data, false);
                }

                let _ = send_stream.send_data(Bytes::new(), true);
            }
        }
    }

    async fn h2_svc<S, CX>(&self, stream: S, ctx: CX)
    where
        S: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
        H: HttpHandler<CX>,
        H::Error: Into<AnyError> + Debug,
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
            monoio::select! {
                 Some(Ok((request, response_handle))) = rx.recv() => {
                         let request = HttpBody::request(request);
                         backend_resp_stream.push( async move {
                             (self.handler_chain.handle(request, ctx).await, response_handle)
                         });
                 }
                 Some(result) = backend_resp_stream.next() => {
                     match result {
                         (Ok((response, _)), response_handle) => {
                             frontend_resp_stream.push(Self::h2_process_response(response, response_handle));
                         }
                         (Err(e), mut response_handle) => {
                             error!("Handler chain returned error : {e:?}");
                             let (parts, _) = generate_response(StatusCode::INTERNAL_SERVER_ERROR, false).into_parts();
                             let response = http::Response::from_parts(parts, ());
                             let _ = response_handle.send_response(response, true);
                         }
                     }
                 }
                 Some(_) = frontend_resp_stream.next() => {
                 }
                  else => {
                     // No more futures to drive, break the loop
                     // and drop the service.
                     break;
                  }
            }
        }

        info!(
            "H2 connection processing complete for {:?}",
            ParamRef::<PeerAddr>::param_ref(&ctx)
        );
    }
}

impl<H, Stream, CX> Service<HttpAccept<Stream, CX>> for HttpCoreService<H>
where
    Stream: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
    H: HttpHandler<CX>,
    H::Error: Into<AnyError> + Debug,
    CX: ParamRef<PeerAddr> + Clone,
{
    type Response = ();
    type Error = Infallible;

    async fn call(
        &self,
        incoming_stream: HttpAccept<Stream, CX>,
    ) -> Result<Self::Response, Self::Error> {
        let (use_h2, stream, ctx) = incoming_stream;
        if use_h2 {
            self.h2_svc(stream, ctx).await
        } else {
            self.h1_svc(stream, ctx).await
        }
        Ok(())
    }
}

// HttpCoreService is a Service and a MakeService.
impl<F: MakeService> MakeService for HttpCoreService<F> {
    type Service = HttpCoreService<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(HttpCoreService {
            handler_chain: self
                .handler_chain
                .make_via_ref(old.map(|o| &o.handler_chain))?,
            keepalive_timeout: self.keepalive_timeout,
            http_timeout: self.http_timeout,
        })
    }
}

impl<F: AsyncMakeService> AsyncMakeService for HttpCoreService<F> {
    type Service = HttpCoreService<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(HttpCoreService {
            handler_chain: self
                .handler_chain
                .make_via_ref(old.map(|o| &o.handler_chain))
                .await?,
            keepalive_timeout: self.keepalive_timeout,
            http_timeout: self.http_timeout,
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

#[derive(Debug, Copy, Clone)]
pub struct HttpTimeout(pub Option<Duration>);

impl Default for HttpTimeout {
    fn default() -> Self {
        HttpTimeout(None)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Timeouts {
    pub keepalive: Keepalive,
    pub timeout: HttpTimeout,
}

impl Default for Timeouts {
    fn default() -> Self {
        const DEFAULT_KEEPALIVE_SEC: u64 = 75;
        Self {
            keepalive: Keepalive(Duration::from_secs(DEFAULT_KEEPALIVE_SEC)),
            timeout: HttpTimeout(None),
        }
    }
}

impl<F> HttpCoreService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Timeouts>,
    {
        layer_fn(|c: &C, inner| Self::new(inner, c.param().keepalive, c.param().timeout))
    }
}
