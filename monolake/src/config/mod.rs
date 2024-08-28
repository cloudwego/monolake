use std::{collections::HashMap, path::Path, time::Duration};

use monolake_core::{
    config::{RuntimeConfig, ServiceConfig},
    listener::ListenerBuilder,
};
use monolake_services::{
    http::{handlers::route::RouteConfig, HttpServerTimeout, Protocol},
    thrift::ttheader::ThriftServerTimeout,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

mod extractor;

#[derive(Debug, Clone)]
pub struct Config {
    pub runtime: RuntimeConfig,
    pub servers: HashMap<String, ServiceConfig<ListenerConfig, ServerConfig>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyType {
    #[default]
    Http,
    Thrift,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    #[allow(unused)]
    pub name: String,
    pub proxy_type: ProxyType,
    #[cfg(feature = "tls")]
    pub tls: monolake_services::tls::TlsConfig,
    pub routes: Vec<RouteConfig>,
    pub http_server_timeout: HttpServerTimeout,
    pub protocol: Protocol,
    pub thrift_server_timeout: ThriftServerTimeout,
    #[cfg(feature = "openid")]
    pub auth_config: Option<AuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerUserConfig {
    pub name: String,
    #[serde(default)]
    pub proxy_type: ProxyType,
    pub tls: Option<TlsUserConfig>,
    pub routes: Vec<RouteConfig>,
    pub http_timeout: Option<HttpTimeout>,
    #[serde(default = "Protocol::default")]
    pub protocol: Protocol,
    pub thrift_timeout: Option<ThriftTimeout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsUserConfig {
    pub key: String,
    pub chain: String,
    #[serde(default)]
    pub stack: TlsStack,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct HttpTimeout {
    // Connection keepalive timeout: If no byte comes when decoder want next request, close the
    // connection. Link Nginx `keepalive_timeout`
    server_keepalive_timeout_sec: Option<u64>,
    // Read full http header.
    // Like Nginx `client_header_timeout`
    server_read_header_timeout_sec: Option<u64>,
    // Receiving full body timeout.
    // Like Nginx `client_body_timeout`
    server_read_body_timeout_sec: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ThriftTimeout {
    // Connection keepalive timeout: If no byte comes when decoder want next request, close the
    // connection. Link Nginx `keepalive_timeout`
    server_keepalive_timeout_sec: Option<u64>,
    // Read full thrift message.
    server_message_timeout_sec: Option<u64>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TlsStack {
    #[default]
    Rustls,
    NativeTls,
}

#[cfg(feature = "openid")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthConfig(pub monolake_services::http::handlers::openid::OpenIdConfig);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ListenerConfig {
    Socket(std::net::SocketAddr),
    Unix(std::path::PathBuf),
}

impl TryFrom<ListenerConfig> for ListenerBuilder {
    type Error = std::io::Error;

    fn try_from(value: ListenerConfig) -> Result<Self, Self::Error> {
        match value {
            ListenerConfig::Socket(addr) => ListenerBuilder::bind_tcp(addr, Default::default()),
            ListenerConfig::Unix(addr) => ListenerBuilder::bind_unix(addr),
        }
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct UserConfig {
            #[serde(default)]
            pub runtime: RuntimeConfig,
            pub servers: HashMap<String, ServiceConfig<ListenerConfig, ServerUserConfig>>,
        }
        // 1. load from file -> UserConfig
        let file_context = monolake_core::util::file_read_sync(path)?;
        let user_config = Self::from_slice::<UserConfig>(&file_context)?;

        // 2. UserConfig -> Config
        let UserConfig { runtime, servers } = user_config;
        let mut servers_new = HashMap::with_capacity(servers.len());
        for (key, server) in servers.into_iter() {
            let ServiceConfig { listener, server } = server;
            #[cfg(feature = "tls")]
            let tls = match server.tls {
                Some(inner) => {
                    let chain = monolake_core::util::file_read_sync(&inner.chain)?;
                    let key = monolake_core::util::file_read_sync(&inner.key)?;
                    match inner.stack {
                        TlsStack::Rustls => {
                            monolake_services::tls::TlsConfig::Rustls((chain, key)).try_into()?
                        }
                        TlsStack::NativeTls => {
                            monolake_services::tls::TlsConfig::Native((chain, key)).try_into()?
                        }
                    }
                }
                None => monolake_services::tls::TlsConfig::None,
            };
            let server_http_timeout = server.http_timeout.unwrap_or_default();
            let server_thrift_timeout = server.thrift_timeout.unwrap_or_default();
            servers_new.insert(
                key,
                ServiceConfig {
                    server: ServerConfig {
                        name: server.name,
                        proxy_type: server.proxy_type,
                        #[cfg(feature = "tls")]
                        tls,
                        routes: server.routes,
                        http_server_timeout: HttpServerTimeout {
                            keepalive_timeout: server_http_timeout
                                .server_keepalive_timeout_sec
                                .map(Duration::from_secs),
                            read_header_timeout: server_http_timeout
                                .server_read_header_timeout_sec
                                .map(Duration::from_secs),
                            read_body_timeout: server_http_timeout
                                .server_read_body_timeout_sec
                                .map(Duration::from_secs),
                        },
                        thrift_server_timeout: ThriftServerTimeout {
                            keepalive_timeout: server_thrift_timeout
                                .server_keepalive_timeout_sec
                                .map(Duration::from_secs),
                            message_timeout: server_thrift_timeout
                                .server_message_timeout_sec
                                .map(Duration::from_secs),
                        },
                        protocol: server.protocol,
                        #[cfg(feature = "openid")]
                        auth_config: None,
                    },
                    listener,
                },
            );
        }
        Ok(Config {
            runtime,
            servers: servers_new,
        })
    }

    pub fn from_slice<T: DeserializeOwned>(content: &[u8]) -> anyhow::Result<T> {
        // read first non-space u8
        let is_json = match content
            .iter()
            .find(|&&b| b != b' ' && b != b'\r' && b != b'\n' && b != b'\t')
        {
            Some(first) => *first == b'{',
            None => false,
        };
        match is_json {
            true => serde_json::from_slice::<T>(content).map_err(Into::into),
            false => toml::from_str::<T>(&String::from_utf8_lossy(content)).map_err(Into::into),
        }
    }
}
