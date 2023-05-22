use std::{fmt::Display, future::Future, time::Duration};

use monoio::time::timeout;
use monolake_core::{
    service::ServiceError,
    service::{Service, ServiceLayer},
};
use tower_layer::{layer_fn, Layer};

#[derive(Clone)]
pub struct TimeoutService<T> {
    inner: T,
    timeout: Duration,
}

impl<R, T> Service<R> for TimeoutService<T>
where
    T: Service<R>,
    T::Error: Display,
    R: 'static,
{
    type Response = Option<T::Response>;

    type Error = ServiceError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx;

    fn call(&self, req: R) -> Self::Future<'_> {
        async {
            match timeout(self.timeout, self.inner.call(req)).await {
                Ok(Ok(resp)) => Ok(Some(resp)),
                Ok(Err(err)) => Err(anyhow::anyhow!("{}", err)),
                Err(_) => Err(anyhow::anyhow!("timeout")),
            }
        }
    }
}

impl<S> ServiceLayer<S> for TimeoutService<S> {
    type Param = Duration;
    type Layer = impl Layer<S, Service = Self>;

    fn layer(timeout: Self::Param) -> Self::Layer {
        layer_fn(move |inner| TimeoutService {
            inner,
            timeout: timeout.clone(),
        })
    }
}
