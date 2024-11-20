//! Routing and request handling for a service-oriented architecture.
//!
//! This module provides components for routing HTTP requests to appropriate upstream
//! servers based on configured routes. It is designed to work with the `service_async`
//! crate, implementing its [`Service`] and [`MakeService`] traits for seamless integration
//! into service stacks.
//!
//! # Key Components
//!
//! - [`RewriteAndRouteHandler`]: The main service component responsible for routing requests.
//! - [`RewriteAndRouteHandlerFactory`]: A factory for creating and updating
//!   `RewriteAndRouteHandler` instances.
//! - [`RouteConfig`]: Configuration structure for defining routes and their associated upstreams.
//! - [`Upstream`]: Represents an upstream server configuration.
//!
//! # Architecture
//!
//! The routing system is built around the following workflow:
//!
//! 1. A `RewriteAndRouteHandler` is created by its factory, initialized with a set of routes.
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
//!         detect::H2Detect,
//!         handlers::{
//!             route::RouteConfig, ConnectionReuseHandler, ContentHandler, RewriteAndRouteHandler,
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
//!     .replace(UpstreamHandler::factory(
//!         Default::default(),
//!         Default::default(),
//!     ))
//!     .push(ContentHandler::layer())
//!     .push(RewriteAndRouteHandler::layer())
//!     .push(ConnectionReuseHandler::layer())
//!     .push(HttpCoreService::layer())
//!     .push(H2Detect::layer());
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
use std::convert::Infallible;

use http::{uri::Scheme, HeaderValue, Request, Response, StatusCode};
use monoio_http::common::body::FixedBody;
use monolake_core::{
    http::{HttpError, HttpHandler, ResponseWithContinue, UnrecoverableError},
    util::uri_serde,
    AnyError,
};
use rand::distributions::WeightedError;
use serde::{Deserialize, Serialize};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, Service,
};

use crate::{
    common::route::{
        EmptyCollectionError, IdentitySelector, Mapping, RandomSelector, RoundRobinSelector,
        Selector, SvcRoute, WeightedRandomSelector,
    },
    http::generate_response,
};

#[derive(Debug)]
pub struct Router<T>(pub matchit::Router<T>);

