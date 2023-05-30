use std::{collections::HashMap, future::Future};

use anyhow::Result;
use monoio::task::JoinHandle;
use monolake_core::{
    config::{ListenerConfig, ServerConfig as ServerConfig, TransportProtocol},
    util::hash::sha256,
};

use crate::runtimes::Runtimes;

use super::{tcp_server::TcpServer, uds_server::UdsServer, Server, ServerWrapper};

#[derive(Debug, Clone)]
pub struct Servers {
    servers: Vec<ServerWrapper>,
}

impl Servers {
    pub async fn start(&mut self, runtimes: Runtimes) -> Result<()> {
        self.init().await?;
        runtimes.execute(self)
    }
}

impl From<HashMap<String, ServerConfig>> for Servers {
    fn from(servers: HashMap<String, ServerConfig>) -> Self {
        let servers: Vec<ServerWrapper> = servers
            .into_values()
            .map(|server_config| {
                server_config
                    .listeners
                    .into_iter()
                    .map(|listener| {
                        let routes = server_config
                            .routes
                            .iter()
                            .map(|route| {
                                let mut route = route.clone();
                                route.id = sha256(&route.path);
                                route
                            })
                            .collect();

                        match listener.transport_protocol() {
                            TransportProtocol::Tcp => match listener {
                                ListenerConfig::SocketAddress(addr) => {
                                    ServerWrapper::TcpServer(TcpServer::new(
                                        server_config.name.to_owned(),
                                        addr.socket_addr,
                                        routes,
                                        server_config.tls.to_owned(),
                                        server_config.keepalive_config.to_owned(),
                                    ))
                                }
                                ListenerConfig::Uds(addr) => ServerWrapper::UdsServer(UdsServer::new(
                                    server_config.name.to_owned(),
                                    addr.uds_path,
                                    routes,
                                    server_config.tls.to_owned(),
                                    server_config.keepalive_config.to_owned(),
                                )),
                            },
                            _ => ServerWrapper::Unknown,
                        }
                    })
                    .collect::<Vec<ServerWrapper>>()
            })
            .flatten()
            .collect();

        Servers { servers }
    }
}

impl Server for Servers {
    type ServeFuture<'a> = impl Future<Output = Result<()>> + 'a where Self:'a;
    type InitFuture<'a> = impl Future<Output = Result<()>> + 'a where Self:'a;

    fn serve(&self) -> Self::ServeFuture<'_> {
        async {
            let tasks = self
                .servers
                .clone()
                .into_iter()
                .map(|server| {
                    monoio::spawn(async move {
                        if let Err(e) = server.serve().await {
                            tracing::error!("Serve Error: {}", e);
                        }
                    })
                })
                .collect::<Vec<JoinHandle<()>>>();

            for task in tasks {
                let _ = task.await;
            }

            Ok(())
        }
    }

    fn init(&mut self) -> Self::InitFuture<'_> {
        async move {
            // init logging
            env_logger::init();
            // init openssl engine
            monoio_native_tls::init();

            for server in &mut self.servers {
                server.init().await?;
            }

            Ok(())
        }
    }
}
