use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};

// Default iouring/epoll entries: 32k
const DEFAULT_ENTRIES: u32 = 32768;

pub const FALLBACK_PARALLELISM: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1) };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig<LC, SC> {
    pub listener: LC,
    #[serde(flatten)]
    pub server: SC,
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
    pub thread_pool: Option<usize>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        RuntimeConfig {
            worker_threads: default_workers(),
            entries: default_entries(),
            sqpoll_idle: None,
            runtime_type: Default::default(),
            cpu_affinity: default_cpu_affinity(),
            thread_pool: None,
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

macro_rules! define_const {
    ($name: ident, $val: expr, $type: ty) => {
        const fn $name() -> $type {
            $val
        }
    };
}

fn default_workers() -> usize {
    std::thread::available_parallelism()
        .unwrap_or(FALLBACK_PARALLELISM)
        .into()
}

define_const!(default_entries, DEFAULT_ENTRIES, u32);
define_const!(default_cpu_affinity, true, bool);

// #[cfg(test)]
// mod tests {
//     use super::Config;

//     #[test]
//     fn test_json_deserialize() {
//         const TEST_CONFIG: &str =
//             "
//             {
//                 \"servers\": {
//                     \"test-server\": {
//                         \
//              \"name\": \"test\",
//                                    \"listener\": {\"socket_addr\" : \
//              \"0.0.0.0:8080\"},
//                                    \"routes\": [{
//                             \
//              \"path\": \"/\",
//                             \"upstreams\": [{
//                                 \
//              \"endpoint\": {\"uds_path\":\"/tmp/test\"},\"weight\": 1 }, {
//                                 \
//              \"endpoint\": {\"uri\":\"https://gateway.example.com/\"},\"weight\": 2 }] }]
//                     }
//                 }
//             }
//         ";

//         let config = Config::from_slice(TEST_CONFIG.as_bytes()).unwrap();
//         assert_eq!("test-server", config.servers.keys().next().unwrap());
//     }

//     #[test]
//     fn test_toml_deserialize() {
//         const TEST_CONFIG: &str = "
//             [servers.test-server]
//             name = 'gateway.example.com'
//             listener = { socket_addr = '[::]:8080' }

//             [[servers.test-server.routes]]
//             path = '/'
//             id = 'test'

//             [[servers.test-server.routes.upstreams]]
//             endpoint = {uri = 'test'}
//             weight = 1

//             [[servers.test-server.routes.upstreams]]
//             endpoint = {uds_path = '/tmp/def.sock'}
//             weight = 2
//         ";

//         let config: Config = Config::from_slice(TEST_CONFIG.as_bytes()).unwrap();
//         assert_eq!("test-server", config.servers.keys().next().unwrap());
//     }
// }
