use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use monolake_core::{
    config::{RuntimeType, ServiceConfig},
    listener::ListenerBuilder,
    server::{Command, Manager},
};
use service_async::AsyncMakeServiceWrapper;
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, EnvFilter};

use crate::{config::Config, factory::l7_factory, util::print_logo};

mod config;
mod context;
mod factory;
mod util;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path of the config file
    #[clap(short, long, value_parser)]
    config: String,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();
    #[cfg(feature = "tls")]
    monoio_native_tls::init();
    print_logo();

    let args = Args::parse();
    let mut config = Config::load(args.config)?;
    #[cfg(target_os = "linux")]
    if matches!(config.runtime.runtime_type, RuntimeType::IoUring) && !monoio::utils::detect_uring()
    {
        config.runtime.runtime_type = RuntimeType::Legacy;
    }
    match config.runtime.runtime_type {
        #[cfg(target_os = "linux")]
        monolake_core::config::RuntimeType::IoUring => {
            monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
                .enable_timer()
                .build()
                .expect("Failed building the Runtime with IoUringDriver")
                .block_on(run(config));
        }
        monolake_core::config::RuntimeType::Legacy => {
            monoio::RuntimeBuilder::<monoio::LegacyDriver>::new()
                .enable_timer()
                .build()
                .expect("Failed building the Runtime with LegacyDriver")
                .block_on(run(config));
        }
    }
    Ok(())
}

async fn run(config: Config) {
    // Start workers
    let mut manager = Manager::new(config.runtime);
    let join_handlers = manager.spawn_workers_async();
    tracing::info!(
        "Start monolake with {:?} runtime, {} worker(s), {} entries and sqpoll {:?}.",
        manager.config().runtime_type,
        join_handlers.len(),
        manager.config().entries,
        manager.config().sqpoll_idle
    );

    // Construct Service Factory and Listener Factory
    for (name, ServiceConfig { listener, server }) in config.servers.into_iter() {
        let lis_fac = ListenerBuilder::try_from(listener).expect("build listener failed");

        let svc_fac = l7_factory(server);

        manager
            .apply(Command::Add(
                Arc::new(name),
                AsyncMakeServiceWrapper(svc_fac),
                AsyncMakeServiceWrapper(Arc::new(lis_fac)),
            ))
            .await
            .err()
            .expect("apply init config failed");
    }
    tracing::info!("init config broadcast successfully");

    // TODO(ihciah): run update task or api server to do config update, maybe in xDS protocol
    // Wait for workers
    for (_, mut close) in join_handlers.into_iter() {
        close.cancellation().await;
    }
}
