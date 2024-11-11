use std::{cell::Cell, convert::Infallible};

use service_async::Service;

/// Generic synchronous selector.
///
/// It abstracts the way to select a service or endpoint, including routing and load balancing.
pub trait Selector<K> {
    /// Select output which can be a reference or a owned type.
    ///
    /// When the usage style is like select a Service and call it, the output can be a reference.
    /// When the usage style is like select something like address, and then call the Service, the
    /// output can be owned if the address is a temporary value.
    ///
    /// Note you may use HRTB to put restrictions on the output type because of GAT.
    type Output<'a>
    where
        Self: 'a;
    type Error;

    fn select(&self, key: &K) -> Result<Self::Output<'_>, Self::Error>;
}

#[derive(Debug, Clone, Copy)]
pub struct EmptyCollectionError;
impl std::fmt::Display for EmptyCollectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "empty collection error")
    }
}

pub struct RandomSelector<T>(pub Vec<T>);

impl<T, A> Selector<A> for RandomSelector<T> {
    type Output<'a>
        = &'a T
    where
        Self: 'a;
    type Error = EmptyCollectionError;

    fn select(&self, _key: &A) -> Result<Self::Output<'_>, Self::Error> {
        if self.0.len() == 1 {
            return Ok(&self.0[0]);
        }

        use rand::seq::SliceRandom;
        self.0
            .choose(&mut rand::thread_rng())
            .ok_or(EmptyCollectionError)
    }
}

pub struct RoundRobinSelector<T> {
    collection: Vec<T>,
    next_idx: Cell<usize>,
}

impl<T> RoundRobinSelector<T> {
    /// Create a new RoundRobinSelector.
    pub fn new(collection: Vec<T>) -> Result<Self, EmptyCollectionError> {
        if collection.is_empty() {
            return Err(EmptyCollectionError);
        }
        Ok(Self {
            collection,
            next_idx: Cell::new(0),
        })
    }
}

impl<T, A> Selector<A> for RoundRobinSelector<T> {
    type Output<'a>
        = &'a T
    where
        Self: 'a;
    type Error = Infallible;

    fn select(&self, _key: &A) -> Result<Self::Output<'_>, Self::Error> {
        let idx = self.next_idx.get();
        self.next_idx.set((idx + 1) % self.collection.len());
        Ok(&self.collection[idx])
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SelectError<ESEL, ESVC> {
    #[error("selector error: {0:?}")]
    SelectorError(ESEL),
    #[error("service error: {0:?}")]
    ServiceError(ESVC),
}

/// Dispatch service based on the selector.
///
/// The selector's output is the target service.
/// This is useful when you want to dispatch request to multiple pre-constructed services.
pub struct SvcDispatch<S>(pub S);

impl<SEL, R, SR, SE, SELE> Service<R> for SvcDispatch<SEL>
where
    SEL: Selector<R, Error = SELE>,
    for<'a> SEL::Output<'a>: Service<R, Response = SR, Error = SE>,
{
    type Response = SR;
    type Error = SelectError<SELE, SE>;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        let svc = self.0.select(&req).map_err(SelectError::SelectorError)?;
        svc.call(req).await.map_err(SelectError::ServiceError)
    }
}

/// Route service based on the selector.
///
/// Get the selector output and call the service with (Req, Out).
pub struct SvcRoute<SVC, SEL> {
    pub svc: SVC,
    pub selector: SEL,
}

impl<SVC, SEL, R, SVCR, SVCE, SELE> Service<R> for SvcRoute<SVC, SEL>
where
    SEL: Selector<R, Error = SELE>,
    for<'a> SVC: Service<(R, SEL::Output<'a>), Response = SVCR, Error = SVCE>,
{
    type Response = SVCR;
    type Error = SelectError<SELE, SVCE>;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        let sel = self
            .selector
            .select(&req)
            .map_err(SelectError::SelectorError)?;

        self.svc
            .call((req, sel))
            .await
            .map_err(SelectError::ServiceError)
    }
}
