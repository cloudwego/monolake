//! Runtime configuration and service setup for asynchronous networking applications.
//!
//! This module provides structures and enums for configuring the runtime environment
//! and services in networking applications. It includes options for worker threads,
//! I/O event handling, and runtime type selection.
//!
//! # Key Components
//!
//! - [`ServiceConfig`]: A generic configuration structure for services.
//! - [`RuntimeConfig`]: Configuration options for the runtime environment.
//! - [`RuntimeType`]: Enum representing different runtime implementation options.
use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};

// Default iouring/epoll entries: 32k
const DEFAULT_ENTRIES: u32 = 32768;

pub const FALLBACK_PARALLELISM: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1) };

/// Configuration structure for a service, combining listener and server configs.
///
/// # Type Parameters
///
/// - `LC`: The type of the listener configuration.
/// - `SC`: The type of the server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig<LC, SC> {
    /// Configuration for the service listener.
    pub listener: LC,
    /// Configuration for the server component of the service.
    #[serde(flatten)]
    pub server: SC,
}

/// Configuration options for the runtime environment.
///
/// This structure allows for fine-tuning of the runtime, including worker threads,
/// I/O multiplexing, and CPU affinity settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Number of worker threads for the runtime.
    #[serde(default = "default_workers")]
    pub worker_threads: usize,

    /// Number of I/O entries for event handling for io_uring.
    #[serde(default = "default_entries")]
    pub entries: u32,

    /// Idle timeout for squall polling (io_uring specific).
    pub sqpoll_idle: Option<u32>,

    /// The type of runtime to use.
    #[serde(default)]
    pub runtime_type: RuntimeType,

    /// Whether to enable CPU affinity for worker threads.
    #[serde(default = "default_cpu_affinity")]
    pub cpu_affinity: bool,

    /// Optional thread pool size for specific runtime implementations.
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

/// Enum representing different runtime implementation options.
///
/// This allows for selection between different runtime backends,
/// such as io_uring on Linux or a legacy implementation on other platforms.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeType {
    /// io_uring-based runtime (Linux only).
    #[cfg(target_os = "linux")]
    IoUring,

    /// Legacy runtime implementation.
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
define_const!(default_cpu_affinity, false, bool);

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
