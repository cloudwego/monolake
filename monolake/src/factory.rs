//! Preconstructed factories.

use std::{fmt::Debug, net::SocketAddr};

use monoio::net::TcpStream;
use monolake_core::{
    config::ServerConfig,
    http::HttpAccept,
    listener::{AcceptedAddr, AcceptedStream},
};
use monolake_services::{
    common::Accept,
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
    FactoryStack::new(config)
        .replace(ProxyHandler::factory())
        .push(ConnReuseHandler::layer())
        .push(RewriteHandler::layer())
        .push(HttpCoreService::layer())
        .check_make_svc::<HttpAccept<TcpStream, SocketAddr>>()
        .push(HttpVersionDetect::layer())
        .push(UnifiedTlsFactory::layer())
        .check_make_svc::<(AcceptedStream, AcceptedAddr)>()
        .into_inner()
}
