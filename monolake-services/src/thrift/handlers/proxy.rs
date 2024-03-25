use std::{convert::Infallible, io};

use monoio::io::{sink::SinkExt, stream::Stream, Splitable};
use monoio_codec::{FramedRead, FramedWrite};
use monoio_thrift::codec::ttheader::{
    RawPayloadCodec, TTHeaderPayloadDecoder, TTHeaderPayloadEncoder,
};
use monoio_transports::{
    connectors::{Connector, UnifiedL4Addr, UnifiedL4Connector, UnifiedL4Stream},
    pool::{PooledConnector, Reuse, ReuseConnector},
};
use monolake_core::{
    context::{PeerAddr, RemoteAddr},
    thrift::{ThriftBody, ThriftRequest, ThriftResponse},
};
use service_async::{AsyncMakeService, MakeService, ParamMaybeRef, ParamRef, Service};
use tracing::info;

use crate::http::handlers::rewrite::{Endpoint, RouteConfig};

type PoolThriftConnector =
    PooledConnector<ReuseConnector<UnifiedL4Connector>, UnifiedL4Addr, Reuse<UnifiedL4Stream>>;

#[derive(Clone, Default)]
pub struct ProxyHandler {
    connector: PoolThriftConnector,
    routes: Vec<RouteConfig>,
}

impl ProxyHandler {
    pub fn new(connector: PoolThriftConnector, routes: Vec<RouteConfig>) -> Self {
        ProxyHandler { connector, routes }
    }

    pub const fn factory(config: Vec<RouteConfig>) -> ProxyHandlerFactory {
        ProxyHandlerFactory { config }
    }
}

impl<CX> Service<(ThriftRequest<ThriftBody>, CX)> for ProxyHandler
where
    CX: ParamRef<PeerAddr> + ParamMaybeRef<Option<RemoteAddr>>,
{
    type Response = ThriftResponse<ThriftBody>;
    type Error = io::Error; // TODO: user error

    async fn call(
        &self,
        (req, _ctx): (ThriftRequest<ThriftBody>, CX),
    ) -> Result<Self::Response, Self::Error> {
        self.send_request(req).await
    }
}

impl ProxyHandler {
    async fn send_request(
        &self,
        req: ThriftRequest<ThriftBody>,
    ) -> Result<ThriftResponse<ThriftBody>, io::Error> {
        // TODO: how to choose key
        let upstream = &self.routes[0].upstreams[0];
        let key = match &upstream.endpoint {
            Endpoint::Socket(addr) => UnifiedL4Addr::Tcp(*addr),
            Endpoint::Unix(path) => UnifiedL4Addr::Unix(path.clone()),
            _ => panic!("not support"),
        };
        let conn = match self.connector.connect(key).await {
            Ok(conn) => conn,
            Err(e) => {
                info!("connect upstream error: {:?}", e);
                return Err(e);
            }
        };

        let (reader, writer) = conn.into_split();

        let mut decoder =
            FramedRead::new(reader, TTHeaderPayloadDecoder::new(RawPayloadCodec::new()));
        let mut encoder =
            FramedWrite::new(writer, TTHeaderPayloadEncoder::new(RawPayloadCodec::new()));

        encoder.send_and_flush(req).await?;

        match decoder.next().await {
            Some(Ok(resp)) => Ok(resp),
            Some(Err(e)) => Err(e),
            None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "TODO: eof")),
        }
    }
}

pub struct ProxyHandlerFactory {
    config: Vec<RouteConfig>,
}

impl MakeService for ProxyHandlerFactory {
    type Service = ProxyHandler;
    type Error = Infallible;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ProxyHandler::new(
            PoolThriftConnector::default(),
            self.config.clone(),
        ))
    }
}

impl AsyncMakeService for ProxyHandlerFactory {
    type Service = ProxyHandler;
    type Error = Infallible;

    async fn make_via_ref(
        &self,
        _old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(ProxyHandler::new(
            PoolThriftConnector::default(),
            self.config.clone(),
        ))
    }
}
