use std::{future::Future, rc::Rc};

use anyhow::bail;
use log::info;
use monoio::net::{unix::SocketAddr, UnixListener, UnixStream};
use monolake_core::{
    service::ServiceError,
    service::{Service, ServiceLayer},
};
use tower_layer::{layer_fn, Layer};

use crate::common::Accept;

#[derive(Default, Clone)]
pub struct UdsListenerService;

impl Service<Rc<UnixListener>> for UdsListenerService {
    type Response = Accept<UnixStream, SocketAddr>;

    type Error = ServiceError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>>
    where
        Self: 'cx;

    fn call(&self, listener: Rc<UnixListener>) -> Self::Future<'_> {
        async move {
            match listener.accept().await {
                Ok(accept) => {
                    info!("Accept a uds connection");
                    return Ok(accept);
                }
                Err(err) => bail!("{}", err),
            }
        }
    }
}

impl<S> ServiceLayer<S> for UdsListenerService {
    type Layer = impl Layer<S, Service = Self>;
    type Param = ();

    fn layer(_: Self::Param) -> Self::Layer {
        layer_fn(move |_: S| UdsListenerService)
    }
}
