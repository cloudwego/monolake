use std::{path::Path, sync::Arc};

use anyhow::Result;
use clap::Parser;
use monolake_core::{
    config::{RuntimeConfig, RuntimeType},
    listener::ListenerBuilder,
    orchestrator::WorkerManager,
};
use service_async::AsyncMakeServiceWrapper;
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, EnvFilter};

use crate::{
    config::{manager::StaticFileConfigManager, Config},
    factory::l7_factory,
    util::print_logo,
};

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
    let mut runtime_config = Config::load_runtime_config(&args.config)?;
    #[cfg(target_os = "linux")]
    if matches!(runtime_config.runtime_type, RuntimeType::IoUring) && !monoio::utils::detect_uring()
    {
        runtime_config.runtime_type = RuntimeType::Legacy;
    }
    match runtime_config.runtime_type {
        #[cfg(target_os = "linux")]
        monolake_core::config::RuntimeType::IoUring => {
            monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
                .enable_timer()
                .build()
                .expect("Failed building the Runtime with IoUringDriver")
                .block_on(run(runtime_config, &args.config));
        }
        monolake_core::config::RuntimeType::Legacy => {
            monoio::RuntimeBuilder::<monoio::LegacyDriver>::new()
                .enable_timer()
                .build()
                .expect("Failed building the Runtime with LegacyDriver")
                .block_on(run(runtime_config, &args.config));
        }
    }
    Ok(())
}

async fn run(runtime_config: RuntimeConfig, service_config_path: impl AsRef<Path>) {
    // Start workers
    let mut manager = WorkerManager::new(runtime_config);
    let join_handlers = manager.spawn_workers_async();
    tracing::info!(
        "Start monolake with {:?} runtime, {} worker(s), {} entries and sqpoll {:?}.",
        manager.config().runtime_type,
        join_handlers.len(),
        manager.config().entries,
        manager.config().sqpoll_idle
    );

    // Create config manager
    let config_manager = StaticFileConfigManager::new(
        manager,
        |config| {
            AsyncMakeServiceWrapper(Arc::new(
                ListenerBuilder::try_from(config).expect("build listener failed"),
            ))
        },
        |config| AsyncMakeServiceWrapper(l7_factory(config)),
    );
    config_manager
        .load_and_watch(&service_config_path)
        .await
        .expect("apply init config failed");
    tracing::info!("init config broadcast successfully");

    // Wait for workers
    for (_, mut close) in join_handlers.into_iter() {
        close.cancellation().await;
    }
}
