use certain_map::Param;
#[cfg(feature = "openid")]
use monolake_services::http::handlers::openid::OpenIdConfig;
use monolake_services::{
    http::{handlers::upstream::HttpUpstreamTimeout, HttpServerTimeout, HttpVersion},
    thrift::ttheader::ThriftServerTimeout,
};

use super::{RouteConfig, ServerConfig};

impl Param<HttpServerTimeout> for ServerConfig {
    #[inline]
    fn param(&self) -> HttpServerTimeout {
        self.http_server_timeout
    }
}

impl Param<HttpUpstreamTimeout> for ServerConfig {
    #[inline]
    fn param(&self) -> HttpUpstreamTimeout {
        self.http_upstream_timeout
    }
}

impl Param<ThriftServerTimeout> for ServerConfig {
    #[inline]
    fn param(&self) -> ThriftServerTimeout {
        self.thrift_server_timeout
    }
}

#[cfg(feature = "openid")]
impl Param<Option<OpenIdConfig>> for ServerConfig {
    fn param(&self) -> Option<OpenIdConfig> {
        self.auth_config.clone().map(|cfg| cfg.0)
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

impl Param<HttpVersion> for ServerConfig {
    #[inline]
    fn param(&self) -> HttpVersion {
        self.upstream_http_version
    }
}
