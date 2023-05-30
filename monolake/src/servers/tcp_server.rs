use std::{future::Future, net::SocketAddr, rc::Rc, fmt::Display};

use anyhow::{anyhow, bail, Result};
use tracing::info;
use monoio::net::{ListenerConfig, TcpListener, TcpStream};
use monolake_core::{
    config::{KeepaliveConfig, RouteConfig, TlsConfig, TlsStack},
    service::{Service, ServiceLayer},
    tls::update_certificate,
};
use monolake_services::{
    common::Accept,
    http::{
        handlers::{ConnReuseHandler, ProxyHandler, RewriteHandler},
        HttpCoreService,
    },
    tls::{NativeTlsService, RustlsService},
};

use monoio_http_client::Client;
use tower_layer::Layer;

use super::Server;

#[derive(Debug, Clone)]
pub struct TcpServer {
    name: String,
    addr: SocketAddr,
    routes: Vec<RouteConfig>,
    tls: Option<TlsConfig>,
    keepalive_config: Option<KeepaliveConfig>,
}

impl TcpServer {
    pub fn new(
        name: String,
        addr: SocketAddr,
        routes: Vec<RouteConfig>,
        tls: Option<TlsConfig>,
        keepalive_config: Option<KeepaliveConfig>,
    ) -> TcpServer {
        TcpServer {
            name,
            addr,
            routes,
            tls,
            keepalive_config,
        }
    }

    pub fn configure_cert(&self) -> Result<()> {
        if self.tls.is_none() {
            // non cert configured.
            return Ok(());
        }

        let tls = self.tls.as_ref().unwrap();

        let (pem_content, key_content) = (
            std::fs::read(tls.chain.clone()),
            std::fs::read(tls.key.clone()),
        );

        if pem_content.is_err() || key_content.is_err() {
            bail!(
                "server: {}, private key read error: {}, certificate chain read error: {}",
                self.name,
                key_content.is_err(),
                pem_content.is_err()
            );
        }

        update_certificate(
            self.name.to_owned(),
            pem_content.unwrap(),
            key_content.unwrap(),
        );

        info!("🚀 ssl certificates for {} loaded.", self.name);

        Ok(())
    }

    async fn listener_loop<Svc>(&self, handler: Rc<Svc>) -> Result<(), anyhow::Error>
    where
        Svc: Service<Accept<TcpStream, SocketAddr>> + 'static,
        Svc::Error: Display,
    {
        let addr = self.addr;
        let listener = TcpListener::bind_with_config(addr, &ListenerConfig::default());
        let listener = listener.map_err(|e| anyhow!("Error when binding address({e})"))?;
        super::serve(listener, handler).await;
        Ok(())
    }
}

impl Server for TcpServer {
    type ServeFuture<'a> = impl Future<Output = Result<()>> + 'a
    where
        Self: 'a;
    type InitFuture<'a> = impl Future<Output = Result<()>> + 'a
        where
            Self: 'a;

    fn serve(&self) -> Self::ServeFuture<'_> {
        async move {
            let client = Rc::new(Client::default());
            let service = HttpCoreService::new(
                (
                    RewriteHandler::layer(Rc::new(self.routes.clone())),
                    ConnReuseHandler::layer(self.keepalive_config.clone()),
                )
                    .layer(ProxyHandler::new(client.clone())),
                self.keepalive_config.clone(),
            );

            match &self.tls {
                Some(tls) => {todo!() },
                // match tls.stack {
                //     TlsStack::Rustls => {
                //         let service = RustlsService::layer(self.name.clone()).layer(service);
                //         self.listener_loop(Rc::new(service)).await
                //     }
                //     TlsStack::NativeTls => {
                //         let service = NativeTlsService::layer(self.name.clone()).layer(service);
                //         self.listener_loop(Rc::new(service)).await
                //     }
                // },
                None => self.listener_loop(Rc::new(service)).await,
            }
        }
    }

    fn init(&mut self) -> Self::InitFuture<'_> {
        async { self.configure_cert() }
    }
}
