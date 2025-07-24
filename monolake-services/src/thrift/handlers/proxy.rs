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

use std::io;

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
use serde::{Deserialize, Serialize};
use service_async::{AsyncMakeService, MakeService, ParamMaybeRef, ParamRef, Service};

use crate::common::selector::{
    IntoWeightedEndpoint, LoadBalanceError, LoadBalanceStrategy, LoadBalancer, Select,
};

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
    endpoints: LoadBalancer<Endpoint>,
}

impl RouteConfig {
    fn proxy_handler(&self) -> Result<ProxyHandler, LoadBalanceError> {
        Ok(ProxyHandler::new(
            new_connector(),
            LoadBalancer::try_from_upstreams(self.load_balancer, self.upstreams.clone())?,
        ))
    }
}

impl ProxyHandler {
    pub fn new(connector: PoolThriftConnector, endpoints: LoadBalancer<Endpoint>) -> Self {
        ProxyHandler {
            connector,
            endpoints,
        }
    }

    pub const fn factory(config: RouteConfig) -> ProxyHandlerFactory {
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
        let endpoint = self.endpoints.select(&req).unwrap();
        let key = match endpoint {
            Endpoint::Socket(addr) => UnifiedL4Addr::Tcp(*addr),
            Endpoint::Unix(path) => UnifiedL4Addr::Unix(path.clone()),
        };
        let mut io = match self.connector.connect(key).await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::info!("connect upstream error: {:?}", e);
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
    config: RouteConfig,
}

impl MakeService for ProxyHandlerFactory {
    type Service = ProxyHandler;
    type Error = LoadBalanceError;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        self.config.proxy_handler()
    }
}

impl AsyncMakeService for ProxyHandlerFactory {
    type Service = ProxyHandler;
    type Error = LoadBalanceError;

    async fn make_via_ref(
        &self,
        _old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        self.config.proxy_handler()
    }
}

/// Configuration for a single route in the routing system.
///
/// This structure defines how a particular path should be routed to one or more upstream servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    #[serde(default)]
    pub load_balancer: LoadBalanceStrategy,

    /// A list of upstream servers that can handle requests matching this route.
    ///
    /// Multiple upstreams allow for load balancing and failover configurations.
    pub upstreams: Vec<Upstream>,
}

const fn default_weight() -> u16 {
    1
}

/// Configuration for an upstream server.
///
/// This structure defines the properties of a single upstream server,
/// including its endpoint, weight for load balancing, and HTTP version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    /// The endpoint of the upstream server.
    pub endpoint: Endpoint,

    /// The weight of this upstream for load balancing purposes.
    ///
    /// A higher weight means the upstream is more likely to be chosen when distributing requests.
    /// If not specified, it defaults to a value provided by the `default_weight` function.
    #[serde(default = "default_weight")]
    pub weight: u16,
}

impl IntoWeightedEndpoint for Upstream {
    type Endpoint = Endpoint;

    #[inline]
    fn into_weighted_endpoint(self) -> (Self::Endpoint, u16) {
        (self.endpoint, self.weight)
    }
}

/// Represents different types of endpoints for upstream servers.
///
/// This enum allows for flexibility in specifying how to connect to an upstream server,
/// supporting various protocols and addressing methods.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Endpoint {
    /// A socket address endpoint.
    ///
    /// This can be used for direct IP:port addressing.
    Socket(std::net::SocketAddr),

    /// A Unix domain socket endpoint.
    ///
    /// This is typically used for local inter-process communication on Unix-like systems.
    Unix(std::path::PathBuf),
}
