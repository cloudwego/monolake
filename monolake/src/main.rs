#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use clap::Parser;

use monoio::net::TcpStream;
use monolake_core::{
    config::{Config, RuntimeConfig},
    listener::{AcceptedAddr, AcceptedStream, ListenerBuilder},
    print_logo,
    service::{stack::FactoryStack, Param},
    tls::TlsConfig,
};
use monolake_services::{
    http::{
        handlers::{ConnReuseHandler, ProxyHandler, RewriteHandler},
        HttpCoreService,
    },
    tcp::toy_echo::EchoReplaceConfig,
    tls::UnifiedTlsFactory,
};
use server::Manager;
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, EnvFilter};

// use crate::factory::l7_factory;

// mod factory;
mod server;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path of the config file
    #[clap(short, long, value_parser)]
    config: String,
}

#[monoio::main(timer_enabled = true)]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();
    monoio_native_tls::init();
    print_logo();

    // Read config
    let args = Args::parse();
    let config = Config::load(&args.config).await?;

    // Start workers
    let runtime_config = RuntimeConfig::default();
    let mut manager = Manager::new(runtime_config);
    let join_handlers = manager.spawn_workers();
    tracing::info!(
        "Start monolake with {:?} runtime, {} worker(s), {} entries and sqpoll {:?}.",
        manager.config().runtime_type,
        join_handlers.len(),
        manager.config().entries,
        manager.config().sqpoll_idle
    );

    // Construct Service Factory and Listener Factory
    for (name, (lis_cfg, svc_cfg)) in config.servers.into_iter() {
        let lis_fac = ListenerBuilder::try_from(lis_cfg).expect("build listener failed");
        // let svc_fac = l7_factory(svc_cfg);
        let svc_fac = FactoryStack::new(svc_cfg)
            .replace(ProxyHandler::factory())
            .push(ConnReuseHandler::layer())
            .push(RewriteHandler::layer())
            .push(HttpCoreService::layer())
            .check_make_svc::<(TcpStream, SocketAddr)>()
            .push(UnifiedTlsFactory::layer())
            .check_make_svc::<(AcceptedStream, AcceptedAddr)>()
            .into_inner();
        manager
            .apply(server::Command::Add(
                name,
                Arc::new(svc_fac),
                Arc::new(lis_fac),
            ))
            .await
            .err()
            .expect("apply init config failed");
    }

    // Wait for workers
    join_handlers.into_iter().for_each(|h| {
        h.join().unwrap();
    });
    Ok(())
}

pub struct DemoConfig {
    echo_replace: u8,
    tls: TlsConfig,
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            echo_replace: b'A',
            tls: TlsConfig::None,
        }
    }
}

impl Param<EchoReplaceConfig> for DemoConfig {
    fn param(&self) -> EchoReplaceConfig {
        EchoReplaceConfig {
            replace: self.echo_replace,
        }
    }
}

impl Param<TlsConfig> for DemoConfig {
    fn param(&self) -> TlsConfig {
        self.tls.clone()
    }
}
