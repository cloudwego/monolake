use service_async::{
    AsyncMakeService, MakeService, Service,
    layer::{FactoryLayer, layer_fn},
};

pub struct Map<S, FN> {
    pub inner: S,
    pub rewrite_f: FN,
}

pub struct MapErr<S, FN> {
    pub inner: S,
    pub rewrite_f: FN,
}

pub struct FnSvc<S, FN> {
    pub inner: S,
    pub rewrite_f: FN,
}

impl<F, FN: Clone + 'static> Map<F, FN> {
    pub fn layer<C>(f: FN) -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(move |_c: &C, inner| Map {
            inner,
            rewrite_f: f.clone(),
        })
    }
}

impl<F, FN: Clone + 'static> MapErr<F, FN> {
    pub fn layer<C>(f: FN) -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(move |_c: &C, inner| MapErr {
            inner,
            rewrite_f: f.clone(),
        })
    }
}

impl<F, FN: Clone + 'static> FnSvc<F, FN> {
    pub fn layer<C>(f: FN) -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(move |_c: &C, inner| FnSvc {
            inner,
            rewrite_f: f.clone(),
        })
    }
}

impl<S: Service<R>, R, FN, NR> Service<R> for Map<S, FN>
where
    FN: Fn(S::Response) -> NR,
{
    type Response = NR;
    type Error = S::Error;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        self.inner.call(req).await.map(&self.rewrite_f)
    }
}

impl<S: Service<R>, R, FN, NE> Service<R> for MapErr<S, FN>
where
    FN: Fn(S::Error) -> NE,
{
    type Response = S::Response;
    type Error = NE;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        self.inner.call(req).await.map_err(&self.rewrite_f)
    }
}

impl<S: Service<R>, R, FN, FR, FE> Service<R> for FnSvc<S, FN>
where
    FN: Fn(Result<S::Response, S::Error>) -> Result<FR, FE>,
{
    type Response = FR;
    type Error = FE;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        (self.rewrite_f)(self.inner.call(req).await)
    }
}

impl<F: AsyncMakeService, FN: Clone> AsyncMakeService for Map<F, FN> {
    type Service = Map<F::Service, FN>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(Map {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(Into::into)?,
            rewrite_f: self.rewrite_f.clone(),
        })
    }
}

impl<F: AsyncMakeService, FN: Clone> AsyncMakeService for MapErr<F, FN> {
    type Service = MapErr<F::Service, FN>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(MapErr {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(Into::into)?,
            rewrite_f: self.rewrite_f.clone(),
        })
    }
}

impl<F: AsyncMakeService, FN: Clone> AsyncMakeService for FnSvc<F, FN> {
    type Service = FnSvc<F::Service, FN>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(FnSvc {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(Into::into)?,
            rewrite_f: self.rewrite_f.clone(),
        })
    }
}

impl<F: MakeService, FN: Clone> MakeService for Map<F, FN> {
    type Service = Map<F::Service, FN>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(Map {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
            rewrite_f: self.rewrite_f.clone(),
        })
    }
}

impl<F: MakeService, FN: Clone> MakeService for MapErr<F, FN> {
    type Service = MapErr<F::Service, FN>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(MapErr {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
            rewrite_f: self.rewrite_f.clone(),
        })
    }
}

impl<F: MakeService, FN: Clone> MakeService for FnSvc<F, FN> {
    type Service = FnSvc<F::Service, FN>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(FnSvc {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
            rewrite_f: self.rewrite_f.clone(),
        })
    }
}
