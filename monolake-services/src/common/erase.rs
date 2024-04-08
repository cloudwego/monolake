use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Service,
};

#[derive(Debug)]
pub struct EraseResp<T> {
    svc: T,
}

impl<T: MakeService> MakeService for EraseResp<T> {
    type Service = EraseResp<T::Service>;
    type Error = T::Error;

    #[inline]
    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(EraseResp {
            svc: self
                .svc
                .make_via_ref(old.map(|o| &o.svc))
                .map_err(Into::into)?,
        })
    }
}

impl<T: AsyncMakeService> AsyncMakeService for EraseResp<T> {
    type Service = EraseResp<T::Service>;
    type Error = T::Error;

    #[inline]
    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(EraseResp {
            svc: self
                .svc
                .make_via_ref(old.map(|o| &o.svc))
                .await
                .map_err(Into::into)?,
        })
    }
}

impl<T: Service<Req>, Req> Service<Req> for EraseResp<T> {
    type Response = ();
    type Error = T::Error;

    #[inline]
    async fn call(&self, req: Req) -> Result<Self::Response, Self::Error> {
        self.svc.call(req).await.map(|_| ())
    }
}

impl<F> EraseResp<F> {
    pub fn layer<C>(&self) -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_c: &C, svc| EraseResp { svc })
    }
}

impl<T> EraseResp<T> {
    #[inline]
    pub const fn new(svc: T) -> Self {
        Self { svc }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.svc
    }
}

#[monoio::test_all]
async fn test_erase() {
    use crate::common::delay::{ DummyService, Delay};

    let es:DummyService = DummyService{};
    let d = 1;
    let s:EraseResp<DummyService> = EraseResp::new(es);
    let s2 = MakeService::make(&s).unwrap();
    let s3 = AsyncMakeService::make(&s).await.unwrap();
    let _ = s.layer::<Delay>();
    let _ = s2.call(&s).await;
    let _ = s.call(&s).await;
    let _ = s3.call(&s).await;
    let _ = s.into_inner();

    assert_eq!(d, 1);
}
