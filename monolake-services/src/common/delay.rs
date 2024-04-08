use std::time::Duration;

use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Param, Service,
};

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

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        monoio::time::sleep(self.delay).await;
        self.inner.call(req).await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Delay(pub Duration);

impl<F> DelayService<F> {
    pub fn layer<C>(&self) -> impl FactoryLayer<C, F, Factory = Self>
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

#[derive(Copy, Clone)]
pub struct DummyService
{
}

impl <R> Service<R> for DummyService
{
    type Response = R;
    type Error = std::io::Error;
    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        Ok(req)
    }
}

impl MakeService for DummyService {
    type Service = Self;
    type Error = std::io::Error;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(DummyService {
        })
    }
}

impl AsyncMakeService for DummyService {
    type Service = Self;
    type Error = std::io::Error;

    async fn make_via_ref(
        &self,
        _old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(DummyService {
        })
    }
}

#[monoio::test_all(timer_enabled = true)]
async fn test_delay() {
    use std::time::Duration;

    let es:DummyService = DummyService{};
    let s:DelayService<DummyService> = DelayService {
        delay: Duration::from_secs(1),
        inner: es,
    };
    let s2 = MakeService::make(&s).unwrap();
    let s3 = AsyncMakeService::make(&s).await.unwrap();
    let _ = s.layer::<Delay>();
    let _ = s2.call(&s).await;
    let _ = s.call(&s).await;
    let _ = s3.call(&s).await;

    assert_eq!(s.delay, Duration::from_secs(1));
}
