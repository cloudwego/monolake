mod servers;
mod tcp_server;
mod uds_server;
use std::{future::Future, io, rc::Rc};

use anyhow::{bail, Result};
use log::{error, info, warn};
use monoio::io::stream::Stream;
use monolake_core::service::Service;
use monolake_services::common::Accept;
pub use servers::Servers;

use self::{tcp_server::TcpServer, uds_server::UdsServer};

pub trait Server {
    type ServeFuture<'a>: Future<Output = Result<()>>
    where
        Self: 'a;
    type InitFuture<'a>: Future<Output = Result<()>>
    where
        Self: 'a;

    fn serve(&self) -> Self::ServeFuture<'_>;

    fn init(&mut self) -> Self::InitFuture<'_>;
}

#[derive(Debug, Clone)]
pub enum ServerWrapper {
    TcpServer(TcpServer),
    UdsServer(UdsServer),
    Unknown,
}

impl Server for ServerWrapper {
    type ServeFuture<'a> = impl Future<Output = Result<()>> + 'a
    where
        Self: 'a;
    type InitFuture<'a> = impl Future<Output = Result<()>> + 'a
        where
            Self: 'a;

    fn serve(&self) -> Self::ServeFuture<'_> {
        async move {
            match self {
                ServerWrapper::TcpServer(server) => server.serve().await,
                ServerWrapper::UdsServer(server) => server.serve().await,
                ServerWrapper::Unknown => bail!("unimplement!"),
            }
        }
    }

    fn init(&mut self) -> Self::InitFuture<'_> {
        async move {
            match self {
                ServerWrapper::TcpServer(server) => server.init().await,
                ServerWrapper::UdsServer(server) => server.init().await,
                ServerWrapper::Unknown => bail!("unimplement!"),
            }
        }
    }
}

async fn serve<S, Svc, A>(mut listener: S, handler: Rc<Svc>)
where
    S: Stream<Item = io::Result<A>> + 'static,
    Svc: Service<A> + 'static,
    A: 'static,
{
    while let Some(accept) = listener.next().await {
        match accept {
            Ok(accept) => {
                let svc = handler.clone();
                monoio::spawn(async move {
                    match svc.call(accept).await {
                        Ok(_) => {
                            info!("Connection complete");
                        }
                        Err(e) => {
                            error!("Connection error: {e}");
                        }
                    }
                });
            }
            Err(e) => warn!("Accept connection failed: {e}"),
        }
    }
}
