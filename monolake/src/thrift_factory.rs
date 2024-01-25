//! Preconstructed factories.

use std::fmt::Debug;

use monoio::net::TcpStream;
use monolake_core::listener::{AcceptedAddr, AcceptedStream};
use monolake_services::{
    common::ContextService,
    tcp::Accept,
    thrift::{handlers::ProxyHandler, ttheader::TtheaderCoreService},
};
use service_async::{stack::FactoryStack, ArcMakeService, Service};

use crate::{
    config::ServerConfig,
    context::{EmptyContext, FullContext},
};

/// Create a new factory for thrift proxy.
// Here we use a fixed generic type `Accept<AcceptedStream, AcceptedAddr>`
// for simplification and make return impl work.
pub fn thrift_factory(
    config: ServerConfig,
) -> ArcMakeService<
    impl Service<Accept<AcceptedStream, AcceptedAddr>, Error = impl Debug>,
    impl Debug,
> {
    let stacks = FactoryStack::new(config)
        .replace(ProxyHandler::factory())
        .push(TtheaderCoreService::layer());

    #[cfg(feature = "tls")]
    let stacks = stacks.push(monolake_services::tls::UnifiedTlsFactory::layer());

    stacks
        .check_make_svc::<(TcpStream, FullContext)>()
        .push(ContextService::<EmptyContext, _>::layer())
        .check_make_svc::<(TcpStream, AcceptedAddr)>()
        .into_arc_factory()
        .into_inner()
}
