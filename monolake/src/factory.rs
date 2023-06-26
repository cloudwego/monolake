//! Preconstructed factories.

use std::fmt::Debug;

use monolake_core::{
    config::ServerConfig,
    context::EmptyContext,
    listener::{AcceptedAddr, AcceptedStream},
};
#[cfg(feature = "openid")]
use monolake_services::http::handlers::OpenIdHandler;
#[cfg(feature = "proxy-protocol")]
use monolake_services::proxy_protocol::ProxyProtocolServiceFactory;
use monolake_services::{
    common::{Accept, ContextService},
    http::{
        handlers::{ConnReuseHandler, ProxyHandler, RewriteHandler},
        HttpCoreService, HttpVersionDetect,
    },
    tls::UnifiedTlsFactory,
};
use service_async::{stack::FactoryStack, MakeService, Service};

/// Create a new factory for l7 proxy.
// Here we use a fixed generic type `Accept<AcceptedStream, AcceptedAddr>`
// for simplification and make return impl work.
pub fn l7_factory(
    config: ServerConfig,
) -> impl MakeService<
    Service = impl Service<Accept<AcceptedStream, AcceptedAddr>, Error = impl Debug>,
    Error = impl Debug,
> {
    let stacks = FactoryStack::new(config)
        .replace(ProxyHandler::factory())
        .push(RewriteHandler::layer());

    #[cfg(feature = "openid")]
    let stacks = stacks.push(OpenIdHandler::layer());

    let stacks = stacks
        .push(ConnReuseHandler::layer())
        .push(HttpCoreService::layer())
        .push(HttpVersionDetect::layer())
        .push(UnifiedTlsFactory::layer());

    #[cfg(feature = "proxy-protocol")]
    let stacks = stacks.push(ProxyProtocolServiceFactory::layer());

    stacks
        .push(ContextService::<EmptyContext, _>::layer())
        .into_inner()
}
