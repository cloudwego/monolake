//! Routing and request handling for a service-oriented architecture.
//!
//! This module provides components for routing HTTP requests to appropriate upstream
//! servers based on configured routes. It is designed to work with the `service_async`
//! crate, implementing its [`Service`] and [`MakeService`] traits for seamless integration
//! into service stacks.
//!
//! # Key Components
//!
//! - [`RoutingHandler`]: The main service component responsible for routing requests.
//! - [`RoutingHandlerFactory`]: A factory for creating and updating `RoutingHandler` instances.
//! - [`RouteConfig`]: Configuration structure for defining routes and their associated upstreams.
//! - [`Upstream`]: Represents an upstream server configuration.
//!
//! # Architecture
//!
//! The routing system is built around the following workflow:
//!
//! 1. A `RoutingHandler` is created by its factory, initialized with a set of routes.
//! 2. Incoming requests are matched against these routes using a [`matchit::Router`].
//! 3. When a match is found, an upstream server is selected (with support for load balancing).
//! 4. The request is rewritten as necessary for the selected upstream.
//! 5. The rewritten request is passed to an inner handler for further processing
//!
//! # Usage
//!
//! This module is typically used as part of a larger service stack. Here's a basic example:
//!
//! ```rust
//! use monolake_services::{
//!     common::ContextService,
//!     http::{
//!         core::HttpCoreService,
//!         detect::HttpVersionDetect,
//!         handlers::{
//!             route::RouteConfig, ConnectionPersistenceHandler, ContentHandler, RoutingHandler,
//!             UpstreamHandler,
//!         },
//!         HttpServerTimeout,
//!     },
//! };
//! use service_async::{layer::FactoryLayer, stack::FactoryStack, Param};
//!
//! // Dummy struct to satisfy Param trait requirements
//! struct DummyConfig;
//!
//! // Implement Param for DummyConfig to return Vec<RouteConfig>
//! impl Param<Vec<RouteConfig>> for DummyConfig {
//!     fn param(&self) -> Vec<RouteConfig> {
//!         vec![]
//!     }
//! }
//! impl Param<HttpServerTimeout> for DummyConfig {
//!     fn param(&self) -> HttpServerTimeout {
//!         HttpServerTimeout::default()
//!     }
//! }
//!
//! let config = DummyConfig;
//! let stacks = FactoryStack::new(config)
//!     .replace(UpstreamHandler::factory(Default::default()))
//!     .push(ContentHandler::layer())
//!     .push(RoutingHandler::layer())
//!     .push(ConnectionPersistenceHandler::layer())
//!     .push(HttpCoreService::layer())
//!     .push(HttpVersionDetect::layer());
//!
//! // Use the service to handle HTTP requests
//! ```
//!
//! # Configuration
//!
//! Routing is configured through [`RouteConfig`] structures, which define paths and their
//! associated upstreams. These configurations can be dynamically updated by recreating
//! the handler through its factory.
//!
//! # Error Handling
//!
//! - Routing errors (no matching route) result in a 404 Not Found response.
//! - Other errors are propagated from the inner handler.
//!
//! # Performance Considerations
//!
//! - The module uses [`matchit::Router`] for efficient path matching.
//! - Upstream selection supports weighted load balancing.
//!
//! # Feature Flags
//!
//! - `tls`: Enables TLS support for upstream connections.
//!
//! # Future Directions
//!
//! - Support for more advanced routing patterns (e.g., regex-based routing).
//! - Enhanced metrics and logging for better observability.
//! - Integration with service discovery systems for dynamic upstream management.
use http::{uri::Scheme, HeaderValue, Request, StatusCode, Version};
use matchit::Router;
use monoio_http::common::body::FixedBody;
use monolake_core::{
    http::{HttpHandler, ResponseWithContinue},
    util::uri_serde,
    AnyError,
};
use serde::{Deserialize, Serialize};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, Service,
};
use tracing::debug;

use crate::http::generate_response;

