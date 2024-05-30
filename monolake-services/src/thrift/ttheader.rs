use std::{fmt::Debug, time::Duration};

use monoio::io::{sink::SinkExt, stream::Stream, AsyncReadRent, AsyncWriteRent};
use monoio_codec::Framed;
use monoio_thrift::codec::ttheader::{RawPayloadCodec, TTHeaderPayloadCodec};
use monolake_core::{context::PeerAddr, thrift::ThriftHandler, AnyError};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, ParamRef, Service,
};
use tracing::{error, info, trace, warn};

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

    async fn svc<S, CX>(&self, stream: S, ctx: CX) -> Result<(), AnyError>
    where
        S: AsyncReadRent + AsyncWriteRent,
        H: ThriftHandler<CX>,
        H::Error: Into<AnyError> + Debug,
        CX: ParamRef<PeerAddr> + Clone,
    {
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
                        return Err(io_error.into());
                    }
                    Err(e) => {
                        info!(
                            "Connection {:?} keepalive timed out",
                            ParamRef::<PeerAddr>::param_ref(&ctx),
                        );
                        return Err(e.into());
                    }
                    _ => {}
                }
            }

            // decode request with message timeout
            let decoded = match self.thrift_timeout.message_timeout {
                Some(message_timeout) => {
                    match monoio::time::timeout(message_timeout, codec.next()).await {
                        Ok(x) => x,
                        Err(e) => {
                            info!(
                                "Connection {:?} message timed out",
                                ParamRef::<PeerAddr>::param_ref(&ctx),
                            );
                            return Err(e.into());
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
                    return Err(err.into());
                }
                None => {
                    // Connection closed normally.
                    trace!("Connection closed normally due to read EOF");
                    break;
                }
            };

            // handle request and reply response
            match self.handler_chain.handle(req, ctx.clone()).await {
                Ok(resp) => {
                    if let Err(e) = codec.send_and_flush(resp).await {
                        warn!("error when reply client: {e}");
                        return Err(e.into());
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
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }
}

impl<H, Stream, CX> Service<(Stream, CX)> for TtheaderCoreService<H>
where
    Stream: AsyncReadRent + AsyncWriteRent + Unpin + 'static,
    H: ThriftHandler<CX>,
    H::Error: Into<AnyError> + Debug,
    CX: ParamRef<PeerAddr> + Clone,
{
    type Response = ();
    type Error = AnyError;

    async fn call(&self, incoming_stream: (Stream, CX)) -> Result<Self::Response, Self::Error> {
        self.svc(incoming_stream.0, incoming_stream.1).await
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
