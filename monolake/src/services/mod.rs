use std::fmt::Debug;

use monolake_core::{environments::Environments, listener::AcceptedStream};
use monolake_services::common::Accept;
use service_async::{MakeService, Service};

pub type MakeServiceType = impl MakeService<
    Service = impl Service<Accept<AcceptedStream, Environments>, Error = impl Debug>,
    Error = impl Debug,
>;

pub trait ServiceBuilder<MakeServiceType> {
    fn build(self) -> MakeServiceType;
}

#[cfg(feature = "gateway")]
mod gateway;
#[cfg(feature = "gateway")]
pub type CurrentServiceBuilder<S> = gateway::GatewayServiceBuilder<S>;

#[cfg(feature = "proxy")]
mod proxy;
#[cfg(feature = "proxy")]
pub type CurrentServiceBuilder<S> = proxy::ProxyServiceBuilder<S>;
