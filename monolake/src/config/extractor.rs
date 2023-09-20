use std::path::PathBuf;

use certain_map::Param;
#[cfg(feature = "openid")]
use monolake_services::http::handlers::openid::OpenIdConfig;
use monolake_services::http::{H2TConfig, Keepalive};

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

impl Param<H2TConfig> for ServerConfig {
    fn param(&self) -> H2TConfig {
        const TEST_UNIX: &str = "/tmp/dyn-thrift-h2tsvc";

        H2TConfig {
            idl_path: vec![PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/idl/example0.thrift"
            ))],
            unix_address: TEST_UNIX.into(),
            max_idle: 10,
        }
    }
}
