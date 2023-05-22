use std::{future::Future, net::SocketAddr, rc::Rc};

use anyhow::{bail, Result};
use log::{error, info, warn};
use monoio::net::{ListenerConfig, TcpListener, TcpStream};
use monolake_core::{
    config::{KeepaliveConfig, Route, TlsConfig, TlsStack},
    service::{Service, ServiceLayer},
    tls::update_certificate,
};
use monolake_services::{
    common::Accept,
    http::{
        handlers::{ConnReuseHandler, ProxyHandler, RewriteHandler},
        HttpCoreService,
    },
    tcp::TcpListenerService,
    tls::{NativeTlsService, RustlsService},
};

use monoio_http_client::Client;
use tower_layer::Layer;

use super::Server;

#[derive(Debug, Clone)]
pub struct TcpServer {
    name: String,
    addr: SocketAddr,
    routes: Vec<Route>,
    tls: Option<TlsConfig>,
    keepalive_config: Option<KeepaliveConfig>,
}

impl TcpServer {
    pub fn new(
        name: String,
        addr: SocketAddr,
        routes: Vec<Route>,
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

    async fn listener_loop<Handler>(&self, handler: Rc<Handler>) -> Result<(), anyhow::Error>
    where
        Handler: Service<Accept<TcpStream, SocketAddr>> + 'static,
    {
        let addr = self.addr;
        let listener = TcpListener::bind_with_config(addr, &ListenerConfig::default());
        if let Err(e) = listener {
            bail!("Error when binding address({})", e);
        }
        let listener = Rc::new(listener.unwrap());
        let svc = TcpListenerService;
        loop {
            info!("Accepting new connection from {:?}", addr);
            let handler = handler.clone();

            match svc.call(listener.clone()).await {
                Ok(accept) => {
                    monoio::spawn(async move {
                        match handler.call(accept).await {
                            Ok(_) => {
                                info!("Connection complete");
                            }
                            Err(e) => {
                                error!("Connection error: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    warn!("Accept connection failed: {}", e);
                }
            }
        }
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
                Some(tls) => match tls.stack {
                    TlsStack::Rustls => {
                        let service = RustlsService::layer(self.name.clone()).layer(service);
                        self.listener_loop(Rc::new(service)).await
                    }
                    TlsStack::NativeTls => {
                        let service = NativeTlsService::layer(self.name.clone()).layer(service);
                        self.listener_loop(Rc::new(service)).await
                    }
                },
                None => self.listener_loop(Rc::new(service)).await,
            }
        }
    }

    fn init(&mut self) -> Self::InitFuture<'_> {
        async { self.configure_cert() }
    }
}
