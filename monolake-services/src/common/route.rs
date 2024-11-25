use std::{cell::Cell, convert::Infallible};

use monolake_core::http::HttpError;
pub use rand::distributions::WeightedError;
use rand::{
    distributions::uniform::{SampleBorrow, SampleUniform},
    prelude::Distribution,
};
use serde::{Deserialize, Serialize};
use service_async::Service;

/// Generic synchronous selector.
///
/// It abstracts the way to select a service or endpoint, including routing and load balancing.
pub trait Selector<K: ?Sized> {
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

/// Randomly select an element from a collection.
#[derive(Debug, Clone)]
pub struct RandomSelector<T>(Vec<T>);

impl<T> RandomSelector<T> {
    /// Create a new RandomSelector.
    pub fn new(collection: Vec<T>) -> Result<Self, EmptyCollectionError> {
        if collection.is_empty() {
            return Err(EmptyCollectionError);
        }
        Ok(Self(collection))
    }
}

impl<T, A: ?Sized> Selector<A> for RandomSelector<T> {
    type Output<'a>
        = &'a T
    where
        Self: 'a;
    type Error = Infallible;

    fn select(&self, _key: &A) -> Result<Self::Output<'_>, Self::Error> {
        if self.0.len() == 1 {
            return Ok(&self.0[0]);
        }

        use rand::seq::SliceRandom;
        Ok(self.0.choose(&mut rand::thread_rng()).unwrap())
    }
}

/// Weighted random selector.
pub struct WeightedRandomSelector<T, X: SampleUniform + PartialOrd> {
    collection: Vec<T>,
    dist: rand::distributions::WeightedIndex<X>,
}

impl<T: std::fmt::Debug, X: SampleUniform + PartialOrd> std::fmt::Debug
    for WeightedRandomSelector<T, X>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeightedRandomSelector")
            .field("collection", &self.collection)
            .finish()
    }
}

struct MapLast<'a, I, X>(I, &'a mut Vec<X>);
impl<I, X, Y> Iterator for MapLast<'_, I, X>
where
    I: Iterator<Item = (X, Y)>,
{
    type Item = Y;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(x, y)| {
            self.1.push(x);
            y
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

struct Count<'a, I>(I, &'a mut usize);
impl<I> Iterator for Count<'_, I>
where
    I: Iterator,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let r = self.0.next();
        if r.is_some() {
            *self.1 += 1;
        }
        r
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<T, X: SampleUniform + PartialOrd> WeightedRandomSelector<T, X> {
    /// Create a new WeightedRandomSelector from elements vec and weights.
    ///
    /// Note: caller must make sure the weights have the same length as the elements and in the same
    /// order. Otherwise, it will take the minimum length of the two.
    pub fn new<I>(mut collection: Vec<T>, weights: I) -> Result<Self, WeightedError>
    where
        I: IntoIterator,
        I::Item: SampleBorrow<X>,
        X: for<'a> ::core::ops::AddAssign<&'a X> + Clone + Default,
    {
        let mut cnt = 0;
        let weights = Count(weights.into_iter().take(collection.len()), &mut cnt);
        let dist = rand::distributions::WeightedIndex::new(weights)?;
        while collection.len() > cnt {
            collection.pop();
        }
        Ok(Self { collection, dist })
    }

    /// Create a new WeightedRandomSelector from an iterator of elements and weights.
    pub fn new_from_iter(input: impl Iterator<Item = (T, X)>) -> Result<Self, WeightedError>
    where
        X: for<'a> ::core::ops::AddAssign<&'a X> + Clone + Default,
    {
        let mut collection = Vec::with_capacity(input.size_hint().0);
        let it = MapLast(input, &mut collection);
        let dist = rand::distributions::WeightedIndex::new(it)?;
        Ok(Self { collection, dist })
    }
}

impl<T, X: SampleUniform + PartialOrd, A: ?Sized> Selector<A> for WeightedRandomSelector<T, X> {
    type Output<'a>
        = &'a T
    where
        Self: 'a;
    type Error = Infallible;

    fn select(&self, _key: &A) -> Result<Self::Output<'_>, Self::Error> {
        let idx = self.dist.sample(&mut rand::thread_rng());
        Ok(&self.collection[idx])
    }
}

/// Round-robin selector.
#[derive(Debug, Clone)]
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

impl<T, A: ?Sized> Selector<A> for RoundRobinSelector<T> {
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

/// Identity selector. It always returns the same item.
#[derive(Debug, Clone)]
pub struct IdentitySelector<T>(pub T);

impl<T, A: ?Sized> Selector<A> for IdentitySelector<T> {
    type Output<'a>
        = &'a T
    where
        Self: 'a;
    type Error = Infallible;