impl Router<LoadBalancer<Endpoint>> {
    pub fn new_from_iter<I, E>(iter: I) -> Result<Self, RoutingFactoryError<E>>
    where
        I: IntoIterator<Item = RouteConfig>,
    {
        let mut router = matchit::Router::new();
        for route in iter {
            router.insert(
                &route.path,
                LoadBalancer::try_from_upstreams(route.load_balancer, route.upstreams).unwrap(),
            )?;
        }
        Ok(Self(router))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RouterError<E> {
    #[error("route empty")]
    RouteEmpty,
    #[error("inner service error: {0:?}")]
    SelectError(#[from] E),
}

impl<B: FixedBody, E> HttpError<B> for RouterError<E> {
    fn to_response(&self) -> Option<Response<B>> {
        Some(generate_response(StatusCode::NOT_FOUND, false))
    }
}

impl<T> Selector<str> for Router<T>
where
    T: Selector<str>,
{
    type Output<'a>
        = T::Output<'a>
    where
        Self: 'a;

    type Error = RouterError<T::Error>;

    #[inline]
    fn select(&self, path: &str) -> Result<Self::Output<'_>, Self::Error> {
        let Ok(r) = self.0.at(path) else {
            return Err(RouterError::RouteEmpty);
        };
        // We are going to ignore the params since it borrows path,
        // however, return it requires the lifetime of the request,
        // which will breaks request ownership movement.
        r.value.select(path).map_err(RouterError::SelectError)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LoadBalanceStrategy {
    #[default]
    Random,
    WeightedRandom,
    RoundRobin,
    First,
}

#[derive(thiserror::Error, Debug)]
pub enum LoadBalanceError {
    #[error("empty upstream")]
    EmptyUpstream,
    #[error("invalid weight")]
    InvalidWeight(#[from] WeightedError),
}

impl From<EmptyCollectionError> for LoadBalanceError {
    #[inline]
    fn from(_: EmptyCollectionError) -> Self {
        Self::EmptyUpstream
    }
}

#[derive(Debug)]
pub enum LoadBalancer<T> {
    Random(RandomSelector<T>),
    WeightedRandom(WeightedRandomSelector<T, u16>),
    RoundRobin(RoundRobinSelector<T>),
    Identity(IdentitySelector<T>),
}

impl LoadBalancer<Endpoint> {
    pub fn try_from_upstreams(
        lb: LoadBalanceStrategy,
        upstreams: impl IntoIterator<Item = Upstream>,
    ) -> Result<Self, LoadBalanceError> {
        let mut it = upstreams.into_iter();
        Ok(match lb {
            LoadBalanceStrategy::Random => {
                RandomSelector::new(it.map(|up| up.endpoint).collect()).map(LoadBalancer::Random)?
            }
            LoadBalanceStrategy::WeightedRandom => {
                struct WeightedIter<I>(I);
                impl<I: Iterator<Item = Upstream>> Iterator for WeightedIter<I> {
                    type Item = (Endpoint, u16);
                    fn next(&mut self) -> Option<Self::Item> {
                        self.0.next().map(|up| (up.endpoint, up.weight))
                    }
                }
                WeightedRandomSelector::new_from_iter(WeightedIter(it))
                    .map(LoadBalancer::WeightedRandom)?
            }
            LoadBalanceStrategy::RoundRobin => {
                RoundRobinSelector::new(it.map(|up| up.endpoint).collect())
                    .map(LoadBalancer::RoundRobin)?
            }
            LoadBalanceStrategy::First => {
                let Some(up) = it.next() else {
                    return Err(LoadBalanceError::EmptyUpstream);
                };
                LoadBalancer::Identity(IdentitySelector(up.endpoint))
            }
        })
    }
}

impl<T, A: ?Sized> Selector<A> for LoadBalancer<T> {
    type Output<'a>
        = &'a T
    where
        Self: 'a;
    type Error = Infallible;

    #[inline]
    fn select(&self, key: &A) -> Result<Self::Output<'_>, Self::Error> {
        match self {
            LoadBalancer::Random(random_selector) => random_selector.select(key),
            LoadBalancer::WeightedRandom(wr_selector) => wr_selector.select(key),
            LoadBalancer::RoundRobin(round_robin_selector) => round_robin_selector.select(key),
            LoadBalancer::Identity(identity_selector) => identity_selector.select(key),
        }
    }
}

pub struct RewriteHandler<H> {
    inner: H,
}

impl<'a, H, CX, B> Service<(Request<B>, &'a Endpoint, CX)> for RewriteHandler<H>
where
    H: HttpHandler<CX, B>,
{
    type Response = ResponseWithContinue<H::Body>;
    type Error = UnrecoverableError<H::Error>;

    #[inline]
    async fn call(
        &self,
        (mut request, ep, cx): (Request<B>, &'a Endpoint, CX),
    ) -> Result<Self::Response, Self::Error> {
        rewrite_request(&mut request, ep);
        return self
            .inner
            .handle(request, cx)
            .await
            .map_err(UnrecoverableError);
    }
}

pub struct PathExtractor;
impl<B> Mapping<Request<B>> for PathExtractor {
    type Out = str;
    #[inline]
    fn map<'a>(&self, input: &'a Request<B>) -> &'a Self::Out {
        input.uri().path()
    }
}

pub struct RewriteAndRouteHandlerFactory<F> {
    inner: F,
    routes: Vec<RouteConfig>,
}

pub type RewriteAndRouteHandler<T> =
    HttpErrorResponder<SvcRoute<Router<LoadBalancer<Endpoint>>, RewriteHandler<T>, PathExtractor>>;

#[derive(thiserror::Error, Debug)]
pub enum RoutingFactoryError<E> {
    #[error("inner error: {0:?}")]
    Inner(E),
    #[error("load balance error: {0:?}")]
    LoadBalanceError(#[from] LoadBalanceError),
    #[error("router error: {0:?}")]
    Router(#[from] matchit::InsertError),
}

impl<F: MakeService> MakeService for RewriteAndRouteHandlerFactory<F> {
    type Service = RewriteAndRouteHandler<F::Service>;
    type Error = RoutingFactoryError<F::Error>;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        let router = Router::new_from_iter(self.routes.clone())?;
        Ok(HttpErrorResponder(SvcRoute {
            svc: RewriteHandler {
                inner: self
                    .inner
                    .make_via_ref(old.map(|o| &o.0.svc.inner))
                    .map_err(RoutingFactoryError::Inner)?,
            },
            selector: router,
            selector_mapper: PathExtractor,
        }))
    }
}

impl<F: AsyncMakeService> AsyncMakeService for RewriteAndRouteHandlerFactory<F>
where
    F::Error: Into<AnyError>,
{
    type Service = RewriteAndRouteHandler<F::Service>;
    type Error = RoutingFactoryError<F::Error>;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        let router = Router::new_from_iter(self.routes.clone())?;
        Ok(HttpErrorResponder(SvcRoute {
            svc: RewriteHandler {
                inner: self
                    .inner
                    .make_via_ref(old.map(|o| &o.0.svc.inner))
                    .await
                    .map_err(RoutingFactoryError::Inner)?,
            },
            selector: router,
            selector_mapper: PathExtractor,
        }))
    }
}

pub struct HttpErrorResponder<T>(pub T);
impl<CX, T, B> Service<(Request<B>, CX)> for HttpErrorResponder<T>
where
    T: HttpHandler<CX, B>,
    T::Error: HttpError<T::Body>,
{
    type Response = ResponseWithContinue<T::Body>;
    type Error = T::Error;

    async fn call(&self, (req, cx): (Request<B>, CX)) -> Result<Self::Response, Self::Error> {
        match self.0.handle(req, cx).await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                if let Some(r) = e.to_response() {
                    Ok((r, true))
                } else {
                    Err(e)
                }
            }
        }
    }
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

