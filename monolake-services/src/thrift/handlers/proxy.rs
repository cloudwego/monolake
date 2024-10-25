//! Thrift proxy handler for routing and forwarding Thrift requests to upstream servers.
//!
//! This module provides a high-performance, asynchronous Thrift proxy service that handles
//! routing and forwarding of Thrift requests to configured upstream servers. It is designed
//! to work with monoio's asynchronous runtime and supports connection pooling for efficient
//! resource utilization.
//!
//! # Key Components
//!
//! - [`ProxyHandler`]: The main service component responsible for proxying Thrift requests to
//!   upstream servers based on configured routes.
//! - [`ProxyHandlerFactory`]: Factory for creating `ProxyHandler` instances.
//! - [`PoolThriftConnector`]: A pooled connector for managing Thrift connections to upstream
//!   servers.
//!
//! # Features
//!
//! - Support for Thrift THeader protocol
//! - Configurable routing of requests to upstream servers
//! - Connection pooling for efficient resource management
//! - Integration with `service_async` for easy composition in service stacks
//! - Support for both TCP and Unix socket connections to upstream servers
//!
//! # Usage
//!
//! `ProxyHandler` is typically used as part of a larger service stack. Here's a basic example:
//!
//! ```ignore
//! use service_async::{layer::FactoryLayer, stack::FactoryStack};
//!
//! use crate::thrift::ProxyHandler;
//!
//! let config = vec![RouteConfig { /* ... */ }];
//! let stack = FactoryStack::new(config.clone())
//!     .push(ProxyHandler::factory(config))
//!     // ... other layers ...
//!     ;
//!
//! let service = stack.make_async().await.unwrap();
//! // Use the service to handle incoming Thrift requests and proxy them to upstream servers
//! ```
//!
//! # Performance Considerations
//!
//! - Uses monoio's efficient async I/O operations for improved performance
//! - Implements connection pooling to reduce connection establishment overhead
//! - Efficient request and response handling using the THeader protocol

use std::{convert::Infallible, io};

use monoio::io::{sink::SinkExt, stream::Stream};
use monoio_codec::Framed;
use monoio_thrift::codec::ttheader::{RawPayloadCodec, TTHeaderPayloadCodec};
use monoio_transports::{
    connectors::{Connector, UnifiedL4Addr, UnifiedL4Connector, UnifiedL4Stream},
    pool::{ConnectorMap, ConnectorMapper, PooledConnector, Reuse, ReuseConnector},
};
use monolake_core::{
    context::{PeerAddr, RemoteAddr},
    thrift::{ThriftBody, ThriftRequest, ThriftResponse},
};
use service_async::{AsyncMakeService, MakeService, ParamMaybeRef, ParamRef, Service};
use tracing::info;

use crate::http::handlers::route::{Endpoint, RouteConfig};

pub type PoolThriftConnector = PooledConnector<
    ReuseConnector<ConnectorMap<UnifiedL4Connector, ThriftConnectorMapper>>,
    UnifiedL4Addr,
    Reuse<Framed<UnifiedL4Stream, TTHeaderPayloadCodec<RawPayloadCodec>>>,
>;

#[inline]
fn new_connector() -> PoolThriftConnector {
    PooledConnector::new_with_default_pool(ReuseConnector(ConnectorMap::new(
        UnifiedL4Connector::default(),
        ThriftConnectorMapper,
    )))
}

/// Mapper for creating Thrift-specific connections from generic network connections.
///
/// `ThriftConnectorMapper` is responsible for wrapping raw network connections with
/// the appropriate Thrift protocol codec (TTHeaderPayloadCodec in this case).
pub struct ThriftConnectorMapper;
impl<C, E> ConnectorMapper<C, E> for ThriftConnectorMapper {
    type Connection = Framed<C, TTHeaderPayloadCodec<RawPayloadCodec>>;
    type Error = E;

    #[inline]
    fn map(&self, inner: Result<C, E>) -> Result<Self::Connection, Self::Error> {
        inner.map(|io| Framed::new(io, TTHeaderPayloadCodec::new(RawPayloadCodec)))
    }
}

/// Thrift proxy handler for routing and forwarding requests to upstream servers.
///
/// `ProxyHandler` is responsible for receiving Thrift requests, selecting an appropriate
/// upstream server based on configured routes, and forwarding the request to that server.
/// It manages connections to upstream servers using a connection pool for efficiency.
/// For implementation details and example usage, see the
/// [module level documentation](crate::thrift::handlers::proxy).
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
        let mut io = match self.connector.connect(key).await {
            Ok(conn) => conn,
            Err(e) => {
                info!("connect upstream error: {:?}", e);
                return Err(e);
            }
        };

        if let Err(e) = io.send_and_flush(req).await {
            io.set_reuse(false);
            return Err(e);
        }

        match io.next().await {
            Some(Ok(resp)) => Ok(resp),
            Some(Err(e)) => {
                io.set_reuse(false);
                Err(e)
            }
            None => {
                io.set_reuse(false);
                Err(io::ErrorKind::UnexpectedEof.into())
            }
        }
    }
}

/// Factory for creating `ProxyHandler` instances.
///
/// `ProxyHandlerFactory` is responsible for creating new `ProxyHandler` instances,
/// initializing them with the necessary configuration and connection pool.
pub struct ProxyHandlerFactory {
    config: Vec<RouteConfig>,
}

impl MakeService for ProxyHandlerFactory {
    type Service = ProxyHandler;
    type Error = Infallible;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ProxyHandler::new(new_connector(), self.config.clone()))
    }
}

impl AsyncMakeService for ProxyHandlerFactory {
    type Service = ProxyHandler;
    type Error = Infallible;

    async fn make_via_ref(
        &self,
        _old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(ProxyHandler::new(new_connector(), self.config.clone()))
    }
}
