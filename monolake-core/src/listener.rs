use std::{io, net::SocketAddr, path::Path};

use monoio::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    io::{stream::Stream, AsyncReadRent, AsyncWriteRent, Split},
    net::{ListenerOpts, TcpListener, TcpStream},
    BufResult,
};
use service_async::{AsyncMakeService, MakeService};

pub enum ListenerBuilder {
    Tcp(SocketAddr, ListenerOpts),
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixListener),
}

impl ListenerBuilder {
    #[cfg(unix)]
    pub fn bind_unix<P: AsRef<Path>>(path: P) -> io::Result<ListenerBuilder> {
        // Try remove file first
        let _ = std::fs::remove_file(path.as_ref());
        let listener = std::os::unix::net::UnixListener::bind(path)?;
        // Because we use std and build async UnixStream form raw fd, we
        // have to make sure it is non_blocking.
        if monoio::utils::is_legacy() {
            listener.set_nonblocking(true)?;
        }
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

impl AsyncMakeService for ListenerBuilder {
    type Service = Listener;
    type Error = io::Error;

    async fn make_via_ref(
        &self,
        _old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
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
    type Item = io::Result<(AcceptedStream, AcceptedAddr)>;

    async fn next(&mut self) -> Option<Self::Item> {
        match self {
            Listener::Tcp(l) => match l.next().await {
                Some(Ok(accepted)) => Some(Ok((
                    AcceptedStream::Tcp(accepted.0),
                    AcceptedAddr::Tcp(accepted.1),
                ))),
                Some(Err(e)) => Some(Err(e)),
                None => None,
            },
            #[cfg(unix)]
            Listener::Unix(l) => match l.next().await {
                Some(Ok(accepted)) => Some(Ok((
                    AcceptedStream::Unix(accepted.0),
                    AcceptedAddr::Unix(accepted.1),
                ))),
                Some(Err(e)) => Some(Err(e)),
                None => None,
            },
        }
    }
}

pub enum AcceptedStream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(monoio::net::UnixStream),
}

unsafe impl Split for AcceptedStream {}

#[derive(Debug, Clone)]
pub enum AcceptedAddr {
    Tcp(SocketAddr),
    #[cfg(unix)]
    Unix(monoio::net::unix::SocketAddr),
}

impl From<SocketAddr> for AcceptedAddr {
    fn from(value: SocketAddr) -> Self {
        Self::Tcp(value)
    }
}

#[cfg(unix)]
impl From<monoio::net::unix::SocketAddr> for AcceptedAddr {
    fn from(value: monoio::net::unix::SocketAddr) -> Self {
        Self::Unix(value)
    }
}

impl AsyncReadRent for AcceptedStream {
    async fn read<T: IoBufMut>(&mut self, buf: T) -> BufResult<usize, T> {
        match self {
            AcceptedStream::Tcp(inner) => inner.read(buf).await,
            AcceptedStream::Unix(inner) => inner.read(buf).await,
        }
    }

    async fn readv<T: IoVecBufMut>(&mut self, buf: T) -> BufResult<usize, T> {
        match self {
            AcceptedStream::Tcp(inner) => inner.readv(buf).await,
            AcceptedStream::Unix(inner) => inner.readv(buf).await,
        }
    }
}

impl AsyncWriteRent for AcceptedStream {
    #[inline]
    async fn write<T: IoBuf>(&mut self, buf: T) -> BufResult<usize, T> {
        match self {
            AcceptedStream::Tcp(inner) => inner.write(buf).await,
            AcceptedStream::Unix(inner) => inner.write(buf).await,
        }
    }

    #[inline]
    async fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> BufResult<usize, T> {
        match self {
            AcceptedStream::Tcp(inner) => inner.writev(buf_vec).await,
            AcceptedStream::Unix(inner) => inner.writev(buf_vec).await,
        }
    }

    #[inline]
    async fn flush(&mut self) -> io::Result<()> {
        match self {
            AcceptedStream::Tcp(inner) => inner.flush().await,
            AcceptedStream::Unix(inner) => inner.flush().await,
        }
    }

    #[inline]
    async fn shutdown(&mut self) -> io::Result<()> {
        match self {
            AcceptedStream::Tcp(inner) => inner.shutdown().await,
            AcceptedStream::Unix(inner) => inner.shutdown().await,
        }
    }
}