/// A handler that routes incoming requests to appropriate upstream servers based on configured
/// routes.
///
/// [`RoutingHandler`] is responsible for matching incoming request paths against a set of
/// predefined routes, selecting an appropriate upstream server, and forwarding the request to that
/// server. It implements the `Service` trait from the `service_async` crate, providing an
/// asynchronous request handling mechanism.
///
/// # Type Parameters
///
/// - `H`: The type of the inner handler, which must implement `HttpHandler`.
///
/// # Fields
///
/// - `inner`: The inner handler that processes requests after routing.
/// - `router`: A `matchit::Router` containing the routing configuration.
///
/// # Usage
///
/// This handler is typically created using the [`RoutingHandlerFactory`], which allows for
/// dynamic creation and updates of the routing configuration. It can be integrated into a
/// service stack using the `layer` method, enabling composition with other services.
///
///
/// # Service Implementation
///
/// The `call` method of this handler performs the following steps:
/// 1. Extracts the path from the incoming request.
/// 2. Matches the path against the configured routes.
/// 3. If a match is found:
///    - Selects an upstream server from the matched route.
///    - Rewrites the request for the selected upstream.
///    - Forwards the request to the inner handler.
/// 4. If no match is found, returns a 404 Not Found response.
///
/// # Performance Considerations
///
/// This handler uses [`matchit::Router`] for efficient path matching, which is generally
/// faster than iterative matching for a large number of routes.
#[derive(Clone)]
pub struct RoutingHandler<H> {
    inner: H,
    router: Router<RouteConfig>,
}

impl<H, CX, B> Service<(Request<B>, CX)> for RoutingHandler<H>
where
    H: HttpHandler<CX, B>,
    H::Body: FixedBody,
{
    type Response = ResponseWithContinue<H::Body>;
    type Error = H::Error;

    async fn call(
        &self,
        (mut request, ctx): (Request<B>, CX),
    ) -> Result<Self::Response, Self::Error> {
        let req_path = request.uri().path();
        tracing::info!("request path: {req_path}");

        match self.router.at(req_path) {
            Ok(route) => {
                let route = route.value;
                tracing::info!("the route id: {}", route.id);
                use rand::seq::SliceRandom;
                let upstream = route
                    .upstreams
                    .choose(&mut rand::thread_rng())
                    .expect("empty upstream list");

                rewrite_request(&mut request, upstream);

                self.inner.handle(request, ctx).await
            }
            Err(e) => {
                debug!("match request uri: {} with error: {e}", request.uri());
                Ok((generate_response(StatusCode::NOT_FOUND, false), true))
            }
        }
    }
}

/// Factory for creating [`RoutingHandler`] instances.
///
/// This factory implements the [`MakeService`] &
/// [`AsyncMakeService`] trait, allowing for dynamic creation and updates of
/// `RoutingHandler` instances. It's designed to work with the `service_async` crate's compositional
/// model.
pub struct RoutingHandlerFactory<F> {
    inner: F,
    routes: Vec<RouteConfig>,
}

#[derive(thiserror::Error, Debug)]
pub enum RoutingFactoryError<E> {
    #[error("inner error: {0:?}")]
    Inner(E),
    #[error("empty upstream")]
    EmptyUpstream,
    #[error("router error: {0:?}")]
    Router(#[from] matchit::InsertError),
}

impl<F: MakeService> MakeService for RoutingHandlerFactory<F> {
    type Service = RoutingHandler<F::Service>;
    type Error = RoutingFactoryError<F::Error>;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        let mut router: Router<RouteConfig> = Router::new();
        for route in self.routes.iter() {
            router.insert(&route.path, route.clone())?;
            if route.upstreams.is_empty() {
                return Err(RoutingFactoryError::EmptyUpstream);
            }
        }
        Ok(RoutingHandler {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(RoutingFactoryError::Inner)?,
            router,
        })
    }
}

impl<F: AsyncMakeService> AsyncMakeService for RoutingHandlerFactory<F>
where
    F::Error: Into<AnyError>,
{
    type Service = RoutingHandler<F::Service>;
    type Error = RoutingFactoryError<F::Error>;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        let mut router: Router<RouteConfig> = Router::new();
        for route in self.routes.iter() {
            router.insert(&route.path, route.clone())?;
            if route.upstreams.is_empty() {
                return Err(RoutingFactoryError::EmptyUpstream);
            }
        }
        Ok(RoutingHandler {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(RoutingFactoryError::Inner)?,
            router,
        })
    }
}

const fn default_weight() -> u16 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[derive(Default)]
pub enum HttpVersion {
    #[default]
    HTTP1_1,
    HTTP2,
}

impl HttpVersion {
    pub fn convert_to_http_version(&self) -> http::Version {
        match self {
            HttpVersion::HTTP1_1 => http::Version::HTTP_11,
            HttpVersion::HTTP2 => http::Version::HTTP_2,
        }
    }
}

/// Configuration for a single route in the routing system.
///
/// This structure defines how a particular path should be routed to one or more upstream servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// Unique identifier for the route.
    #[serde(skip)]
    pub id: String,

    /// The path pattern to match incoming requests against.
    ///
    /// This can be an exact path or a pattern supported by the routing system.
    pub path: String,

    /// A list of upstream servers that can handle requests matching this route.
    ///
    /// Multiple upstreams allow for load balancing and failover configurations.
    pub upstreams: Vec<Upstream>,
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

    /// The HTTP version to use when communicating with this upstream.
    ///
    /// If not specified, it defaults to the value provided by `HttpVersion::default`.
    #[serde(default = "HttpVersion::default")]
    pub version: HttpVersion,
}

