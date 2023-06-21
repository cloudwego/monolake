use monolake_core::{config::ServerConfig, environments::Environments, listener::AcceptedStream};
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

pub struct ProxyServiceBuilder<S> {
    service: FactoryStack<ServerConfig, S>,
}

impl ProxyServiceBuilder<MakeServiceType> {
    pub fn new(config: ServerConfig) -> Self {
        let stacks = FactoryStack::new(config)
            .replace(ProxyHandler::factory())
            .push(RewriteHandler::layer());

        let stacks = stacks
            .push(ConnReuseHandler::layer())
            .push(HttpCoreService::layer())
            .check_make_svc::<Accept<AcceptedStream, Environments>>();

        let stacks = stacks.push(UnifiedTlsFactory::layer());

        #[cfg(feature = "proxy-protocol")]
        let stacks = stacks.push(ProxyProtocolServiceFactory::layer());
        let stacks = stacks.check_make_svc::<Accept<AcceptedStream, Environments>>();
        ProxyServiceBuilder { service: stacks }
    }
}

impl ServiceBuilder<MakeServiceType> for ProxyServiceBuilder<MakeServiceType> {
    fn build(self) -> MakeServiceType {
        self.service.into_inner()
    }
}
