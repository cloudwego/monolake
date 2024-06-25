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
use monolake_core::{context::PeerAddr, listener::AcceptedAddr};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    AsyncMakeService, MakeService, ParamSet, Service,
};

/// A service to insert Context into the request processing pipeline, compatible with `certain_map`.
#[derive(Debug, Clone, Copy)]
pub struct ContextService<CX, T> {
    pub inner: T,
    pub ctx: CX,
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
