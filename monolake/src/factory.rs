//! Preconstructed factories.
use std::fmt::Debug;

use certain_map::Param;
use monoio::net::TcpStream;
use monolake_core::listener::{AcceptedAddr, AcceptedStream};
#[cfg(feature = "openid")]
use monolake_services::http::handlers::OpenIdHandler;
#[cfg(feature = "proxy-protocol")]
use monolake_services::proxy_protocol::ProxyProtocolServiceFactory;
use monolake_services::{
    common::ContextService,
    http::{
        core::HttpCoreService,
        detect::HttpVersionDetect,
        handlers::{
            upstream::HttpUpstreamTimeout, ConnectionReuseHandler, ContentHandler,
            RewriteAndRouteHandler, UpstreamHandler,
        },
        HttpVersion,
    },
    tcp::Accept,
    thrift::{handlers::ProxyHandler as TProxyHandler, ttheader::TtheaderCoreService},
};
use service_async::{stack::FactoryStack, ArcMakeService, Service};

use crate::{
    config::ServerConfig,
    context::{Context, FullContext},
};

/// Create a new factory for l7 proxy.
// Here we use a fixed generic type `Accept<AcceptedStream, AcceptedAddr>`
// for simplification and make return impl work.
#[allow(dead_code)]
pub fn l7_factory(
    config: ServerConfig,
) -> ArcMakeService<
    impl Service<Accept<AcceptedStream, AcceptedAddr>, Error = impl Debug>,
    impl Debug,
> {
    match config.proxy_type {
        crate::config::ProxyType::Http => {
            let protocol: HttpVersion = config.param();
            let http_upstream_timeout: HttpUpstreamTimeout = config.param();
            let stacks = FactoryStack::new(config.clone())
                .replace(UpstreamHandler::factory(http_upstream_timeout, protocol))
                .push(ContentHandler::layer())
                .push(RewriteAndRouteHandler::layer());

            #[cfg(feature = "openid")]
            let stacks = stacks.push(OpenIdHandler::layer());

            let stacks = stacks
                .push(ConnectionReuseHandler::layer())
                .push(HttpCoreService::layer())
                .push(HttpVersionDetect::layer());

            #[cfg(feature = "tls")]
            let stacks = stacks.push(monolake_services::tls::UnifiedTlsFactory::layer());

            #[cfg(feature = "proxy-protocol")]
            let stacks = stacks.push(ProxyProtocolServiceFactory::layer());

            stacks
                .check_make_svc::<(TcpStream, FullContext)>()
                .push(ContextService::<Context, _>::layer())
                .check_make_svc::<(TcpStream, AcceptedAddr)>()
                .into_boxed_service()
                .into_arc_factory()
                .into_inner()
        }
        crate::config::ProxyType::Thrift => {
            let proxy_config = config.param();
            let stacks = FactoryStack::new(config)
                .replace(TProxyHandler::factory(proxy_config))
                .push(TtheaderCoreService::layer());

            #[cfg(feature = "tls")]
            let stacks = stacks.push(monolake_services::tls::UnifiedTlsFactory::layer());

            stacks
                .check_make_svc::<(TcpStream, FullContext)>()
                .push(ContextService::<Context, _>::layer())
                .check_make_svc::<(TcpStream, AcceptedAddr)>()
                .into_boxed_service()
                .into_arc_factory()
                .into_inner()
        }
    }
}
