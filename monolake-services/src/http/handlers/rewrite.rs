use std::future::Future;

use http::{Request, StatusCode};
use matchit::Router;
use monoio_http::h1::payload::Payload;
use monolake_core::{
    config::RouteConfig,
    environments::Environments,
    http::{HttpHandler, ResponseWithContinue, Rewrite},
};
use rand::RngCore;
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

impl<H> Service<(Request<Payload>, Environments)> for RewriteHandler<H>
where
    H: HttpHandler,
{
    type Response = ResponseWithContinue;
    type Error = H::Error;
    type Future<'a> = impl Future<Output = Result<Self::Response, Self::Error>> + 'a
    where
        Self: 'a;

    fn call(
        &self,
        (mut request, environments): (Request<Payload>, Environments),
    ) -> Self::Future<'_> {
        async move {
            let req_path = request.uri().path();
            tracing::info!("request path: {}", req_path);

            match self.router.at(req_path) {
                Ok(route) => {
                    let route = route.value;
                    tracing::info!("the route id: {}", route.id);
                    let upstreams = &route.upstreams;
                    let mut rng = rand::thread_rng();
                    let next = rng.next_u32() as usize % upstreams.len();
                    let upstream: &monolake_core::config::Upstream = &upstreams[next];

                    Rewrite::rewrite_request(&mut request, &upstream.endpoint);

                    self.inner.handle(request, environments).await
                }
                Err(e) => {
                    debug!("match request uri: {} with error: {}", request.uri(), e);
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

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use matchit::Router;
    use monolake_core::config::{Endpoint, RouteConfig, Upstream, Uri};

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
            path: format!("/{}", n),
            upstreams: Vec::from([Upstream {
                endpoint: Endpoint::Uri(Uri {
                    uri: format!("http://test{}.endpoint", n).parse().unwrap(),
                }),
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
