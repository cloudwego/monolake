//! Timeout service for adding timeout functionality to HTTP handlers.
//!
//! This module provides a `TimeoutService` that wraps an inner service and applies
//! a timeout to its execution. It's designed to work seamlessly with the `service_async`
//! framework and can be easily integrated into a service stack.
//!
//! # Key Components
//!
//! - [`TimeoutService`]: The main service component that adds timeout functionality to an inner
//!   service.
//! - [`TimeoutError`]: Error type for timeout-related errors.
//! - [`Timeout`]: A simple wrapper around `Duration` for configuration purposes.
//!
//! # Features
//!
//! - Adds configurable timeout to any inner service
//! - Propagates inner service errors alongside timeout errors
//!
//! # Performance Considerations
//!
//! - Adds minimal overhead to the inner service execution
//! - Uses efficient timeout mechanism provided by the `monoio` runtime

use std::time::Duration;

use monoio::time::timeout;
use service_async::{
    AsyncMakeService, MakeService, Param, Service,
    layer::{FactoryLayer, layer_fn},
};

/// Service that adds timeout functionality to an inner service.
#[derive(Clone)]
pub struct TimeoutService<T> {
    pub timeout: Duration,
    pub inner: T,
}

#[derive(thiserror::Error, Debug)]
pub enum TimeoutError<E> {
    #[error("inner error: {0:?}")]
    Inner(E),
    #[error("timeout")]
    Timeout,
}

impl<R, T: Service<R>> Service<R> for TimeoutService<T> {
    type Response = T::Response;
    type Error = TimeoutError<T::Error>;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        match timeout(self.timeout, self.inner.call(req)).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(err)) => Err(TimeoutError::Inner(err)),
            Err(_) => Err(TimeoutError::Timeout),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Timeout(pub Duration);

impl<F> TimeoutService<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<Timeout>,
    {
        layer_fn(|c: &C, inner| TimeoutService {
            timeout: c.param().0,
            inner,
        })
    }
}

impl<F: MakeService> MakeService for TimeoutService<F> {
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

impl<F: AsyncMakeService> AsyncMakeService for TimeoutService<F> {
    type Service = TimeoutService<F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(TimeoutService {
            timeout: self.timeout,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(Into::into)?,
        })
    }
}