    fn select(&self, _key: &A) -> Result<Self::Output<'_>, Self::Error> {
        Ok(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalanceStrategy {
    #[default]
    Random,
    WeightedRandom,
    RoundRobin,
    First,
}

#[derive(thiserror::Error, Debug)]
pub enum LoadBalanceError {
    #[error("empty upstream")]
    EmptyUpstream,
    #[error("invalid weight")]
    InvalidWeight(#[from] WeightedError),
}

impl From<EmptyCollectionError> for LoadBalanceError {
    #[inline]
    fn from(_: EmptyCollectionError) -> Self {
        Self::EmptyUpstream
    }
}

#[derive(Debug)]
pub enum LoadBalancer<T> {
    Random(RandomSelector<T>),
    WeightedRandom(WeightedRandomSelector<T, u16>),
    RoundRobin(RoundRobinSelector<T>),
    Identity(IdentitySelector<T>),
}

pub trait IntoWeightedEndpoint {
    type Endpoint;
    fn into_weighted_endpoint(self) -> (Self::Endpoint, u16);
}

impl<T> LoadBalancer<T> {
    pub fn try_from_upstreams<U>(
        lb: LoadBalanceStrategy,
        upstreams: impl IntoIterator<Item = U>,
    ) -> Result<Self, LoadBalanceError>
    where
        U: IntoWeightedEndpoint<Endpoint = T>,
    {
        let mut it = upstreams.into_iter();
        Ok(match lb {
            LoadBalanceStrategy::Random => {
                RandomSelector::new(it.map(|up| up.into_weighted_endpoint().0).collect())
                    .map(LoadBalancer::Random)?
            }
            LoadBalanceStrategy::WeightedRandom => {
                struct WeightedIter<I>(I);
                impl<I: Iterator, EP> Iterator for WeightedIter<I>
                where
                    I::Item: IntoWeightedEndpoint<Endpoint = EP>,
                {
                    type Item = (EP, u16);
                    fn next(&mut self) -> Option<Self::Item> {
                        self.0.next().map(|up| up.into_weighted_endpoint())
                    }
                }
                WeightedRandomSelector::new_from_iter(WeightedIter(it))
                    .map(LoadBalancer::WeightedRandom)?
            }
            LoadBalanceStrategy::RoundRobin => {
                RoundRobinSelector::new(it.map(|up| up.into_weighted_endpoint().0).collect())
                    .map(LoadBalancer::RoundRobin)?
            }
            LoadBalanceStrategy::First => {
                let Some(up) = it.next() else {
                    return Err(LoadBalanceError::EmptyUpstream);
                };
                LoadBalancer::Identity(IdentitySelector(up.into_weighted_endpoint().0))
            }
        })
    }
}

impl<T, A: ?Sized> Selector<A> for LoadBalancer<T> {
    type Output<'a>
        = &'a T
    where
        Self: 'a;
    type Error = Infallible;

    #[inline]
    fn select(&self, key: &A) -> Result<Self::Output<'_>, Self::Error> {
        match self {
            LoadBalancer::Random(random_selector) => random_selector.select(key),
            LoadBalancer::WeightedRandom(wr_selector) => wr_selector.select(key),
            LoadBalancer::RoundRobin(round_robin_selector) => round_robin_selector.select(key),
            LoadBalancer::Identity(identity_selector) => identity_selector.select(key),
        }
    }
}

/// Error type for SvcRoute to indicate the error from selector or service.
#[derive(thiserror::Error, Debug)]
pub enum SelectError<ESEL, ESVC> {
    #[error("selector error: {0:?}")]
    SelectorError(ESEL),
    #[error("service error: {0:?}")]
    ServiceError(ESVC),
}

impl<B, ESEL: HttpError<B>, ESVC: HttpError<B>> HttpError<B> for SelectError<ESEL, ESVC> {
    #[inline]
    fn to_response(&self) -> Option<http::Response<B>> {
        match self {
            SelectError::SelectorError(e) => e.to_response(),
            SelectError::ServiceError(e) => e.to_response(),
        }
    }
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
pub struct SvcRoute<SEL, SVC, F> {
    pub selector: SEL,
    pub selector_mapper: F,
    pub svc: SVC,
}

pub trait Mapping<In> {
    type Out: ?Sized;
    fn map<'a>(&self, input: &'a In) -> &'a Self::Out;
}

impl<SVC, SEL, F, R, SVCR, SVCE, CX> Service<(R, CX)> for SvcRoute<SEL, SVC, F>
where
    F: Mapping<R>,
    SEL: Selector<F::Out>,
    for<'a> SVC: Service<(R, SEL::Output<'a>, CX), Response = SVCR, Error = SVCE>,
{
    type Response = SVCR;
    type Error = SelectError<SEL::Error, SVCE>;

    async fn call(&self, (req, cx): (R, CX)) -> Result<Self::Response, Self::Error> {
        let req_transformed = self.selector_mapper.map(&req);
        let sel_out = self
            .selector
            .select(req_transformed)
            .map_err(SelectError::SelectorError)?;

        self.svc
            .call((req, sel_out, cx))
            .await
            .map_err(SelectError::ServiceError)
    }
}
