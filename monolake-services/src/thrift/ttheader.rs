//! Core Thrift THeader protocol service implementation for handling downstream client connections.
//!
//! This module provides a high-performance, asynchronous Thrift service that handles
//! connections from downstream clients using the THeader protocol. It is designed to work
//! with monoio's asynchronous runtime, providing fine-grained control over various timeouts.
//!
//! # Key Components
//!
//! - [`TtheaderCoreService`]: The main service component responsible for handling Thrift THeader
//!   connections from downstream clients. It can be composed of a stack of handlers implementing
//!   the [`ThriftHandler`] trait.
//! - [`ThriftServerTimeout`]: Configuration for various timeout settings in the Thrift server.
//!
//! # Features
//!
//! - Support for Thrift THeader protocol
//! - Composable design allowing a stack of [`ThriftHandler`] implementations
//! - Efficient handling of concurrent requests using asynchronous I/O
//! - Configurable timeout settings for different stages of request processing
//! - Automatic message framing and error handling
//!
//! # Usage
//!
//! [`TtheaderCoreService`] is typically used as part of a larger service stack. Here's a basic
//! example:
//!
//! ```ignore
//! use service_async::{layer::FactoryLayer, stack::FactoryStack};
//!
//! use crate::thrift::TtheaderCoreService;
//!
//! let config = Config { /* ... */ };
//! let proxy_config = Config { /* ... */ };
//! let stack = FactoryStack::new(config)
//!     .replace(TProxyHandler::factory(proxy_config))
//!     .push(TtheaderCoreService::layer());
//!
//! let service = stack.make_async().await.unwrap();
//! // Use the service to handle incoming Thrift THeader connections from downstream clients
//! ```
//!
//! # Handler Composition
//!
//! [`TtheaderCoreService`] can be composed of multiple handlers implementing the [`ThriftHandler`]
//! trait. This allows for a flexible and modular approach to request processing. Handlers can be
//! chained together to form a processing pipeline, each handling a specific aspect of the
//! Thrift request/response cycle.
//!
//! # Performance Considerations
//!
//! - Uses monoio's efficient async I/O operations for improved performance
//! - Implements connection keep-alive to reduce connection overhead
//! - Efficient message framing and decoding using the THeader protocol

use std::{convert::Infallible, fmt::Debug, time::Duration};

use certain_map::{Attach, Fork};
use monoio::io::{sink::SinkExt, stream::Stream, AsyncReadRent, AsyncWriteRent};
use monoio_codec::Framed;
use monoio_thrift::codec::ttheader::{RawPayloadCodec, TTHeaderPayloadCodec};
use monolake_core::{context::PeerAddr, thrift::ThriftHandler, AnyError};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, ParamRef, Service,
};
use tracing::{error, info, trace, warn};

/// Core Thrift service handler supporting the THeader protocol.
///
/// `TtheaderCoreService` is responsible for accepting Thrift connections, decoding requests,
/// routing them through a handler chain, and encoding responses. It supports the THeader
/// protocol for efficient message framing and metadata handling.
/// For implementation details and example usage, see the
/// [module level documentation](crate::thrift::ttheader).
#[derive(Clone)]
pub struct TtheaderCoreService<H> {
    handler_chain: H,
    thrift_timeout: ThriftServerTimeout,
}

impl<H> TtheaderCoreService<H> {
    pub fn new(handler_chain: H, thrift_timeout: ThriftServerTimeout) -> Self {
        TtheaderCoreService {
            handler_chain,
            thrift_timeout,
        }
    }
}

