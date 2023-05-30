#![feature(impl_trait_in_assoc_type)]

use std::{io, net::SocketAddr, sync::Arc};

use anyhow::Result;
use clap::Parser;

use monoio::net::{TcpListener, TcpStream};
use monolake_core::{
    config::RuntimeConfig,
    print_logo,
    service::{stack::FactoryStack, KeepFirst, MakeService, Param},
};
use monolake_services::{
    tcp::echo::{EchoConfig, EchoService},
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
        .push(EchoService::layer())
        .check_make_svc::<TcpStream>()
        .push_map_target(KeepFirst)
        .push(UnifiedTlsFactory::layer())
        .into_inner();
    let listener_factory = SimpleTcpListenerBuilder {
        addr: "0.0.0.0:8080".parse().unwrap(),
    };

    // Broadcast Add command to worker threads
    let broadcast_result = manager
        .apply(server::Command::Add(
            "demo".to_string(),
            Arc::new(factory_chain),
            Arc::new(listener_factory),
        ))
        .await;
    for r in broadcast_result.into_iter() {
        r.unwrap();
    }
    tracing::info!("broadcast site factory to workers successfully");

    // Wait for workers
    join_handlers.into_iter().for_each(|h| {
        h.join().unwrap();
    });
    Ok(())
}

pub struct DemoConfig {
    echo_buffer_size: usize,
    tls: TlsConfig,
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            echo_buffer_size: 1024,
            tls: TlsConfig::None,
        }
    }
}

impl Param<EchoConfig> for DemoConfig {
    fn param(&self) -> EchoConfig {
        EchoConfig {
            buffer_size: self.echo_buffer_size,
        }
    }
}

impl Param<TlsConfig> for DemoConfig {
    fn param(&self) -> TlsConfig {
        self.tls.clone()
    }
}

#[derive(Clone)]
struct SimpleTcpListenerBuilder {
    addr: SocketAddr,
}

impl MakeService for SimpleTcpListenerBuilder {
    type Service = TcpListener;
    type Error = io::Error;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        TcpListener::bind(self.addr)
    }
}
