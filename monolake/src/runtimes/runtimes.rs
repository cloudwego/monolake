use std::thread;

use anyhow::Result;

use log::info;
use monoio::utils::bind_to_cpu_set;
use monolake_core::{config::RuntimeConfig, max_parallel_count};

use super::RuntimeWrapper;
use crate::servers::Server;

#[derive(Debug, Clone)]
pub struct Runtimes {
    config: RuntimeConfig,
}

impl Runtimes {
    pub fn new(config: RuntimeConfig) -> Self {
        Runtimes { config }
    }

    fn bind_cpu(cpu_affinity: bool, worker: usize) {
        if !cpu_affinity {
            return;
        }

        let cpu_counts = max_parallel_count().get();
        let cpu_id = worker % cpu_counts;
        bind_to_cpu_set(vec![cpu_id]).unwrap();
    }

    pub fn execute<S>(&self, server: &S) -> Result<()>
    where
        S: Server + Clone + Send + 'static,
    {
        let mut handlers = vec![];

        info!(
            "Start monolake with {:?} runtime, {} worker(s), {} entries and sqpoll {:?}.",
            self.config.runtime_type,
            self.config.workers,
            self.config.entries,
            self.config.sqpoll_idle
        );

        (0..self.config.workers).for_each(|worker| {
            let server = server.clone();
            let config = self.config.clone();
            handlers.push(thread::spawn(move || {
                let mut runtime = RuntimeWrapper::from(&config);
                Runtimes::bind_cpu(config.cpu_affinity, worker as usize);
                runtime.exec(server.serve())
            }));
        });

        handlers.into_iter().for_each(|handler| {
            let _ = handler.join();
        });

        Ok(())
    }
}
