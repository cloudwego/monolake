use crate::{service::Param, tls::TlsConfig};

use super::{KeepaliveConfig, RouteConfig, ServerConfig};

impl Param<Option<KeepaliveConfig>> for ServerConfig {
    fn param(&self) -> Option<KeepaliveConfig> {
        self.keepalive_config
    }
}

impl Param<Vec<RouteConfig>> for ServerConfig {
    fn param(&self) -> Vec<RouteConfig> {
        self.routes.clone()
    }
}

impl Param<TlsConfig> for ServerConfig {
    // TODO: add a `build` for ServerConfig to finish the io and convertion
    fn param(&self) -> TlsConfig {
        match &self.tls {
            Some(tls) => tls.try_into().expect("load cert and key failed"),
            None => TlsConfig::None,
        }
    }
}
