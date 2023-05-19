use std::future::Future;

use anyhow::Result;

#[cfg(all(target_os = "linux"))]
use monoio::IoUringDriver;

#[cfg(all(target_os = "linux"))]
use monolake_core::MIN_SQPOLL_IDLE_TIME;

use monoio::{time::TimeDriver, LegacyDriver, Runtime, RuntimeBuilder};
use monolake_core::config::{RuntimeConfig, RuntimeType};

pub enum RuntimeWrapper {
    #[cfg(all(target_os = "linux"))]
    IoUring(Runtime<TimeDriver<IoUringDriver>>),
    Legacy(Runtime<TimeDriver<LegacyDriver>>),
}

impl From<&RuntimeConfig> for RuntimeWrapper {
    fn from(config: &RuntimeConfig) -> Self {
        match config.runtime_type {
            #[cfg(all(target_os = "linux"))]
            RuntimeType::IoUring => {
                let builder = match config.sqpoll_idle {
                    Some(idle) => {
                        let builder = RuntimeBuilder::<monoio::IoUringDriver>::new();
                        let idle = MIN_SQPOLL_IDLE_TIME.max(idle);
                        let mut uring_builder = io_uring::IoUring::builder();
                        uring_builder.setup_sqpoll(idle);
                        builder.uring_builder(&uring_builder)
                    }
                    None => RuntimeBuilder::<monoio::IoUringDriver>::new(),
                };
                let runtime = builder
                    .enable_timer()
                    .with_entries(config.entries)
                    .build()
                    .unwrap();
                RuntimeWrapper::IoUring(runtime)
            }
            RuntimeType::Legacy => {
                let runtime = RuntimeBuilder::<monoio::LegacyDriver>::new()
                    .enable_timer()
                    .with_entries(config.entries)
                    .build()
                    .unwrap();
                RuntimeWrapper::Legacy(runtime)
            }
        }
    }
}

impl RuntimeWrapper {
    pub fn exec<F>(&mut self, future: F) -> F::Output
    where
        F: Future<Output = Result<()>>,
    {
        match self {
            #[cfg(all(target_os = "linux"))]
            RuntimeWrapper::IoUring(driver) => driver.block_on(future),
            RuntimeWrapper::Legacy(driver) => driver.block_on(future),
        }
    }
}
