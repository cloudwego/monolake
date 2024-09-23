//! Core HTTP service implementation for handling downstream client connections.
//!
//! This module provides a high-performance, asynchronous HTTP service that handles
//! connections from downstream clients. It supports HTTP/1, HTTP/1.1, and HTTP/2 protocols,
//! and is designed to work with monoio's asynchronous runtime, providing fine-grained
//! control over various timeouts.
//!
//! # Key Components
//!
//! - [`HttpCoreService`]: The main service component responsible for handling HTTP connections from
//!   downstream clients. It can be composed of a stack of handlers implementing the `HttpHandler`
//!   trait.
//! - [`HttpServerTimeout`]: Configuration for various timeout settings in the HTTP server.
//!
//! # Features
//!
//! - Support for HTTP/1, HTTP/1.1, and HTTP/2 protocols
//! - Composable design allowing a stack of `HttpHandler` implementations
//! - Automatic protocol detection when combined with `HttpVersionDetect`
//! - Efficient handling of concurrent requests using asynchronous I/O
//! - Configurable timeout settings for different stages of request processing
//! - Integration with `service_async` for easy composition in service stacks
//! - Automatic response encoding and error handling
//!
//! # Usage
//!
//! `HttpCoreService` is typically used as part of a larger service stack, often in combination
//! with `HttpVersionDetect` for automatic protocol detection. Here's a basic example:
//!
//! ```ignore
//! use service_async::{layer::FactoryLayer, stack::FactoryStack};
//!
//! use crate::http::{HttpCoreService, HttpVersionDetect};
//!
//! let config = Config { /* ... */ };
//! let stack = FactoryStack::new(config)
//!     .push(HttpCoreService::layer())
//!     .push(HttpVersionDetect::layer())
//!     // ... other handlers implementing HttpHandler ...
//!     ;
//!
//! let service = stack.make_async().await.unwrap();
//! // Use the service to handle incoming HTTP connections from downstream clients
//! ```
//!
//! # Handler Composition
//!
//! `HttpCoreService` can be composed of multiple handlers implementing the `HttpHandler` trait.
//! This allows for a flexible and modular approach to request processing. Handlers can be
//! chained together to form a processing pipeline, each handling a specific aspect of the
//! HTTP request/response cycle.
//!
//! # Automatic Protocol Detection
//!
//! When used in conjunction with `HttpVersionDetect`, `HttpCoreService` can automatically
//! detect whether an incoming connection is using HTTP/1, HTTP/1.1, or HTTP/2, and handle
//! it appropriately. This allows for seamless support of multiple HTTP versions without
//! the need for separate server configurations.
//!
//! # Performance Considerations
//!
//! - Uses monoio's efficient async I/O operations for improved performance
//! - Implements connection keep-alive for HTTP/1.1 to reduce connection overhead
//! - Supports HTTP/2 multiplexing for efficient handling of concurrent requests
//! - Automatic protocol detection allows for optimized handling based on the client's capabilities
use std::{convert::Infallible, fmt::Debug, pin::Pin, time::Duration};

use bytes::Bytes;
use certain_map::{Attach, Fork};
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

/// Core HTTP service handler supporting both HTTP/1.1 and HTTP/2 protocols.
///
/// `HttpCoreService` is responsible for accepting HTTP connections, decoding requests,
/// routing them through a handler chain, and encoding responses. It supports both
/// HTTP/1.1 with keep-alive and HTTP/2 with multiplexing.
/// For implementation details and example usage, see the
/// [module level documentation](crate::http::core).
#[derive(Clone)]
pub struct HttpCoreService<H> {
    handler_chain: H,
    http_timeout: HttpServerTimeout,
}

impl<H> HttpCoreService<H> {
    pub fn new(handler_chain: H, http_timeout: HttpServerTimeout) -> Self {
        HttpCoreService {
            handler_chain,
            http_timeout,
        }
    }

