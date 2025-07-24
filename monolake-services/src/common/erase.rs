use service_async::{
    AsyncMakeService, MakeService, Service,
    layer::{FactoryLayer, layer_fn},
};

#[derive(Debug)]
pub struct EraseResp<T> {
    pub svc: T,
}

impl<T: MakeService> MakeService for EraseResp<T> {
    type Service = EraseResp<T::Service>;
    type Error = T::Error;

    #[inline]
    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(EraseResp {
            svc: self.svc.make_via_ref(old.map(|o| &o.svc))?,
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
            svc: self.svc.make_via_ref(old.map(|o| &o.svc)).await?,
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
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
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
