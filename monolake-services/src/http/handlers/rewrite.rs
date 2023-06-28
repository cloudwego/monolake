use std::future::Future;

use http::{uri::Scheme, HeaderValue, Request, StatusCode};
use matchit::Router;
use monoio_http::h1::payload::Payload;
use monolake_core::http::{HttpHandler, ResponseWithContinue};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};
use tracing::debug;

use crate::http::generate_response;

#[derive(Clone)]
pub struct RewriteHandler<H> {
    inner: H,
    router: Router<RouteConfig>,
}

impl<H, CX> Service<(Request<Payload>, CX)> for RewriteHandler<H>
where
    H: HttpHandler<CX>,
{
    type Response = ResponseWithContinue;
    type Error = H::Error;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a, CX: 'a;

    fn call(&self, (mut request, ctx): (Request<Payload>, CX)) -> Self::Future<'_> {
        async move {
            let req_path = request.uri().path();
            tracing::info!("request path: {req_path}");

            match self.router.at(req_path) {
                Ok(route) => {
                    let route = route.value;
                    tracing::info!("the route id: {}", route.id);
                    let upstreams = &route.upstreams;
                    let mut rng = rand::thread_rng();
                    let next = rng.next_u32() as usize % upstreams.len();
                    let upstream: &Upstream = &upstreams[next];

                    rewrite_request(&mut request, &upstream.endpoint);

                    self.inner.handle(request, ctx).await
                }
                Err(e) => {
                    debug!("match request uri: {} with error: {e}", request.uri());
                    Ok((generate_response(StatusCode::NOT_FOUND, false), true))
                }
            }
        }
    }
}

// RewriteHandler is a Service and a MakeService.
impl<F> MakeService for RewriteHandler<F>
where
    F: MakeService,
{
    type Service = RewriteHandler<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(RewriteHandler {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
            router: self.router.clone(),
        })
    }
}

const fn default_weight() -> u16 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    #[serde(skip)]
    pub id: String,
    pub path: String,
    pub upstreams: Vec<Upstream>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub endpoint: Endpoint,
    #[serde(default = "default_weight")]
    pub weight: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Endpoint {
    #[serde(with = "http_serde::uri")]
    Uri(http::Uri),
    Socket(std::net::SocketAddr),
    Unix(std::path::PathBuf),
}

impl<F> RewriteHandler<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Vec<RouteConfig>>,
    {
        layer_fn(|c: &C, inner| {
            let routes = c.param();
            let mut router: Router<RouteConfig> = Router::new();
            for route in routes.into_iter() {
                router.insert(route.path.clone(), route.clone()).unwrap();
            }
            Self { inner, router }
        })
    }
}

fn rewrite_request(request: &mut Request<Payload>, endpoint: &Endpoint) {
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
        request
            .headers_mut()
            .insert(http::header::HOST, header_value);

        let scheme = match remote.scheme() {
            Some(scheme) => scheme.to_owned(),
            None => Scheme::HTTP,
        };

        let uri = request.uri_mut();
        let path_and_query = match uri.path_and_query() {
            Some(path_and_query) => path_and_query.as_str(),
            None => "/",
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

    use matchit::Router;

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
