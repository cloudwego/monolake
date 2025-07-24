use std::time::Duration;

use service_async::{
    AsyncMakeService, MakeService, Param, Service,
    layer::{FactoryLayer, layer_fn},
};

#[derive(Clone)]
pub struct DelayService<T> {
    pub delay: Duration,
    pub inner: T,
}

impl<R, T> Service<R> for DelayService<T>
where
    T: Service<R>,
{
    type Response = T::Response;
    type Error = T::Error;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        monoio::time::sleep(self.delay).await;
        self.inner.call(req).await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Delay(pub Duration);

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

impl<F: MakeService> MakeService for DelayService<F> {
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

impl<F: AsyncMakeService> AsyncMakeService for DelayService<F> {
    type Service = DelayService<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(DelayService {
            delay: self.delay,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(Into::into)?,
        })
    }
}
