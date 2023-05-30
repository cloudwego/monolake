use std::{io, net::SocketAddr, path::Path};

use monoio::{
    io::stream::Stream,
    net::{ListenerOpts, TcpListener, TcpStream},
};

use crate::service::MakeService;

pub enum ListenerBuilder {
    Tcp(SocketAddr, ListenerOpts),
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixListener),
}

impl ListenerBuilder {
    #[cfg(unix)]
    pub fn bind_unix<P: AsRef<Path>>(path: P) -> io::Result<ListenerBuilder> {
        let listener = std::os::unix::net::UnixListener::bind(path)?;
        Ok(Self::Unix(listener))
    }

    pub fn bind_tcp(addr: SocketAddr, opts: ListenerOpts) -> io::Result<ListenerBuilder> {
        Ok(Self::Tcp(addr, opts))
    }

    pub fn build(&self) -> io::Result<Listener> {
        match self {
            ListenerBuilder::Tcp(addr, opts) => {
                TcpListener::bind_with_config(addr, opts).map(Listener::Tcp)
            }
            #[cfg(unix)]
            ListenerBuilder::Unix(listener) => {
                let sys_listener = listener.try_clone()?;
                monoio::net::UnixListener::from_std(sys_listener).map(Listener::Unix)
            }
        }
    }
}

impl MakeService for ListenerBuilder {
    type Service = Listener;
    type Error = io::Error;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        self.build()
    }
}

/// Unified listener.
pub enum Listener {
    Tcp(TcpListener),
    #[cfg(unix)]
    Unix(monoio::net::UnixListener),
}

impl Stream for Listener {
    type Item = io::Result<Accepted>;

    type NextFuture<'a> = impl std::future::Future<Output = Option<Self::Item>> + 'a
    where
        Self: 'a;

    fn next(&mut self) -> Self::NextFuture<'_> {
        async move {
            match self {
                Listener::Tcp(l) => match l.next().await {
                    Some(Ok(accepted)) => Some(Ok(Accepted::Tcp(accepted.0, accepted.1))),
                    Some(Err(e)) => Some(Err(e)),
                    None => None,
                },
                #[cfg(unix)]
                Listener::Unix(l) => match l.next().await {
                    Some(Ok(accepted)) => Some(Ok(Accepted::Unix(accepted.0, accepted.1))),
                    Some(Err(e)) => Some(Err(e)),
                    None => None,
                },
            }
        }
    }
}

pub enum Accepted {
    Tcp(TcpStream, SocketAddr),
    #[cfg(unix)]
    Unix(monoio::net::UnixStream, monoio::net::unix::SocketAddr),
}