    async fn h1_svc<S, CXIn, CXStore, CXState, Err>(&self, stream: S, ctx: CXIn)
    where
        CXIn: ParamRef<PeerAddr> + Fork<Store = CXStore, State = CXState>,
        CXStore: 'static,
        for<'a> CXState: Attach<CXStore>,
        for<'a> H: HttpHandler<
            <CXState as Attach<CXStore>>::Hdr<'a>,
            HttpBody,
            Body = HttpBody,
            Error = Err,
        >,
        Err: Into<AnyError> + Debug,
        S: Split + AsyncReadRent + AsyncWriteRent,
    {
        let (reader, writer) = stream.into_split();
        let mut decoder = RequestDecoder::new(reader);
        let mut encoder = GenericEncoder::new(writer);
        decoder.set_timeout(self.http_timeout.keepalive_timeout);

        loop {
            // decode request with header timeout
            let decoded = match self.http_timeout.read_header_timeout {
                Some(header_timeout) => {
                    match monoio::time::timeout(header_timeout, decoder.next()).await {
                        Ok(inner) => inner,
                        Err(_) => {
                            info!(
                                "Connection {:?} decode http header timed out",
                                ParamRef::<PeerAddr>::param_ref(&ctx),
                            );
                            break;
                        }
                    }
                }
                None => decoder.next().await,
            };

            let req = match decoded {
                Some(Ok(req)) => HttpBody::request(req),
                Some(Err(err)) => {
                    // decode error
                    warn!("decode request header failed: {err}");
                    break;
                }
                None => {
                    // EOF
                    info!(
                        "Connection {:?} closed",
                        ParamRef::<PeerAddr>::param_ref(&ctx),
                    );
                    break;
                }
            };

            // fork ctx
            let (mut store, state) = ctx.fork();
            let forked_ctx = unsafe { state.attach(&mut store) };

            // handle request and reply response
            // 1. do these things simultaneously: read body and send + handle request
            let mut acc_fut = AccompanyPair::new(
                self.handler_chain.handle(req, forked_ctx),
                decoder.fill_payload(),
            );
            let res = unsafe { Pin::new_unchecked(&mut acc_fut) }.await;
            match res {
                Ok((resp, should_cont)) => {
                    // 2. do these things simultaneously: read body and send + handle response
                    let mut f = acc_fut.replace(encoder.send_and_flush(resp));
                    match self.http_timeout.read_body_timeout {
                        None => {
                            if let Err(e) = unsafe { Pin::new_unchecked(&mut f) }.await {
                                warn!("error when encode and write response: {e}");
                                break;
                            }
                        }
                        Some(body_timeout) => {
                            match monoio::time::timeout(body_timeout, unsafe {
                                Pin::new_unchecked(&mut f)
                            })
                            .await
                            {
                                Err(_) => {
                                    info!(
                                        "Connection {:?} write timed out",
                                        ParamRef::<PeerAddr>::param_ref(&ctx),
                                    );
                                    break;
                                }
                                Ok(Err(e)) => {
                                    warn!("error when encode and write response: {e}");
                                    break;
                                }
                                _ => (),
                            }
                        }
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
                        .send_and_flush(generate_response::<HttpBody>(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            true,
                        ))
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

    async fn h2_svc<S, CXIn, CXStore, CXState, Err>(&self, stream: S, ctx: CXIn)
    where
        CXIn: ParamRef<PeerAddr> + Fork<Store = CXStore, State = CXState>,
        CXStore: 'static,
        for<'a> CXState: Attach<CXStore>,
        for<'a> H: HttpHandler<
            <CXState as Attach<CXStore>>::Hdr<'a>,
            HttpBody,
            Body = HttpBody,
            Error = Err,
        >,
        Err: Into<AnyError> + Debug,
        S: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
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
            monoio::select! {
                 Some(Ok((request, response_handle))) = rx.recv() => {
                        let request = HttpBody::request(request);
                        // fork ctx
                        let (mut store, state) = ctx.fork();
                        backend_resp_stream.push(async move {
                            let forked_ctx = unsafe { state.attach(&mut store) };
                            (self.handler_chain.handle(request, forked_ctx).await, response_handle)
                        });
                 }
                 Some(result) = backend_resp_stream.next() => {
                     match result {
                         (Ok((response, _)), response_handle) => {
                             frontend_resp_stream.push(Self::h2_process_response(response, response_handle));
                         }
                         (Err(e), mut response_handle) => {
                             error!("Handler chain returned error : {e:?}");
                             let (parts, _) = generate_response::<HttpBody>(StatusCode::INTERNAL_SERVER_ERROR, false).into_parts();
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

impl<H, Stream, CXIn, CXStore, CXState, Err> Service<HttpAccept<Stream, CXIn>>
    for HttpCoreService<H>
where
    CXIn: ParamRef<PeerAddr> + Fork<Store = CXStore, State = CXState>,
    CXStore: 'static,
    for<'a> CXState: Attach<CXStore>,
    for<'a> H:
        HttpHandler<<CXState as Attach<CXStore>>::Hdr<'a>, HttpBody, Body = HttpBody, Error = Err>,
    Stream: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
    Err: Into<AnyError> + Debug,
{
    type Response = ();
    type Error = Infallible;

    async fn call(
        &self,
        incoming_stream: HttpAccept<Stream, CXIn>,
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
            http_timeout: self.http_timeout,
        })
    }
}
/// Represents the timeout settings for the HTTP server.
///
/// The `HttpServerTimeout` struct contains three optional fields:
/// - `keepalive_timeout`: The timeout for keeping the connection alive. If no byte is received
///   within this timeout, the connection will be closed.
/// - `read_header_timeout`: The timeout for reading the full HTTP header.
/// - `read_body_timeout`: The timeout for receiving the full request body.
///
/// By default, the `keepalive_timeout` is set to 75 seconds, while the other two timeouts are not
/// set.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct HttpServerTimeout {
    pub keepalive_timeout: Option<Duration>,
    pub read_header_timeout: Option<Duration>,
    pub read_body_timeout: Option<Duration>,
}

impl Default for HttpServerTimeout {
    fn default() -> Self {
        const DEFAULT_KEEPALIVE_SEC: u64 = 75;
        Self {
            keepalive_timeout: Some(Duration::from_secs(DEFAULT_KEEPALIVE_SEC)),
            read_header_timeout: None,
            read_body_timeout: None,
        }
    }
}

impl<F> HttpCoreService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<HttpServerTimeout>,
    {
        layer_fn(|c: &C, inner| Self::new(inner, c.param()))
    }
}
