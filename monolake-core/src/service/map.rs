use std::future::Future;

use super::{MakeService, Service};

pub trait MapTarget<T> {
    type Target;

    fn map_target(&self, t: T) -> Self::Target;
}

impl<F, T, U> MapTarget<T> for F
where
    F: Fn(T) -> U,
{
    type Target = U;
    fn map_target(&self, t: T) -> U {
        (self)(t)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KeepFirst;

impl<A, B> MapTarget<(A, B)> for KeepFirst {
    type Target = A;

    fn map_target(&self, t: (A, B)) -> Self::Target {
        t.0
    }
}

pub struct MapTargetService<T, F> {
    pub f: F,
    pub inner: T,
}

impl<T, F, R> Service<R> for MapTargetService<T, F>
where
    F: MapTarget<R>,
    T: Service<F::Target>,
{
    type Response = T::Response;

    type Error = T::Error;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        R: 'cx;

    fn call(&self, req: R) -> Self::Future<'_> {
        let req = self.f.map_target(req);
        self.inner.call(req)
    }
}

impl<FAC, F> MakeService for MapTargetService<FAC, F>
where
    FAC: MakeService,
    F: Clone,
{
    type Service = MapTargetService<FAC::Service, F>;
    type Error = FAC::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(MapTargetService {
            f: self.f.clone(),
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
        })
    }
}
