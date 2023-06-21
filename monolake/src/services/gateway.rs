use monolake_core::{config::ServerConfig, environments::Environments, listener::AcceptedStream};
#[cfg(feature = "openid")]
use monolake_services::http::handlers::OpenIdHandler;
#[cfg(feature = "proxy-protocol")]
use monolake_services::proxy_protocol::ProxyProtocolServiceFactory;
use monolake_services::{
    common::Accept,
    http::{
        handlers::{ConnReuseHandler, ProxyHandler, RewriteHandler},
        HttpCoreService,
    },
    tls::UnifiedTlsFactory,
};
use service_async::stack::FactoryStack;

use super::{MakeServiceType, ServiceBuilder};

pub struct GatewayServiceBuilder<S> {
    service: FactoryStack<ServerConfig, S>,
}

impl GatewayServiceBuilder<MakeServiceType> {
    pub fn new(config: ServerConfig) -> Self {
        let stacks = FactoryStack::new(config)
            .replace(ProxyHandler::factory())
            .push(RewriteHandler::layer());

        #[cfg(feature = "openid")]
        let stacks = stacks.push(OpenIdHandler::layer());

        let stacks = stacks
            .push(ConnReuseHandler::layer())
            .push(HttpCoreService::layer())
            .check_make_svc::<Accept<AcceptedStream, Environments>>();

        let stacks = stacks.push(UnifiedTlsFactory::layer());

        #[cfg(feature = "proxy-protocol")]
        let stacks = stacks.push(ProxyProtocolServiceFactory::layer());
        let stacks = stacks.check_make_svc::<Accept<AcceptedStream, Environments>>();
        GatewayServiceBuilder { service: stacks }
    }
}

impl ServiceBuilder<MakeServiceType> for GatewayServiceBuilder<MakeServiceType> {
    fn build(self) -> MakeServiceType {
        self.service.into_inner()
    }
}
