use std::{future::Future, net::SocketAddr, rc::Rc};

use anyhow::bail;
use log::info;
use monoio::net::{TcpListener, TcpStream};
use monolake_core::{
    service::ServiceError,
    service::{Service, ServiceLayer},
};
use tower_layer::{layer_fn, Layer};

use crate::common::Accept;

#[derive(Default, Clone)]
pub struct TcpListenerService;

impl Service<Rc<TcpListener>> for TcpListenerService {
    type Response = Accept<TcpStream, SocketAddr>;

    type Error = ServiceError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>>
    where
        Self: 'cx;

    fn call(&self, listener: Rc<TcpListener>) -> Self::Future<'_> {
        async move {
            match listener.accept().await {
                Ok(accept) => {
                    info!("accept a tcp connection");
                    return Ok(accept);
                }
                Err(err) => bail!("{}", err),
            }
        }
    }
}

impl<S> ServiceLayer<S> for TcpListenerService {
    type Layer = impl Layer<S, Service = Self>;
    type Param = ();

    fn layer(_: Self::Param) -> Self::Layer {
        layer_fn(move |_: S| TcpListenerService)
    }
}
