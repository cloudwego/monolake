use std::{convert::Infallible, fmt::Debug, time::Duration};

use monoio::io::{sink::SinkExt, stream::Stream, AsyncReadRent, AsyncWriteRent, Split, Splitable};
use monoio_codec::{FramedRead, FramedWrite};
use monoio_thrift::codec::ttheader::{
    RawPayloadCodec, TTHeaderPayloadDecoder, TTHeaderPayloadEncoder,
};
use monolake_core::{context::PeerAddr, thrift::ThriftHandler, AnyError};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, ParamRef, Service,
};
use tracing::{debug, error, info, warn};

use crate::http::Keepalive;

#[derive(Clone)]
pub struct TtheaderCoreService<H> {
    handler_chain: H,
    keepalive_timeout: Duration,
}

impl<H> TtheaderCoreService<H> {
    pub fn new(handler_chain: H, keepalive_config: Keepalive) -> Self {
        TtheaderCoreService {
            handler_chain,
            keepalive_timeout: keepalive_config.0,
        }
    }

    async fn svc<S, CX>(&self, stream: S, ctx: CX)
    where
        S: Split + AsyncReadRent + AsyncWriteRent,
        H: ThriftHandler<CX>,
        H::Error: Into<AnyError> + Debug,
        CX: ParamRef<PeerAddr> + Clone,
    {
        let (reader, writer) = stream.into_split();
        let mut decoder =
            FramedRead::new(reader, TTHeaderPayloadDecoder::new(RawPayloadCodec::new()));
        let mut encoder =
            FramedWrite::new(writer, TTHeaderPayloadEncoder::new(RawPayloadCodec::new()));

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
                    debug!(
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

            // handle request and reply response
            match self.handler_chain.handle(req, ctx.clone()).await {
                Ok(resp) => {
                    if let Err(e) = encoder.send_and_flush(resp).await {
                        warn!("error when reply client: {e}");
                        break;
                    }
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
    }
}

impl<H, Stream, CX> Service<(Stream, CX)> for TtheaderCoreService<H>
where
    Stream: Split + AsyncReadRent + AsyncWriteRent + Unpin + 'static,
    H: ThriftHandler<CX>,
    H::Error: Into<AnyError> + Debug,
    CX: ParamRef<PeerAddr> + Clone,
{
    type Response = ();
    type Error = Infallible;

    async fn call(&self, incoming_stream: (Stream, CX)) -> Result<Self::Response, Self::Error> {
        self.svc(incoming_stream.0, incoming_stream.1).await;
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
            keepalive_timeout: self.keepalive_timeout,
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
            keepalive_timeout: self.keepalive_timeout,
        })
    }
}

impl<F> TtheaderCoreService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Keepalive>,
    {
        layer_fn(|c: &C, inner| Self::new(inner, c.param()))
    }
}
