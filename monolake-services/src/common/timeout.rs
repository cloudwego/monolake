use std::time::Duration;

use monoio::time::timeout;
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, Service,
};

#[derive(Clone)]
pub struct TimeoutService<T> {
    timeout: Duration,
    inner: T,
}

#[derive(thiserror::Error, Debug)]
pub enum TimeoutError<E> {
    #[error("inner error: {0:?}")]
    Inner(#[from] E),
    #[error("timeout")]
    Timeout,
}

impl<R, T: Service<R>> Service<R> for TimeoutService<T> {
    type Response = T::Response;
    type Error = TimeoutError<T::Error>;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        match timeout(self.timeout, self.inner.call(req)).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(err)) => Err(TimeoutError::Inner(err)),
            Err(_) => Err(TimeoutError::Timeout),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Timeout(pub Duration);

impl<F> TimeoutService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Timeout>,
    {
        layer_fn(|c: &C, inner| TimeoutService {
            timeout: c.param().0,
            inner,
        })
    }
}

impl<F: MakeService> MakeService for TimeoutService<F> {
    type Service = TimeoutService<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(TimeoutService {
            timeout: self.timeout,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
        })
    }
}

impl<F: AsyncMakeService> AsyncMakeService for TimeoutService<F> {
    type Service = TimeoutService<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(TimeoutService {
            timeout: self.timeout,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(Into::into)?,
        })
    }
}
