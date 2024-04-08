use std::time::Duration;

use monoio::time::timeout;
use monolake_core::AnyError;
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, Service,
};

#[derive(Clone)]
pub struct TimeoutService<T> {
    timeout: Duration,
    inner: T,
}

impl<R, T> Service<R> for TimeoutService<T>
where
    T: Service<R>,
    T::Error: Into<AnyError>,
{
    type Response = T::Response;
    type Error = AnyError;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        match timeout(self.timeout, self.inner.call(req)).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(err)) => Err(err.into()),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Timeout(pub Duration);

impl<F> TimeoutService<F> {
    pub fn layer<C>(&self) -> impl FactoryLayer<C, F, Factory = Self>
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

#[monoio::test_all(timer_enabled = true)]
async fn test_erase() {
    use std::time::Duration;
    use crate::common::delay::DummyService;

    let es:DummyService = DummyService{};
    let s:TimeoutService<DummyService> = TimeoutService {
        timeout: Duration::from_secs(1),
        inner: es,
    };
    let _s2 = MakeService::make(&s).unwrap();
    let s3 = AsyncMakeService::make(&s).await.unwrap();
    let _ = s.layer::<Timeout>();
    let _ = s3.call(&s).await;

    assert_eq!(s.timeout, Duration::from_secs(1));
}
