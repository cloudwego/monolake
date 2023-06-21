use service_async::Param;

#[cfg(feature = "openid")]
use super::OpenIdConfig;
use super::{KeepaliveConfig, RouteConfig, ServerConfig};
use crate::tls::TlsConfig;

impl Param<Option<KeepaliveConfig>> for ServerConfig {
    fn param(&self) -> Option<KeepaliveConfig> {
        self.keepalive_config
    }
}

#[cfg(feature = "openid")]
impl Param<Option<OpenIdConfig>> for ServerConfig {
    fn param(&self) -> Option<OpenIdConfig> {
        if self.auth_config.is_none() {
            return None;
        }
        match self.auth_config.clone().unwrap() {
            super::AuthConfig::OpenIdConfig(openid_config) => Some(openid_config),
        }
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

impl Param<()> for ServerConfig {
    fn param(&self) {}
}
