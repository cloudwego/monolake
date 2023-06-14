use std::{convert::Infallible, fmt::Debug, future::Future, pin::Pin, time::Duration};

use http::StatusCode;
use monoio::io::{sink::SinkExt, stream::Stream, AsyncReadRent, AsyncWriteRent, Split, Splitable};
use monoio_http::h1::codec::{
    decoder::{FillPayload, RequestDecoder},
    encoder::GenericEncoder,
};
use monolake_core::{
    config::{KeepaliveConfig, DEFAULT_TIMEOUT},
    http::{HttpError, HttpHandler},
};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};
use tracing::{error, info, warn};

use super::{generate_response, util::AccompanyPair};
use crate::common::Accept;

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

    fn call(&self, incoming_stream: Accept<Stream, SocketAddr>) -> Self::Future<'_> {
        let (stream, addr) = incoming_stream;
        let (reader, writer) = stream.into_split();
        let mut decoder = RequestDecoder::new(reader);
        let mut encoder = GenericEncoder::new(writer);

        async move {
            loop {
                // decode request with keepalive timeout
                let req = match monoio::time::timeout(self.keepalive_timeout, decoder.next()).await
                {
                    Ok(Some(Ok(req))) => req,
                    Ok(Some(Err(err))) => {
                        // decode error
                        warn!("decode request header failed: {err}");
                        break;
                    }
                    Ok(None) => {
                        // EOF
                        info!("Connection {addr:?} closed");
                        break;
                    }
                    Err(_) => {
                        // timeout
                        info!("Connection {addr:?} keepalive timed out");
                        break;
                    }
                };

                // Check if we should keepalive

                // handle request and reply response
                // 1. do these things simultaneously: read body and send + handle request
                let mut acc_fut =
                    AccompanyPair::new(self.handler_chain.handle(req), decoder.fill_payload());
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
                            .send_and_flush(generate_response(
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
