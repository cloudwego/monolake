use std::{future::Future, time::Duration};

use monoio::time::timeout;
use monolake_core::{
    service::{
        layer::{layer_fn, FactoryLayer},
        MakeService, Param, Service,
    },
    AnyError,
};

#[derive(Debug, Clone, Copy)]
pub struct Timeout(pub Duration);

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

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        R: 'cx;

    fn call(&self, req: R) -> Self::Future<'_> {
        async {
            match timeout(self.timeout, self.inner.call(req)).await {
                Ok(Ok(resp)) => Ok(resp),
                Ok(Err(err)) => Err(err.into()),
                Err(e) => Err(e.into()),
            }
        }
    }
}

impl<F> TimeoutService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Timeout>,
    {
        layer_fn::<C, _, _, _>(|c, inner| TimeoutService {
            timeout: c.param().0,
            inner,
        })
    }
}

impl<F> MakeService for TimeoutService<F>
where
    F: MakeService,
{
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
