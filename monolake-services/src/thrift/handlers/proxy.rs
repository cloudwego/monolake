use std::{convert::Infallible, io};

use monoio::io::{sink::SinkExt, stream::Stream, Splitable};
use monoio_codec::{FramedRead, FramedWrite};
use monoio_thrift::codec::ttheader::{
    RawPayloadCodec, TTHeader, TTHeaderPayloadDecoder, TTHeaderPayloadEncoder,
};
use monoio_transports::{
    connectors::{
        unified_connector::{
            UnifiedTransportAddr, UnifiedTransportConnection, UnifiedTransportConnector,
        },
        Connector,
    },
    pooled::connector::PooledConnector,
};
use monolake_core::{
    context::{PeerAddr, RemoteAddr},
    listener::AcceptedAddr,
    thrift::{ThriftBody, ThriftRequest, ThriftResponse},
};
use service_async::{AsyncMakeService, MakeService, ParamMaybeRef, ParamRef, Service};
use tracing::info;

use crate::http::handlers::rewrite::{Endpoint, RouteConfig};

type PoolThriftConnector = PooledConnector<
    UnifiedTransportConnector,
    UnifiedTransportAddr,
    UnifiedTransportConnection,
    (),
>;

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
    type Error = monoio_transports::Error; // TODO: user error

    async fn call(
        &self,
        (mut req, ctx): (ThriftRequest<ThriftBody>, CX),
    ) -> Result<Self::Response, Self::Error> {
        add_metainfo(&mut req.ttheader, &ctx);
        self.send_request(req).await
    }
}

impl ProxyHandler {
    async fn send_request(
        &self,
        req: ThriftRequest<ThriftBody>,
    ) -> Result<ThriftResponse<ThriftBody>, monoio_transports::Error> {
        // TODO: how to choose key
        let upstream = &self.routes[0].upstreams[0];
        let key = match &upstream.endpoint {
            Endpoint::Socket(addr) => {
                UnifiedTransportAddr::Tcp(addr.ip().to_string().into(), addr.port())
            }
            Endpoint::Unix(path) => UnifiedTransportAddr::Unix(path.clone()),
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

        if let Err(e) = encoder.send_and_flush(req).await {
            return Err(e.into());
        }

        match decoder.next().await {
            Some(Ok(resp)) => Ok(resp),
            Some(Err(e)) => Err(e.into()),
            None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "TODO: eof").into()),
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

fn add_metainfo<CX>(headers: &mut TTHeader, ctx: &CX)
where
    CX: ParamRef<PeerAddr> + ParamMaybeRef<Option<RemoteAddr>>,
{
    let peer_addr = ParamRef::<PeerAddr>::param_ref(ctx);
    let remote_addr = ParamMaybeRef::<Option<RemoteAddr>>::param_maybe_ref(ctx);
    let addr = remote_addr
        .and_then(|addr| addr.as_ref().map(|x| &x.0))
        .unwrap_or(&peer_addr.0);

    let addr = match addr {
        AcceptedAddr::Tcp(addr) => addr.ip().to_string().into(),
        AcceptedAddr::Unix(addr) => addr.as_pathname().and_then(|s| s.to_str()).unwrap().into(),
    };
    headers.str_headers.insert("rip".into(), addr);
}
