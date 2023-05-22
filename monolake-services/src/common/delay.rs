use std::{future::Future, time::Duration};

use monolake_core::service::{Service, ServiceLayer};
use tower_layer::{layer_fn, Layer};

#[derive(Clone)]
pub struct DelayService<T> {
    inner: T,
    duration: Duration,
}

impl<R, T> Service<R> for DelayService<T>
where
    T: Service<R>,
    R: 'static,
{
    type Response = T::Response;

    type Error = T::Error;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx;

    fn call(&self, req: R) -> Self::Future<'_> {
        async {
            monoio::time::sleep(self.duration).await;
            self.inner.call(req).await
        }
    }
}

impl<S> ServiceLayer<S> for DelayService<S> {
    type Param = Duration;
    type Layer = impl Layer<S, Service = Self>;
    fn layer(param: Self::Param) -> Self::Layer {
        layer_fn(move |inner| DelayService {
            inner,
            duration: param,
        })
    }
}
