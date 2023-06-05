//! Preconstructed factories.

use std::{net::SocketAddr, sync::Arc};

use monoio::net::TcpStream;
use monoio_http_client::Client;
use monolake_core::{
    config::ServerConfig,
    service::{stack::FactoryStack, MakeService}, AnyError, listener::{AcceptedStream, AcceptedAddr},
};
use monolake_services::{
    http::{
        handlers::{ConnReuseHandler, ProxyHandler, RewriteHandler},
        HttpCoreService,
    },
    tls::UnifiedTlsFactory,
};

// type L7Factory = impl MakeService<Service = S> where Self::Error: Debug, S: Service<A> + 'static,S::Error: Debug, A: 'static;

pub fn l7_factory(config: ServerConfig) -> impl MakeService //<Error = AnyError>
// <Service = S, Error = HttpError>
// where
//     S: Service<A> + 'static,
//     S::Error: std::fmt::Debug,
//     A: 'static,
{
    FactoryStack::new(config)
        .replace(ProxyHandler::factory())
        .push(ConnReuseHandler::layer())
        .push(RewriteHandler::layer())
        .push(HttpCoreService::layer())
        .check_make_svc::<(TcpStream, SocketAddr)>()
        .push(UnifiedTlsFactory::layer())
        .check_make_svc::<(AcceptedStream, AcceptedAddr)>()
        .into_inner()
}