    #[serde(default)]
    pub load_balancer: LoadBalanceStrategy,

    /// The path pattern to match incoming requests against.
    ///
    /// This can be an exact path or a pattern supported by the routing system.
    pub path: String,

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

impl<F> RewriteAndRouteHandler<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = RewriteAndRouteHandlerFactory<F>>
    where
        C: Param<Vec<RouteConfig>>,
    {
        layer_fn(|c: &C, inner| {
            let routes = c.param();
            RewriteAndRouteHandlerFactory { inner, routes }
        })
    }
}

fn rewrite_request<B>(request: &mut Request<B>, endpoint: &Endpoint) {
    let remote = match endpoint {
        Endpoint::Uri(uri) => uri,
        _ => unimplemented!("not implement"),
    };

    if let Some(authority) = remote.authority() {
        let header_value =
            HeaderValue::from_str(authority.as_str()).unwrap_or(HeaderValue::from_static(""));
        tracing::debug!(
            "Request: {:?} -> {:?}",
            request.headers().get(http::header::HOST),
            header_value
        );

        request.headers_mut().remove(http::header::HOST);

        request
            .headers_mut()
            .insert(http::header::HOST, header_value);

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
            load_balancer: Default::default(),
            path: format!("/{n}"),
            upstreams: Vec::from([Upstream {
                endpoint: Endpoint::Uri(format!("http://test{n}.endpoint").parse().unwrap()),
                weight: Default::default(),
            }]),
        })
    }

    #[test]
    fn test_iterate_match() {
        let mut router: matchit::Router<RouteConfig> = matchit::Router::new();
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