/// Represents different types of endpoints for upstream servers.
///
/// This enum allows for flexibility in specifying how to connect to an upstream server,
/// supporting various protocols and addressing methods.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Endpoint {
    /// A URI endpoint.
    ///
    /// This variant uses custom serialization/deserialization logic defined in `uri_serde`.
    #[serde(with = "uri_serde")]
    Uri(http::Uri),

    /// A socket address endpoint.
    ///
    /// This can be used for direct IP:port addressing.
    Socket(std::net::SocketAddr),

    /// A Unix domain socket endpoint.
    ///
    /// This is typically used for local inter-process communication on Unix-like systems.
    Unix(std::path::PathBuf),
}

impl<F> RoutingHandler<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = RoutingHandlerFactory<F>>
    where
        C: Param<Vec<RouteConfig>>,
    {
        layer_fn(|c: &C, inner| {
            let routes = c.param();
            RoutingHandlerFactory { inner, routes }
        })
    }
}

fn rewrite_request<B>(request: &mut Request<B>, upstream: &Upstream) {
    let remote = match &upstream.endpoint {
        Endpoint::Uri(uri) => uri,
        _ => unimplemented!("not implement"),
    };

    let endpoint_version = upstream.version.convert_to_http_version();
    *request.version_mut() = endpoint_version;

    if let Some(authority) = remote.authority() {
        let header_value =
            HeaderValue::from_str(authority.as_str()).unwrap_or(HeaderValue::from_static(""));
        tracing::debug!(
            "Request: {:?} -> {:?}",
            request.headers().get(http::header::HOST),
            header_value
        );

        match endpoint_version {
            Version::HTTP_11 => request.headers_mut().remove(http::header::HOST),
            Version::HTTP_2 => request.headers_mut().remove(http::header::HOST),
            _ => unimplemented!(),
        };

        if upstream.version.convert_to_http_version() == Version::HTTP_2 {
            request.headers_mut().remove(http::header::HOST);
        } else {
            request
                .headers_mut()
                .insert(http::header::HOST, header_value);
        }

        let scheme = match remote.scheme() {
            Some(scheme) => scheme.to_owned(),
            None => Scheme::HTTP,
        };

        let uri = request.uri_mut();
        let path_and_query = match uri.path_and_query() {
            Some(path_and_query) => match path_and_query.query() {
                Some(query) => format!("{}?{}", remote.path(), query),
                None => String::from(remote.path()),
            },
            None => "/".to_string(),
        };
        *uri = http::Uri::builder()
            .authority(authority.to_owned())
            .scheme(scheme)
            .path_and_query(path_and_query)
            .build()
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    fn iterate_match<'a>(req_path: &str, routes: &'a [RouteConfig]) -> Option<&'a RouteConfig> {
        let mut target_route = None;
        let mut route_len = 0;
        for route in routes.iter() {
            let route_path = &route.path;
            let route_path_len = route_path.len();
            if req_path.starts_with(route_path) && route_path_len > route_len {
                target_route = Some(route);
                route_len = route_path_len;
            }
        }
        target_route
    }

    fn create_routes() -> impl Iterator<Item = RouteConfig> {
        let total_routes = 1024 * 100;
        (0..total_routes).map(|n| RouteConfig {
            id: "testroute".to_string(),
            path: format!("/{n}"),
            upstreams: Vec::from([Upstream {
                endpoint: Endpoint::Uri(format!("http://test{n}.endpoint").parse().unwrap()),
                weight: Default::default(),
                version: HttpVersion::HTTP1_1,
            }]),
        })
    }

    #[test]
    fn test_iterate_match() {
        let mut router: Router<RouteConfig> = Router::new();
        create_routes().for_each(|route| router.insert(route.path.clone(), route).unwrap());
        let routes: Vec<RouteConfig> = create_routes().collect();
        let target_path = "/1024";

        let current = SystemTime::now();
        let iterate_route = iterate_match(target_path, &routes).unwrap();
        let iterate_match_elapsed = current.elapsed().unwrap().as_micros();

        let current = SystemTime::now();
        let matchit_route = router.at(target_path).unwrap().value;
        let matchit_match_elapsed = current.elapsed().unwrap().as_micros();

        assert_eq!(
            format!("{:?}", iterate_route),
            format!("{:?}", matchit_route)
        );
        println!("{:?}", iterate_route);
        assert!(matchit_match_elapsed < (iterate_match_elapsed / 100));
    }
}
