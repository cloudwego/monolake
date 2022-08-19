use std::{collections::HashMap, future::Future};

use anyhow::bail;

use log::debug;
use monoio::{
    io::{sink::SinkExt, stream::Stream},
    net::TcpStream,
};
use monoio_gateway_core::{
    dns::{http::Domain, Resolvable},
    error::GError,
    http::router::{RouterConfig, RouterRule},
    service::{Layer, Service},
};
use monoio_http::{
    common::request::Request,
    h1::{
        codec::{
            decoder::{RequestDecoder, ResponseDecoder},
            encoder::GenericEncoder,
        },
        payload::Payload,
    },
};

use super::{
    accept::TcpAccept,
    tls::TlsAccept,
    transfer::{HttpTransferParams, HttpsTransferParams},
};
#[derive(Clone)]
pub struct RouterService<T, A> {
    inner: T,
    routes: HashMap<String, RouterConfig<A>>,
}

impl<T> Service<TlsAccept> for RouterService<T, Domain>
where
    T: Service<HttpsTransferParams>,
{
    type Response = Option<T::Response>;

    type Error = GError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>>
    where
        Self: 'cx;

    fn call(&mut self, req: TlsAccept) -> Self::Future<'_> {
        async {
            let (local_read, local_write) = req.0.split();
            let _local_encoder = GenericEncoder::new(local_write);
            let mut local_decoder = RequestDecoder::new(local_read);
            match local_decoder.next().await {
                Some(Ok(req)) => {
                    let req: Request = req;
                    let host = get_host(&req);
                    match host {
                        Some(host) => {
                            let domain = Domain::with_uri(host.parse()?);
                            let target = self.match_target(&host.to_owned());
                            match target {
                                Some(target) => {
                                    let m = longest_match(req.uri().path(), target.get_rules());
                                    if let Some(rule) = m {
                                        let proxy_pass = rule.get_proxy_pass();
                                        match proxy_pass.resolve().await? {
                                            Some(_endpoint) => {
                                                // connect endpoint
                                                todo!()
                                            }
                                            _ => {}
                                        }
                                    } else {
                                        // no match router rule
                                        debug!("no matching router rule, {}", domain);
                                    }
                                }
                                None => {
                                    debug!("no matching endpoint, ignoring {}", domain);
                                }
                            }
                        }
                        None => {
                            // no host, ignore!
                            debug!("request has no host, uri: {}", req.uri());
                        }
                    }
                }
                Some(Err(err)) => {
                    // TODO: fallback to tcp
                    debug!("detect ssl failed, fallback to tcp");
                    bail!("{}", err)
                }
                _ => {}
            }
            Ok(None)
        }
    }
}

impl<T> Service<TcpAccept> for RouterService<T, Domain>
where
    T: Service<HttpTransferParams>,
{
    type Response = Option<T::Response>;

    type Error = GError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>>
    where
        Self: 'cx;

    fn call(&mut self, local_stream: TcpAccept) -> Self::Future<'_> {
        async move {
            debug!("find route for {:?}", local_stream.1);
            let (local_read, local_write) = local_stream.0.into_split();
            let local_encoder = GenericEncoder::new(local_write);
            let mut local_decoder = RequestDecoder::new(local_read);
            match local_decoder.next().await {
                Some(Ok(req)) => {
                    let req: Request = req;
                    let host = get_host(&req);
                    match host {
                        Some(host) => {
                            let domain = Domain::with_uri(host.parse()?);
                            let target = self.match_target(&host.to_owned());
                            match target {
                                Some(target) => {
                                    let m = longest_match(req.uri().path(), target.get_rules());
                                    if let Some(rule) = m {
                                        let proxy_pass = rule.get_proxy_pass();
                                        match proxy_pass.resolve().await? {
                                            Some(endpoint) => {
                                                // connect endpoint
                                                let remote = TcpStream::connect(endpoint).await?;
                                                let (remote_read, remote_write) =
                                                    remote.into_split();
                                                let mut remote_encoder =
                                                    GenericEncoder::new(remote_write);
                                                let remote_decoder =
                                                    ResponseDecoder::new(remote_read);
                                                remote_encoder.send_and_flush(req).await?;
                                                match self
                                                    .inner
                                                    .call((
                                                        local_encoder,
                                                        local_decoder,
                                                        remote_encoder,
                                                        remote_decoder,
                                                    ))
                                                    .await
                                                {
                                                    Ok(resp) => {
                                                        return Ok(Some(resp));
                                                    }
                                                    Err(_) => bail!("transfer failed"),
                                                }
                                            }
                                            _ => {}
                                        }
                                    } else {
                                        // no match router rule
                                        debug!("no matching router rule, {}", domain);
                                    }
                                }
                                None => {
                                    debug!("no matching endpoint, ignoring {}", domain);
                                }
                            }
                        }
                        None => {
                            // no host, ignore!
                            debug!("request has no host, uri: {}", req.uri());
                        }
                    }
                }
                Some(Err(err)) => {
                    // TODO: fallback to tcp
                    debug!("detect failed, fallback to tcp: {:?}", local_stream.1);
                    bail!("{}", err)
                }
                _ => {}
            }
            Ok(None)
        }
    }
}

impl<T, A> RouterService<T, A>
where
    A: Resolvable,
{
    #[inline]
    fn match_target(&self, host: &String) -> Option<&RouterConfig<A>> {
        self.routes.get(host)
    }
}

pub struct RouterLayer<A> {
    routes: HashMap<String, RouterConfig<A>>,
}

impl<A> RouterLayer<A> {
    pub fn new(routes: HashMap<String, RouterConfig<A>>) -> Self {
        Self { routes }
    }
}

impl<S, A> Layer<S> for RouterLayer<A>
where
    A: Resolvable,
{
    type Service = RouterService<S, A>;

    fn layer(&self, service: S) -> Self::Service {
        RouterService {
            inner: service,
            routes: self.routes.clone(),
        }
    }
}

#[inline]
fn longest_match<'cx>(
    req_path: &'cx str,
    routes: &'cx Vec<RouterRule<Domain>>,
) -> Option<&'cx RouterRule<Domain>> {
    let mut target_route = None;
    let mut route_len = 0;
    for route in routes.iter() {
        let route_path = route.get_path();
        let route_path_len = route_path.len();
        if req_path.starts_with(route_path) && route_path_len > route_len {
            target_route = Some(route);
            route_len = route_path_len;
        }
    }
    target_route
}

#[inline]
fn get_host(req: &Request<Payload>) -> Option<&str> {
    match req.headers().get("host") {
        Some(host) => Some(host.to_str().unwrap_or("")),
        None => None,
    }
}
