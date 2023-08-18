use certain_map::Param;
#[cfg(feature = "openid")]
use monolake_services::http::handlers::openid::OpenIdConfig;
use monolake_services::http::Keepalive;

use super::{RouteConfig, ServerConfig};

impl Param<Keepalive> for ServerConfig {
    fn param(&self) -> Keepalive {
        self.keepalive_config
    }
}

#[cfg(feature = "openid")]
impl Param<Option<OpenIdConfig>> for ServerConfig {
    fn param(&self) -> Option<OpenIdConfig> {
        self.auth_config.clone().map(|cfg| match cfg {
            super::AuthConfig::OpenIdConfig(inner) => inner,
        })
    }
}

impl Param<Vec<RouteConfig>> for ServerConfig {
    fn param(&self) -> Vec<RouteConfig> {
        self.routes.clone()
    }
}

#[cfg(feature = "tls")]
impl Param<monolake_services::tls::TlsConfig> for ServerConfig {
    fn param(&self) -> monolake_services::tls::TlsConfig {
        self.tls.clone()
    }
}
