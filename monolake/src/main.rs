#![feature(impl_trait_in_assoc_type)]

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use clap::Parser;

use monoio::net::TcpStream;
use monolake_core::{
    config::RuntimeConfig,
    listener::ListenerBuilder,
    print_logo,
    service::{stack::FactoryStack, KeepFirst, Param},
};
use monolake_services::{
    tcp::toy_echo::{EchoReplaceConfig, EchoReplaceService},
    tls::{TlsConfig, UnifiedTlsFactory},
};
use server::Manager;
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, EnvFilter};
// use runtimes::Runtimes;
// use servers::Servers;
// mod runtimes;
// mod servers;

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
    print_logo();

    // Start workers
    let runtime_config = RuntimeConfig::default();
    let mut manager = Manager::new(runtime_config);
    let join_handlers = manager.spawn_workers();
    tracing::info!("{} workers started", join_handlers.len());

    // Construct Service Factory and Listener Factory
    let demo_config = DemoConfig::default();
    let factory_chain = FactoryStack::new(demo_config)
        .push(EchoReplaceService::layer())
        .check_make_svc::<TcpStream>()
        .push_map_target(KeepFirst)
        .push(UnifiedTlsFactory::layer())
        .into_inner();
    let listener_factory_unified =
        ListenerBuilder::Tcp("0.0.0.0:8080".parse().unwrap(), Default::default());

    // Broadcast Add command to worker threads
    let broadcast_result = manager
        .apply(server::Command::Add(
            "demo".to_string(),
            Arc::new(factory_chain),
            Arc::new(listener_factory_unified),
        ))
        .await;
    for r in broadcast_result.into_iter() {
        r.unwrap();
    }
    tracing::info!("broadcast site add to workers successfully");

    // Wait for 10 secs and update the service.
    monoio::time::sleep(Duration::from_secs(10)).await;
    let demo_config_new = DemoConfig {
        echo_replace: b'B',
        ..Default::default()
    };
    let factory_chain = FactoryStack::new(demo_config_new)
        .push(EchoReplaceService::layer())
        .check_make_svc::<TcpStream>()
        .push_map_target(KeepFirst)
        .push(UnifiedTlsFactory::layer())
        .into_inner();
    let broadcast_result = manager
        .apply(server::Command::Update(
            "demo".to_string(),
            Arc::new(factory_chain),
        ))
        .await;
    for r in broadcast_result.into_iter() {
        r.unwrap();
    }
    tracing::info!("broadcast site update to workers successfully");

    // Wait for 10 secs and remove the service.
    monoio::time::sleep(Duration::from_secs(10)).await;
    let broadcast_result = manager
        .apply(server::Command::Remove("demo".to_string()))
        .await;
    for r in broadcast_result.into_iter() {
        r.unwrap();
    }
    tracing::info!("broadcast site remove to workers successfully");

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
