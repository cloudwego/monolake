use anyhow::{bail, Result};
use log::{error, info, warn};
use monoio::net::{unix::SocketAddr, UnixListener, UnixStream};
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
    tls::{NativeTlsService, RustlsService},
    uds::UdsListenerService,
};

use std::{
    cell::UnsafeCell,
    future::Future,
    os::{
        fd::{FromRawFd, IntoRawFd, RawFd},
        unix::net::UnixListener as StdUnixListener,
    },
    path::PathBuf,
    rc::Rc,
};

use monoio_http_client::Client;
use tower_layer::Layer;

use super::Server;

#[derive(Debug, Clone)]
pub struct UdsServer {
    name: String,
    addr: PathBuf,
    routes: Vec<Route>,
    tls: Option<TlsConfig>,
    listener: Option<RawFd>,
    keepalive_config: Option<KeepaliveConfig>,
}

impl UdsServer {
    pub fn new(
        name: String,
        addr: PathBuf,
        routes: Vec<Route>,
        tls: Option<TlsConfig>,
        keepalive_config: Option<KeepaliveConfig>,
    ) -> UdsServer {
        UdsServer {
            name,
            addr,
            routes,
            tls,
            listener: None,
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

    #[allow(unreachable_code, unused_assignments, unused_variables, unused_unsafe)]
    async fn listener_loop<Handler>(
        &self,
        handler: Rc<UnsafeCell<Handler>>,
    ) -> Result<(), anyhow::Error>
    where
        Handler: Service<Accept<UnixStream, SocketAddr>> + 'static,
    {
        let listener: Rc<UnixListener> = match self.listener {
            Some(raw_fd) => unsafe {
                Rc::new(UnixListener::from_std(StdUnixListener::from_raw_fd(
                    raw_fd,
                ))?)
            },
            None => bail!("The raw fd is not exist for the uds listener"),
        };

        // let listener = Rc::new(listener);
        let mut svc = UdsListenerService::default();
        loop {
            info!("Accepting new connection from {:?}", self.addr.clone());
            let handler = handler.clone();

            match svc.call(listener.clone()).await {
                Ok(accept) => {
                    monoio::spawn(async move {
                        match unsafe { &mut *handler.get() }.call(accept).await {
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
                    std::thread::sleep(std::time::Duration::from_secs(10));
                }
            }
        }
    }
}

impl Server for UdsServer {
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
                        self.listener_loop(Rc::new(UnsafeCell::new(service))).await
                    }
                    TlsStack::NativeTls => {
                        let service = NativeTlsService::layer(self.name.clone()).layer(service);
                        self.listener_loop(Rc::new(UnsafeCell::new(service))).await
                    }
                },
                None => self.listener_loop(Rc::new(UnsafeCell::new(service))).await,
            }
        }
    }

    fn init(&mut self) -> Self::InitFuture<'_> {
        async {
            self.configure_cert()?;
            if self.addr.exists() {
                std::fs::remove_file(self.addr.clone())?
            }
            let listener = StdUnixListener::bind(self.addr.clone())?;
            self.listener = Some(listener.into_raw_fd());
            Ok(())
        }
    }
}
