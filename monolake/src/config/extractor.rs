use certain_map::Param;
#[cfg(feature = "openid")]
use monolake_services::http::handlers::openid::OpenIdConfig;
use monolake_services::{
    http::{
        handlers::{route::RouteConfig as HttpRouteConfig, upstream::HttpUpstreamTimeout},
        HttpServerTimeout, HttpVersion,
    },
    thrift::{ttheader::ThriftServerTimeout, RouteConfig as ThriftRouteConfig},
};

use super::ServerConfig;

impl Param<HttpServerTimeout> for ServerConfig {
    #[inline]
    fn param(&self) -> HttpServerTimeout {
        match &self.protocol {
            super::ServerProtocolConfig::Http { server_timeout, .. } => *server_timeout,
            super::ServerProtocolConfig::Thrift { .. } => {
                panic!("extract http server timeout from thrift config")
            }
        }
    }
}

impl Param<HttpUpstreamTimeout> for ServerConfig {
    #[inline]
    fn param(&self) -> HttpUpstreamTimeout {
        match &self.protocol {
            super::ServerProtocolConfig::Http {
                upstream_timeout, ..
            } => *upstream_timeout,
            super::ServerProtocolConfig::Thrift { .. } => {
                panic!("extract http upstream timeout from thrift config")
            }
        }
    }
}

impl Param<ThriftServerTimeout> for ServerConfig {
    #[inline]
    fn param(&self) -> ThriftServerTimeout {
        match &self.protocol {
            super::ServerProtocolConfig::Thrift { server_timeout, .. } => *server_timeout,
            super::ServerProtocolConfig::Http { .. } => {
                panic!("extract thrift server timeout from http config")
            }
        }
    }
}

#[cfg(feature = "openid")]
impl Param<Option<OpenIdConfig>> for ServerConfig {
    fn param(&self) -> Option<OpenIdConfig> {
        self.auth_config.clone().map(|cfg| cfg.0)
    }
}

impl Param<Vec<HttpRouteConfig>> for ServerConfig {
    #[inline]
    fn param(&self) -> Vec<HttpRouteConfig> {
        match &self.protocol {
            super::ServerProtocolConfig::Http { routes, .. } => routes.clone(),
            super::ServerProtocolConfig::Thrift { .. } => {
                panic!("extract http routes from thrift config")
            }
        }
    }
}

impl Param<ThriftRouteConfig> for ServerConfig {
    #[inline]
    fn param(&self) -> ThriftRouteConfig {
        match &self.protocol {
            super::ServerProtocolConfig::Thrift { route, .. } => route.clone(),
            super::ServerProtocolConfig::Http { .. } => {
                panic!("extract thrift routes from http config")
            }
        }
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
        match &self.protocol {
            super::ServerProtocolConfig::Http {
                upstream_http_version,
                ..
            } => *upstream_http_version,
            super::ServerProtocolConfig::Thrift { .. } => {
                panic!("extract http version from thrift config")
            }
        }
    }
}
