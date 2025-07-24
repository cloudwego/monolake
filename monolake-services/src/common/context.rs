//! Context insertion service for request handling, with support for `certain_map`.
//!
//! This module provides a `ContextService` that inserts context information,
//! into the request processing pipeline. It's designed
//! to work seamlessly with the `service_async` framework and the `certain_map` crate
//! for efficient context management.
//!
//! # Key Components
//!
//! - [`ContextService`]: The main service component that adds context information to requests.
//!
//! # Features
//!
//! - Works with `certain_map` for flexible and type-safe context management
//!
//! # Usage with certain_map
//!
//! `ContextService` is designed to work with contexts defined using the `certain_map` macro.
//! This allows for efficient and type-safe context management. Here's an example of how to
//! define a context and use it with `ContextService`:
//!
//! # Usage in a Service Stack
//!
//! `ContextService` is typically used as part of a larger service stack. Here's an example
//! from a Layer 7 proxy factory:
//!
//! ```ignore
//! use monolake_services::common::ContextService;
//! use service_async::stack::FactoryStack;
//!
//! let stacks = FactoryStack::new(config)
//!         // ... other layers ...
//!         .push(ContextService::<EmptyContext, _>::layer())
//!         // ... more processing ...
//!         ;
//! // ... rest of the factory setup ...
//! ```
//!
//! In this example, `ContextService` is used to transform an `EmptyContext` into a `FullContext`
//! by setting the `peer_addr` field.
use std::marker::PhantomData;

use certain_map::Handler;
use monolake_core::{context::PeerAddr, listener::AcceptedAddr};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, ParamSet, Service,
};

/// A service to insert Context into the request processing pipeline, compatible with `certain_map`.
#[derive(Debug)]
pub struct ContextService<CXStore, T> {
    pub inner: T,
    pub ctx: PhantomData<CXStore>,
}

unsafe impl<CXStore, T: Send> Send for ContextService<CXStore, T> {}
unsafe impl<CXStore, T: Sync> Sync for ContextService<CXStore, T> {}

// Manually impl Clone because CXStore does not have to impl Clone.
impl<CXStore, T> Clone for ContextService<CXStore, T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            ctx: PhantomData,
        }
    }
}

// Manually impl Copy because CXStore does not have to impl Copy.
impl<CXStore, T> Copy for ContextService<CXStore, T> where T: Copy {}

impl<R, T, CXStore, Resp, Err> Service<(R, AcceptedAddr)> for ContextService<CXStore, T>
where
    CXStore: Default + Handler,
    // HRTB is your friend!
    // Please pay attention to when to use bound associated types and when to use associated types
    // directly(here `Transformed` is not bound but `Response` and `Error` are).
    for<'a> CXStore::Hdr<'a>: ParamSet<PeerAddr>,
    for<'a> T: Service<
        (R, <CXStore::Hdr<'a> as ParamSet<PeerAddr>>::Transformed),
        Response = Resp,
        Error = Err,
    >,
{
    type Response = Resp;
    type Error = Err;

    async fn call(&self, (req, addr): (R, AcceptedAddr)) -> Result<Self::Response, Self::Error> {
        let mut store = CXStore::default();
        let hdr = store.handler();
        let hdr = hdr.param_set(PeerAddr(addr));
        self.inner.call((req, hdr)).await
    }
}

impl<CX, F> ContextService<CX, F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_: &C, inner| ContextService {
            inner,
            ctx: PhantomData,
        })
    }
}

impl<CXStore, F: MakeService> MakeService for ContextService<CXStore, F> {
    type Service = ContextService<CXStore, F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(ContextService {
            ctx: PhantomData,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .map_err(Into::into)?,
        })
    }
}

impl<CXStore, F: AsyncMakeService> AsyncMakeService for ContextService<CXStore, F> {
    type Service = ContextService<CXStore, F::Service>;
    type Error = F::Error;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        Ok(ContextService {
            ctx: PhantomData,
            inner: self
                .inner
                .make_via_ref(old.map(|o| &o.inner))
                .await
                .map_err(Into::into)?,
        })
    }
}
