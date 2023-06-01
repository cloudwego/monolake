use std::{
    collections::HashMap,
    num::NonZeroUsize,
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
};

use anyhow::bail;
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};

mod parsers;
use parsers::parse;

// MAX configuration file size: 16 MB
const MAX_CONFIG_FILE_SIZE: usize = 16 * 1024 * 1024;
// Read buffer size: 8 KB
const READ_BUFFER_SIZE: usize = 8 * 1024;
// Default iouring/epoll entries: 32k
const DEFAULT_ENTRIES: u32 = 32768;

pub const DEFAULT_TIME: usize = 3600;
pub const DEFAULT_TIMEOUT: usize = 75;
pub const DEFAULT_REQUESTS: usize = 1000;
pub const MAX_CONFIG_SIZE_LIMIT: usize = 8072;
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
pub const MIN_SQPOLL_IDLE_TIME: u32 = 1000; // 1s idle time.
pub const FALLBACK_PARALLELISM: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1) };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub runtime: RuntimeConfig,
    pub servers: HashMap<String, ServerConfig>,
}

const fn default_entries() -> u32 {
    DEFAULT_ENTRIES
}

fn default_workers() -> usize {
    std::thread::available_parallelism()
        .unwrap_or(FALLBACK_PARALLELISM)
        .into()
}

const fn default_cpu_affinity() -> bool {
    true
}

const fn default_keepalive_requests() -> usize {
    DEFAULT_REQUESTS
}

const fn default_keepalive_time() -> usize {
    DEFAULT_TIME
}

const fn default_keepalive_timeout() -> usize {
    DEFAULT_TIMEOUT
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeType {
    #[cfg(all(target_os = "linux"))]
    IoUring,
    Legacy,
}

impl Default for RuntimeType {
    #[cfg(all(target_os = "linux"))]
    fn default() -> Self {
        Self::IoUring
    }
    #[cfg(not(target_os = "linux"))]
    fn default() -> Self {
        Self::Legacy
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub listeners: Vec<ListenerConfig>,
    pub tls: Option<TlsConfig>,
    pub routes: Vec<RouteConfig>,
    pub keepalive_config: Option<KeepaliveConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub key: String,
    pub chain: String,
    #[serde(default)]
    pub stack: TlsStack,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct KeepaliveConfig {
    #[serde(default = "default_keepalive_requests")]
    pub keepalive_requests: usize,
    #[serde(default = "default_keepalive_time")]
    pub keepalive_time: usize,
    #[serde(default = "default_keepalive_timeout")]
    pub keepalive_timeout: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ListenerConfig {
    SocketAddress(SocketAddress),
    Uds(Uds),
}

impl ListenerConfig {
    pub fn transport_protocol(&self) -> TransportProtocol {
        match self {
            Self::SocketAddress(s) => s.transport_protocol.to_owned(),
            Self::Uds(u) => u.transport_protocol.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    #[serde(skip)]
    pub id: String,
    pub path: String,
    pub upstreams: Vec<Upstream>,
}

fn default_weight() -> u16 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub endpoint: Endpoint,
    #[serde(default = "default_weight")]
    pub weight: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Endpoint {
    Uri(Uri),
    SocketAddress(SocketAddress),
    Uds(Uds),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransportProtocol {
    Tcp,
    Udp,
}

impl Default for TransportProtocol {
    fn default() -> Self {
        Self::Tcp
    }
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

async fn read_file(path: impl AsRef<Path>) -> anyhow::Result<Bytes> {
    let mut data = BytesMut::new();

    let file = match monoio::fs::File::open(path).await {
        Ok(file) => file,
        Err(e) => bail!("Config: error open file: {:?}", e),
    };

    let mut buffer = BytesMut::with_capacity(READ_BUFFER_SIZE);
    let mut current: u64 = 0;

    loop {
        let (res, buf) = file.read_at(buffer, current).await;
        let n = res?;
        buffer = buf;

        if data.len() + n > MAX_CONFIG_FILE_SIZE {
            bail!("Config: max file size: {}", MAX_CONFIG_FILE_SIZE);
        }

        data.extend_from_slice(&buffer[..n]);

        if n < READ_BUFFER_SIZE {
            break;
        }

        current += n as u64;
        buffer.clear();
    }

    Ok(Bytes::from(data))
}

fn parse_extension(path: &impl AsRef<Path>) -> String {
    let extension = path
        .as_ref()
        .extension()
        .unwrap_or_default()
        .as_bytes()
        .to_ascii_lowercase();
    String::from_utf8(extension).unwrap_or_default()
}

impl Config {
    pub async fn load(path: impl AsRef<Path>) -> anyhow::Result<Config> {
        parse(parse_extension(&path), &read_file(path).await?)
    }
}

#[cfg(test)]
mod tests {

    use crate::config::parsers::parse;
    use bytes::Bytes;

    use super::Config;

    #[test]
    fn test_json_deserialize() {
        const TEST_CONFIG: &str = "
            {
                \"servers\": {
                    \"test-server\": {
                        \"name\": \"test\",
                        \"listeners\": [{\"socket_addr\" : \"0.0.0.0:8080\"}],
                        \"routes\": [{
                            \"path\": \"/\",
                            \"upstreams\": [{
                                \"endpoint\": {\"uds_path\":\"/tmp/test\"},
                                \"weight\": 1
                            }, {
                                \"endpoint\": {\"uri\":\"https://gateway.example.com/\"},
                                \"weight\": 2
                            }]
                        }]
                    }
                }
            }
        ";

        let config: Config = parse("json".to_string(), &Bytes::from(TEST_CONFIG)).unwrap();
        assert_eq!("test-server", config.servers.keys().next().unwrap());
    }

    #[test]
    fn test_toml_deserialize() {
        const TEST_CONFIG: &str = "
            [servers.test-server]
            name = 'gateway.example.com'
            listeners = [
                { socket_addr = '[::]:8080' },
                { socket_addr = '0.0.0.0:8080' },
                { uds_path = '/tmp/abc.sock' }
            ]

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

        let config: Config = parse("toml".to_string(), &Bytes::from(TEST_CONFIG)).unwrap();
        assert_eq!("test-server", config.servers.keys().next().unwrap());
    }
}
