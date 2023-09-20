//! Preconstructed factories.

use std::fmt::Debug;

use monoio::net::TcpStream;
use monolake_core::listener::{AcceptedAddr, AcceptedStream};
#[cfg(feature = "openid")]
use monolake_services::http::handlers::OpenIdHandler;
#[cfg(feature = "proxy-protocol")]
use monolake_services::proxy_protocol::ProxyProtocolServiceFactory;
use monolake_services::{
    common::ContextService,
    http::{
        handlers::{ConnReuseHandler, ProxyHandler, RewriteHandler},
        HttpCoreService, HttpVersionDetect, H2THandler,
    },
    tcp::Accept,
};
use service_async::{stack::FactoryStack, ArcMakeService, Service};

use crate::{
    config::ServerConfig,
    context::{EmptyContext, FullContext},
};

/// Create a new factory for l7 proxy.
// Here we use a fixed generic type `Accept<AcceptedStream, AcceptedAddr>`
// for simplification and make return impl work.
pub fn l7_factory(
    config: ServerConfig,
) -> ArcMakeService<
    impl Service<Accept<AcceptedStream, AcceptedAddr>, Error = impl Debug>,
    impl Debug,
> {
    let stacks = FactoryStack::new(config)
        .replace(ProxyHandler::factory())
        // by default we don't decode http body
        // .push(monolake_services::http::handlers::ContentHandler::layer())
        .push(RewriteHandler::layer())
        .push(H2THandler::layer());

    #[cfg(feature = "openid")]
    let stacks = stacks.push(OpenIdHandler::layer());

    let stacks = stacks
        .push(ConnReuseHandler::layer())
        .push(HttpCoreService::layer())
        .push(HttpVersionDetect::layer());

    #[cfg(feature = "tls")]
    let stacks = stacks.push(monolake_services::tls::UnifiedTlsFactory::layer());

    #[cfg(feature = "proxy-protocol")]
    let stacks = stacks.push(ProxyProtocolServiceFactory::layer());

    stacks
        .check_make_svc::<(TcpStream, FullContext)>()
        .push(ContextService::<EmptyContext, _>::layer())
        .check_make_svc::<(TcpStream, AcceptedAddr)>()
        .push_arc_factory()
        .into_inner()
}
