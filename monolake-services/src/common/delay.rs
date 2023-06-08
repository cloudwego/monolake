use std::{future::Future, time::Duration};

use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};

#[derive(Debug, Clone, Copy)]
pub struct Delay(pub Duration);

#[derive(Clone)]
pub struct DelayService<T> {
    delay: Duration,
    inner: T,
}

impl<R, T> Service<R> for DelayService<T>
where
    T: Service<R>,
{
    type Response = T::Response;

    type Error = T::Error;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        R: 'cx;

    fn call(&self, req: R) -> Self::Future<'_> {
        async {
            monoio::time::sleep(self.delay).await;
            self.inner.call(req).await
        }
    }
}

impl<F> DelayService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Delay>,
    {
        layer_fn(|c: &C, inner| DelayService {
            delay: c.param().0,
            inner,
        })
    }
}

impl<F> MakeService for DelayService<F>
where
    F: MakeService,
{
    type Service = DelayService<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(DelayService {
            delay: self.delay,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
        })
    }
}
