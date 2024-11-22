use std::{collections::HashMap, path::Path, time::Duration};

use monolake_core::{
    config::{RuntimeConfig, ServiceConfig},
    listener::ListenerBuilder,
};
use monolake_services::{
    http::{
        handlers::{route::RouteConfig as HttpRouteConfig, upstream::HttpUpstreamTimeout},
        HttpServerTimeout, HttpVersion,
    },
    thrift::{ttheader::ThriftServerTimeout, RouteConfig as ThriftRouteConfig},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

mod extractor;
pub mod manager;

#[allow(unused)]
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
    #[cfg(feature = "tls")]
    pub tls: monolake_services::tls::TlsConfig,
    #[cfg(feature = "openid")]
    pub auth_config: Option<AuthConfig>,
    pub protocol: ServerProtocolConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerUserConfig {
    pub name: String,
    pub tls: Option<TlsUserConfig>,

    #[serde(flatten)]
    pub protocol_config: ServerProtocolUserConfig,
}

#[derive(Debug, Clone)]
pub enum ServerProtocolConfig {
    Http {
        routes: Vec<HttpRouteConfig>,
        server_timeout: HttpServerTimeout,
        upstream_timeout: HttpUpstreamTimeout,
        upstream_http_version: HttpVersion,
        opt_handlers: HttpOptHandlers,
    },
    Thrift {
        route: ThriftRouteConfig,
        server_timeout: ThriftServerTimeout,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "proxy_type")]
pub enum ServerProtocolUserConfig {
    Http(ServerHttpUserConfig),
    Thrift(ServerThriftUserConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHttpUserConfig {
    pub routes: Vec<HttpRouteConfig>,
    #[serde(default)]
    pub timeout: HttpTimeout,
    #[serde(default)]
    pub upstream_http_version: HttpVersion,
    #[serde(default)]
    pub http_opt_handlers: HttpOptHandlers,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerThriftUserConfig {
    pub route: ThriftRouteConfig,
    #[serde(default)]
    pub timeout: ThriftTimeout,
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
    // Connect timeout
    // Like Nginx 'proxy_connect_timeout'
    upstream_connect_timeout_sec: Option<u64>,
    // Read response timeout
    upstream_read_timeout_sec: Option<u64>,
}

impl From<HttpTimeout> for HttpServerTimeout {
    fn from(t: HttpTimeout) -> Self {
        HttpServerTimeout {
            keepalive_timeout: t.server_keepalive_timeout_sec.map(Duration::from_secs),
            read_header_timeout: t.server_read_header_timeout_sec.map(Duration::from_secs),
            read_body_timeout: t.server_read_body_timeout_sec.map(Duration::from_secs),
        }
    }
}
impl From<HttpTimeout> for HttpUpstreamTimeout {
    fn from(t: HttpTimeout) -> Self {
        HttpUpstreamTimeout {
            connect_timeout: t.upstream_connect_timeout_sec.map(Duration::from_secs),
            read_timeout: t.upstream_read_timeout_sec.map(Duration::from_secs),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ThriftTimeout {
    // Connection keepalive timeout: If no byte comes when decoder want next request, close the
    // connection. Link Nginx `keepalive_timeout`
    server_keepalive_timeout_sec: Option<u64>,
    // Read full thrift message.
    server_message_timeout_sec: Option<u64>,
}

impl From<ThriftTimeout> for ThriftServerTimeout {
    fn from(t: ThriftTimeout) -> Self {
        ThriftServerTimeout {
            keepalive_timeout: t.server_keepalive_timeout_sec.map(Duration::from_secs),
            message_timeout: t.server_message_timeout_sec.map(Duration::from_secs),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TlsStack {
    #[default]
    Rustls,
    NativeTls,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct HttpOptHandlers {
    // Enable content handler in the handler chain
    pub content_handler: bool,
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
    #[allow(unused)]
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        struct UserConfig {
            #[serde(default)]
            runtime: RuntimeConfig,
            servers: HashMap<String, ServiceConfig<ListenerConfig, ServerUserConfig>>,
        }
        // 1. load from file -> UserConfig
        let file_context = monolake_core::util::file_read_sync(path)?;
        let user_config = parse_from_slice::<UserConfig>(&file_context)?;

        // 2. UserConfig -> Config
        let UserConfig { runtime, servers } = user_config;
        let servers_new = build_server_config(servers)?;
        Ok(Config {
            runtime,
            servers: servers_new,
        })
    }

    pub fn load_runtime_config(path: impl AsRef<Path>) -> anyhow::Result<RuntimeConfig> {
        #[derive(Deserialize)]
        struct RuntimeConfigContainer {
            runtime: RuntimeConfig,
        }
        let file_content = monolake_core::util::file_read_sync(path)?;
        let container = parse_from_slice::<RuntimeConfigContainer>(&file_content)?;
        Ok(container.runtime)
    }

    pub fn parse_service_config(
        file_content: &[u8],
    ) -> anyhow::Result<HashMap<String, ServiceConfig<ListenerConfig, ServerConfig>>> {
        #[derive(Deserialize)]
        struct UserConfigContainer {
            servers: HashMap<String, ServiceConfig<ListenerConfig, ServerUserConfig>>,
        }

        let container = parse_from_slice::<UserConfigContainer>(file_content)?;
        build_server_config(container.servers)
    }
}

pub fn build_server_config(
    servers: HashMap<String, ServiceConfig<ListenerConfig, ServerUserConfig>>,
) -> anyhow::Result<HashMap<String, ServiceConfig<ListenerConfig, ServerConfig>>> {
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

        let protocol = match server.protocol_config {
            ServerProtocolUserConfig::Http(http) => {
                let routes = http.routes;
                let server_timeout = http.timeout.into();
                let upstream_timeout = http.timeout.into();
                let upstream_http_version = http.upstream_http_version;
                let opt_handlers = http.http_opt_handlers;
                ServerProtocolConfig::Http {
                    routes,
                    server_timeout,
                    upstream_timeout,
                    upstream_http_version,
                    opt_handlers,
                }
            }
            ServerProtocolUserConfig::Thrift(thrift) => ServerProtocolConfig::Thrift {
                route: thrift.route,
                server_timeout: thrift.timeout.into(),
            },
        };

        let svc_cfg = ServiceConfig {
            listener,
            server: ServerConfig {
                name: server.name,
                #[cfg(feature = "tls")]
                tls,
                #[cfg(feature = "openid")]
                auth_config: None,
                protocol,
            },
        };
        servers_new.insert(key, svc_cfg);
    }
    Ok(servers_new)
}

pub fn parse_from_slice<T: DeserializeOwned>(content: &[u8]) -> anyhow::Result<T> {
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
