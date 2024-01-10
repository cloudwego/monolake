use monolake_core::{context::PeerAddr, listener::AcceptedAddr};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, ParamSet, Service,
};

/// A service to insert Context
/// The Context will be forked from factory and PeerAddr will be set into it.
#[derive(Debug, Clone, Copy)]
pub struct ContextService<CX, T> {
    inner: T,
    ctx: CX,
}

impl<R, T, CX> Service<(R, AcceptedAddr)> for ContextService<CX, T>
where
    T: Service<(R, CX::Transformed)>,
    CX: ParamSet<PeerAddr> + Clone,
{
    type Response = T::Response;
    type Error = T::Error;

    async fn call(&self, (req, addr): (R, AcceptedAddr)) -> Result<Self::Response, Self::Error> {
        let ctx = self.ctx.clone().param_set(PeerAddr(addr));
        self.inner.call((req, ctx)).await
    }
}

impl<CX, F> ContextService<CX, F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        CX: Default,
    {
        layer_fn(|_: &C, inner| ContextService {
            inner,
            ctx: Default::default(),
        })
    }
}

impl<CX: Clone, F: MakeService> MakeService for ContextService<CX, F> {
    type Service = ContextService<CX, F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ContextService {
            ctx: self.ctx.clone(),
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
        })
    }
}

impl<CX: Clone, F: AsyncMakeService> AsyncMakeService for ContextService<CX, F> {
    type Service = ContextService<CX, F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(ContextService {
            ctx: self.ctx.clone(),
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(Into::into)?,
        })
    }
}
