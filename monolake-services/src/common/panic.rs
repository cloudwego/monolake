use std::{any::Any, panic::AssertUnwindSafe};

use futures::FutureExt;
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, Service,
};

pub struct CatchPanicService<S> {
    inner: S,
}

/// `CatchPanicService` is designed to prevent a panic from causing
/// the entire program to crash by converting the panic into an error.
/// It's important to note that the user must ensure that the inner service
/// is 'UnwindSafe' before using this service. If the inner service is not
/// 'UnwindSafe', the behavior is undefined.
impl<R, S> Service<R> for CatchPanicService<S>
where
    S: Service<R>,
    S::Error: From<Box<dyn Any + Send + 'static>>,
{
    type Response = S::Response;
    type Error = S::Error;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        match AssertUnwindSafe(self.inner.call(req)).catch_unwind().await {
            Ok(r) => r,
            Err(e) => Err(e.into()),
        }
    }
}

impl<F> CatchPanicService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_c: &C, inner| CatchPanicService { inner })
    }
}

impl<F: MakeService> MakeService for CatchPanicService<F> {
    type Service = CatchPanicService<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(CatchPanicService {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
        })
    }
}

impl<F: AsyncMakeService> AsyncMakeService for CatchPanicService<F> {
    type Service = CatchPanicService<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(CatchPanicService {
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(Into::into)?,
        })
    }
}
