use std::{future::Future, io, net::SocketAddr};

use monoio::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    io::{stream::Stream, AsyncReadRent, AsyncWriteRent, Split},
    net::{ListenerOpts, TcpListener, TcpStream},
    BufResult,
};
use service_async::MakeService;

pub enum ListenerBuilder {
    Tcp(SocketAddr, ListenerOpts),
    #[cfg(target_os = "linux")]
    Unix(std::os::unix::net::UnixListener),
}

impl ListenerBuilder {
    #[cfg(target_os = "linux")]
    pub fn bind_unix<P: AsRef<std::path::Path>>(path: P) -> io::Result<ListenerBuilder> {
        // Try remove file first
        let _ = std::fs::remove_file(path.as_ref());
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
            #[cfg(target_os = "linux")]
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
    #[cfg(target_os = "linux")]
    Unix(monoio::net::UnixListener),
}

impl Stream for Listener {
    type Item = io::Result<(AcceptedStream, AcceptedAddr)>;

    type NextFuture<'a> = impl std::future::Future<Output = Option<Self::Item>> + 'a
    where
        Self: 'a;

    fn next(&mut self) -> Self::NextFuture<'_> {
        async move {
            match self {
                Listener::Tcp(l) => match l.next().await {
                    Some(Ok(accepted)) => Some(Ok((
                        AcceptedStream::Tcp(accepted.0),
                        AcceptedAddr::Tcp(accepted.1),
                    ))),
                    Some(Err(e)) => Some(Err(e)),
                    None => None,
                },
                #[cfg(target_os = "linux")]
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
}

pub enum AcceptedStream {
    Tcp(TcpStream),
    #[cfg(target_os = "linux")]
    Unix(monoio::net::UnixStream),
}

unsafe impl Split for AcceptedStream {}

#[derive(Debug, Clone)]
pub enum AcceptedAddr {
    Tcp(SocketAddr),
    #[cfg(target_os = "linux")]
    Unix(monoio::net::unix::SocketAddr),
}

impl From<SocketAddr> for AcceptedAddr {
    fn from(value: SocketAddr) -> Self {
        Self::Tcp(value)
    }
}

#[cfg(target_os = "linux")]
impl From<monoio::net::unix::SocketAddr> for AcceptedAddr {
    fn from(value: monoio::net::unix::SocketAddr) -> Self {
        Self::Unix(value)
    }
}

impl AsyncReadRent for AcceptedStream {
    type ReadFuture<'a, B> = impl Future<Output = BufResult<usize, B>> +'a
    where
        B: IoBufMut + 'a, Self: 'a;
    type ReadvFuture<'a, B> = impl Future<Output = BufResult<usize, B>> + 'a
    where
        B: IoVecBufMut + 'a, Self: 'a;

    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        async move {
            match self {
                AcceptedStream::Tcp(inner) => inner.read(buf).await,
                #[cfg(target_os = "linux")]
                AcceptedStream::Unix(inner) => inner.read(buf).await,
            }
        }
    }

    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        async move {
            match self {
                AcceptedStream::Tcp(inner) => inner.readv(buf).await,
                #[cfg(target_os = "linux")]
                AcceptedStream::Unix(inner) => inner.readv(buf).await,
            }
        }
    }
}

impl AsyncWriteRent for AcceptedStream {
    type WriteFuture<'a, T> = impl Future<Output = BufResult<usize, T>> + 'a
    where
        T: IoBuf + 'a, Self: 'a;

    type WritevFuture<'a, T>= impl Future<Output = BufResult<usize, T>> + 'a where
        T: IoVecBuf + 'a, Self: 'a;

    type FlushFuture<'a> = impl Future<Output = io::Result<()>> + 'a where Self: 'a;

    type ShutdownFuture<'a> = impl Future<Output = io::Result<()>> + 'a where Self: 'a;

    #[inline]
    fn write<T: IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        async move {
            match self {
                AcceptedStream::Tcp(inner) => inner.write(buf).await,
                #[cfg(target_os = "linux")]
                AcceptedStream::Unix(inner) => inner.write(buf).await,
            }
        }
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        async move {
            match self {
                AcceptedStream::Tcp(inner) => inner.writev(buf_vec).await,
                #[cfg(target_os = "linux")]
                AcceptedStream::Unix(inner) => inner.writev(buf_vec).await,
            }
        }
    }

    #[inline]
    fn flush(&mut self) -> Self::FlushFuture<'_> {
        async move {
            match self {
                AcceptedStream::Tcp(inner) => inner.flush().await,
                #[cfg(target_os = "linux")]
                AcceptedStream::Unix(inner) => inner.flush().await,
            }
        }
    }

    #[inline]
    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        async move {
            match self {
                AcceptedStream::Tcp(inner) => inner.shutdown().await,
                #[cfg(target_os = "linux")]
                AcceptedStream::Unix(inner) => inner.shutdown().await,
            }
        }
    }
}
