//! Panic-catching service for enhancing stability in handlerss.
//!
//! This module provides a `CatchPanicService` that wraps an inner service and catches
//! any panics that might occur during its execution, converting them into errors.
//! It's designed to work seamlessly with the `service_async` framework and can be
//! easily integrated into a service stack to improve overall system stability.
//!
//! # Key Components
//!
//! - [`CatchPanicService`]: The main service component that adds panic-catching functionality to an
//!   inner service.
//! - [`CatchPanicError`]: Error type that encapsulates both inner service errors and caught panics.
//!
//! # Features
//!
//! - Catches panics in the inner service and converts them to errors
//! - Preserves inner service errors alongside panic-derived errors
//!
//! # Usage
//!
//! `CatchPanicService` is typically used as part of a larger service stack. Here's a basic example:
//!
//! ```ignore
//! use service_async::{layer::FactoryLayer, stack::FactoryStack};
//!
//! use crate::catch_panic::CatchPanicService;
//!
//! let config = Config {
//!     // ... config ...
//! };
//! let stack = FactoryStack::new(config)
//!     .push(MyService::layer())
//!     .push(CatchPanicService::layer())
//!     // ... other layers ...
//!     ;
//!
//! let service = stack.make_async().await.unwrap();
//! // Use the service to handle requests with panic protection
//! ```
//!
//! # Safety Considerations
//!
//! It's crucial to ensure that the inner service wrapped by `CatchPanicService` is
//! `UnwindSafe`. If the inner service is not `UnwindSafe`, the behavior of
//! `CatchPanicService` is undefined and may lead to unexpected results.
//!
//! # Error Handling
//!
//! The `CatchPanicService` wraps errors from the inner service and adds a new `Panic`
//! error variant for caught panics. Users should handle both inner service errors
//! and panic-derived errors when using this service.
//!
//! # Performance Considerations
//!
//! - Adds minimal overhead to the inner service execution
//! - Uses Rust's `catch_unwind` mechanism, which has a small performance cost

use std::{fmt::Debug, panic::AssertUnwindSafe};

use futures::FutureExt;
use service_async::{
    AsyncMakeService, MakeService, Service,
    layer::{FactoryLayer, layer_fn},
};

pub struct CatchPanicService<S> {
    pub inner: S,
}

#[derive(thiserror::Error, Debug)]
pub enum CatchPanicError<E> {
    #[error("inner error: {0:?}")]
    Inner(E),
    // to make it Sync, construct a String instead of Box<dyn Ayn + Send>
    #[error("inner panic: {0}")]
    Panic(String),
}

// Service that catches panics from an inner service and converts them to errors.
/// # Safety
///
/// The inner service must be `UnwindSafe` for this wrapper to function correctly.
/// Using `CatchPanicService` with a non-`UnwindSafe` inner service may lead to
/// undefined behavior.
impl<R, S> Service<R> for CatchPanicService<S>
where
    S: Service<R>,
{
    type Response = S::Response;
    type Error = CatchPanicError<S::Error>;

    async fn call(&self, req: R) -> Result<Self::Response, Self::Error> {
        match AssertUnwindSafe(self.inner.call(req)).catch_unwind().await {
            Ok(Ok(r)) => Ok(r),
            Ok(Err(e)) => Err(CatchPanicError::Inner(e)),
            Err(e) => Err(CatchPanicError::Panic(format!("{e:?}"))),
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
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
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
            inner: self.inner.make_via_ref(old.map(|o| &o.inner)).await?,
        })
    }
}
