use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread, time,
};

use anyhow::Result;
use clap::Parser;
use monolake_core::{
    config::ServiceConfig,
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

#[monoio::main(timer_enabled = true)]
async fn main() -> Result<()> {
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
    
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

    // Read config
    let args = Args::parse();
    let config = Config::load(&args.config).await?;

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

    while !term.load(Ordering::Relaxed) {
        // Do some time-limited stuff here
        // (if this could block forever, then there's no guarantee the signal will have any
        // effect).
        thread::sleep(time::Duration::from_millis(100));
    }
    tracing::info!("SIGTERM received");

    // TODO(ihciah): run update task or api server to do config update, maybe in xDS protocol
    // Wait for workers
    // for (_, mut close) in join_handlers.into_iter() {
    //     close.cancellation().await;
    // }
    Ok(())
}