impl<H, Stream, CXIn, CXStore, CXState, ERR> Service<(Stream, CXIn)> for TtheaderCoreService<H>
where
    CXIn: ParamRef<PeerAddr> + Fork<Store = CXStore, State = CXState>,
    CXStore: 'static,
    for<'a> CXState: Attach<CXStore>,
    for<'a> H: ThriftHandler<<CXState as Attach<CXStore>>::Hdr<'a>, Error = ERR>,
    ERR: Into<AnyError> + Debug,
    Stream: AsyncReadRent + AsyncWriteRent + Unpin + 'static,
{
    type Response = ();
    type Error = Infallible;

    async fn call(&self, (stream, ctx): (Stream, CXIn)) -> Result<Self::Response, Self::Error> {
        let mut codec = Framed::new(stream, TTHeaderPayloadCodec::new(RawPayloadCodec::new()));
        loop {
            if let Some(keepalive_timeout) = self.thrift_timeout.keepalive_timeout {
                match monoio::time::timeout(keepalive_timeout, codec.peek_data()).await {
                    Ok(Ok([])) => {
                        // Connection closed normally.
                        info!("Connection closed due to keepalive timeout");
                        break;
                    }
                    Ok(Err(io_error)) => {
                        error!(
                            "Connection {:?} io error: {io_error}",
                            ParamRef::<PeerAddr>::param_ref(&ctx)
                        );
                        break;
                    }
                    Err(_) => {
                        info!(
                            "Connection {:?} keepalive timed out",
                            ParamRef::<PeerAddr>::param_ref(&ctx),
                        );
                        break;
                    }
                    _ => {}
                }
            }

            // decode request with message timeout
            let decoded = match self.thrift_timeout.message_timeout {
                Some(message_timeout) => {
                    match monoio::time::timeout(message_timeout, codec.next()).await {
                        Ok(x) => x,
                        Err(_) => {
                            info!(
                                "Connection {:?} message timed out",
                                ParamRef::<PeerAddr>::param_ref(&ctx),
                            );
                            break;
                        }
                    }
                }
                None => codec.next().await,
            };

            let req = match decoded {
                Some(Ok(req)) => req,
                Some(Err(err)) => {
                    // decode error
                    error!("decode thrift message failed: {err}");
                    break;
                }
                None => {
                    // Connection closed normally.
                    trace!("Connection closed normally due to read EOF");
                    break;
                }
            };

            // fork ctx
            let (mut store, state) = ctx.fork();
            let forked_ctx = unsafe { state.attach(&mut store) };

            // handle request and reply response
            match self.handler_chain.handle(req, forked_ctx).await {
                Ok(resp) => {
                    if let Err(e) = codec.send_and_flush(resp).await {
                        warn!("error when reply client: {e}");
                        break;
                    }
                    trace!("sent thrift response");
                }
                Err(e) => {
                    // something error when process request(not a biz error)
                    error!("error when processing request: {e:?}");
                    // todo: error resp
                    // if let Err(e) = encoder
                    // .send_and_flush(generate_response(StatusCode::INTERNAL_SERVER_ERROR, true))
                    // .await
                    // {
                    // warn!("error when reply client: {e}");
                    // }
                    break;
                }
            }
        }
        Ok(())
    }
}

// TtheaderCoreService is a Service and a MakeService.
impl<F> MakeService for TtheaderCoreService<F>
where
    F: MakeService,
{
    type Service = TtheaderCoreService<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(TtheaderCoreService {
            handler_chain: self
                .handler_chain
                .make_via_ref(old.map(|o| &o.handler_chain))?,
            thrift_timeout: self.thrift_timeout,
        })
    }
}

impl<F: AsyncMakeService> AsyncMakeService for TtheaderCoreService<F> {
    type Service = TtheaderCoreService<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(TtheaderCoreService {
            handler_chain: self
                .handler_chain
                .make_via_ref(old.map(|o| &o.handler_chain))
                .await?,
            thrift_timeout: self.thrift_timeout,
        })
    }
}

/// Configuration for Thrift server timeouts.
///
/// This struct allows setting timeouts for connection keepalive and message reading,
/// providing fine-grained control over the Thrift server's behavior.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ThriftServerTimeout {
    // Connection keepalive timeout: If no byte comes when decoder want next request, close the
    // connection. Link Nginx `keepalive_timeout`
    pub keepalive_timeout: Option<Duration>,
    // Read full thrift message.
    pub message_timeout: Option<Duration>,
}

impl<F> TtheaderCoreService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<ThriftServerTimeout>,
    {
        layer_fn(|c: &C, inner| Self::new(inner, c.param()))
    }
}
