use std::{
    collections::HashMap,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::listener::ListenerBuilder;

mod extrator;

// MAX configuration file size: 16 MB
const MAX_CONFIG_FILE_SIZE: usize = 16 * 1024 * 1024;
// Read buffer size: 8 KB
const READ_BUFFER_SIZE: usize = 8 * 1024;
// Default iouring/epoll entries: 32k
const DEFAULT_ENTRIES: u32 = 32768;

pub const DEFAULT_TIME: u64 = 3600;
pub const DEFAULT_TIMEOUT: usize = 75;
pub const DEFAULT_REQUESTS: usize = 1000;
pub const MAX_CONFIG_SIZE_LIMIT: usize = 8072;
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
pub const MIN_SQPOLL_IDLE_TIME: u32 = 1000; // 1s idle time.
pub const FALLBACK_PARALLELISM: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1) };
pub const APLN_PROTOCOLS: [&[u8]; 2] = [b"h2", b"http/1.1"];

macro_rules! define_const {
    ($name: ident, $val: expr, $type: ty) => {
        const fn $name() -> $type {
            $val
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub runtime: RuntimeConfig,
    pub servers: HashMap<String, ServerConfigWithListener>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfigWithListener {
    pub listener: ListenerConfig,
    #[serde(flatten)]
    pub server: ServerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_workers")]
    pub worker_threads: usize,
    #[serde(default = "default_entries")]
    pub entries: u32,
    pub sqpoll_idle: Option<u32>,
    #[serde(default)]
    pub runtime_type: RuntimeType,
    #[serde(default = "default_cpu_affinity")]
    pub cpu_affinity: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        RuntimeConfig {
            worker_threads: default_workers(),
            entries: default_entries(),
            sqpoll_idle: None,
            runtime_type: Default::default(),
            cpu_affinity: default_cpu_affinity(),
        }
    }
}

fn default_workers() -> usize {
    std::thread::available_parallelism()
        .unwrap_or(FALLBACK_PARALLELISM)
        .into()
}

define_const!(default_entries, DEFAULT_ENTRIES, u32);
define_const!(default_cpu_affinity, true, bool);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeType {
    #[cfg(target_os = "linux")]
    IoUring,
    Legacy,
}

impl Default for RuntimeType {
    #[cfg(target_os = "linux")]
    fn default() -> Self {
        Self::IoUring
    }
    #[cfg(not(target_os = "linux"))]
    fn default() -> Self {
        Self::Legacy
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub tls: Option<TlsConfig>,
    pub routes: Vec<RouteConfig>,
    pub keepalive_config: Option<KeepaliveConfig>,
    pub auth_config: Option<AuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ListenerConfig {
    SocketAddress(SocketAddress),
    Uds(Uds),
}

impl TryFrom<ListenerConfig> for ListenerBuilder {
    type Error = std::io::Error;

    fn try_from(value: ListenerConfig) -> Result<Self, Self::Error> {
        match value {
            ListenerConfig::SocketAddress(addr) => {
                ListenerBuilder::bind_tcp(addr.socket_addr, Default::default())
            }
            ListenerConfig::Uds(addr) => ListenerBuilder::bind_unix(addr.uds_path),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub key: String,
    pub chain: String,
    #[serde(default)]
    pub stack: TlsStack,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TlsStack {
    Rustls,
    NativeTls,
}

impl Default for TlsStack {
    fn default() -> Self {
        Self::Rustls
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    #[serde(skip)]
    pub id: String,
    pub path: String,
    pub upstreams: Vec<Upstream>,
}

// TODO(ihciah): rename _name to _sec
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct KeepaliveConfig {
    #[serde(default = "default_keepalive_requests")]
    pub keepalive_requests: usize,
    #[serde(default = "default_keepalive_time")]
    pub keepalive_time: u64,
    #[serde(default = "default_keepalive_timeout")]
    pub keepalive_timeout: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum AuthConfig {
    #[cfg(feature = "openid")]
    OpenIdConfig(OpenIdConfig),
}

#[cfg(feature = "openid")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenIdConfig {
    // TODO: Need to add openid scopes etc.
    pub client_id: String,
    pub client_secret: String,
    pub issuer_url: String,
    pub redirect_url: String,
}

define_const!(default_keepalive_requests, DEFAULT_REQUESTS, usize);
define_const!(default_keepalive_time, DEFAULT_TIME, u64);
define_const!(default_keepalive_timeout, DEFAULT_TIMEOUT, usize);

impl KeepaliveConfig {
    pub fn keepalive_time(&self) -> Duration {
        Duration::from_secs(self.keepalive_time)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub endpoint: Endpoint,
    #[serde(default = "default_weight")]
    pub weight: u16,
}

define_const!(default_weight, 1, u16);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Endpoint {
    Uri(Uri),
    SocketAddress(SocketAddress),
    Uds(Uds),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransportProtocol {
    #[default]
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocketAddress {
    pub socket_addr: std::net::SocketAddr,
    #[serde(default)]
    pub transport_protocol: TransportProtocol,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Uds {
    pub uds_path: PathBuf,
    #[serde(default)]
    pub transport_protocol: TransportProtocol,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Uri {
    #[serde(with = "http_serde::uri")]
    pub uri: http::Uri,
}

impl Config {
    pub async fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Self::from_slice(&Self::read_file(path).await?)
    }

    pub fn from_slice(content: &[u8]) -> anyhow::Result<Self> {
        // read first non-space u8
        let is_json = match content
            .iter()
            .find(|&&b| b != b' ' && b != b'\r' && b != b'\n' && b != b'\t')
        {
            Some(first) => *first == b'{',
            None => false,
        };
        match is_json {
            true => serde_json::from_slice::<Self>(content).map_err(Into::into),
            false => toml::from_str::<Self>(&String::from_utf8_lossy(content)).map_err(Into::into),
        }
    }

    async fn read_file(path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>> {
        let file = match monoio::fs::File::open(path).await {
            Ok(file) => file,
            Err(e) => bail!("Config: error open file: {:?}", e),
        };

        let mut data = Vec::new();
        let mut buffer = Vec::with_capacity(READ_BUFFER_SIZE);

        loop {
            let (res, buf) = file.read_at(buffer, data.len() as u64).await;
            let n = res?;
            buffer = buf;
            if n == 0 {
                break;
            }

            if data.len() + n > MAX_CONFIG_FILE_SIZE {
                bail!("Config: max file size: {}", MAX_CONFIG_FILE_SIZE);
            }
            data.extend_from_slice(&buffer);
        }

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn test_json_deserialize() {
        const TEST_CONFIG: &str =
            "
            {
                \"servers\": {
                    \"test-server\": {
                        \
             \"name\": \"test\",
                                   \"listener\": {\"socket_addr\" : \
             \"0.0.0.0:8080\"},
                                   \"routes\": [{
                            \
             \"path\": \"/\",
                            \"upstreams\": [{
                                \
             \"endpoint\": {\"uds_path\":\"/tmp/test\"},\"weight\": 1 }, {
                                \
             \"endpoint\": {\"uri\":\"https://gateway.example.com/\"},\"weight\": 2 }] }]
                    }
                }
            }
        ";

        let config = Config::from_slice(TEST_CONFIG.as_bytes()).unwrap();
        assert_eq!("test-server", config.servers.keys().next().unwrap());
    }

    #[test]
    fn test_toml_deserialize() {
        const TEST_CONFIG: &str = "
            [servers.test-server]
            name = 'gateway.example.com'
            listener = { socket_addr = '[::]:8080' }

            [[servers.test-server.routes]]
            path = '/'
            id = 'test'

            [[servers.test-server.routes.upstreams]]
            endpoint = {uri = 'test'}
            weight = 1

            [[servers.test-server.routes.upstreams]]
            endpoint = {uds_path = '/tmp/def.sock'}
            weight = 2
        ";

        let config: Config = Config::from_slice(TEST_CONFIG.as_bytes()).unwrap();
        assert_eq!("test-server", config.servers.keys().next().unwrap());
    }
}
