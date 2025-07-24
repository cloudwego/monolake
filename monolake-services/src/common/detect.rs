use std::{future::Future, io, io::Cursor};

use monoio::{
    buf::IoBufMut,
    io::{AsyncReadRent, AsyncReadRentExt, PrefixedReadIo},
};
use service_async::Service;

/// Detect is a trait for detecting a certain pattern in the input stream.
///
/// It accepts an input stream and returns a tuple of the detected pattern and the wrapped input
/// stream which is usually a `PrefixedReadIo`. The implementation can choose to whether add the
/// prefix data.
/// If it fails to detect the pattern, it should represent the error inside the `DetOut`.
pub trait Detect<IO> {
    type DetOut;
    type IOOut;

    fn detect(&self, io: IO) -> impl Future<Output = io::Result<(Self::DetOut, Self::IOOut)>>;
}

/// DetectService is a service that detects a certain pattern in the input stream and forwards the
/// detected pattern and the wrapped input stream to the inner service.
pub struct DetectService<D, S> {
    pub detector: D,
    pub inner: S,
}

#[derive(thiserror::Error, Debug)]
pub enum DetectError<E> {
    #[error("service error: {0:?}")]
    Svc(E),
    #[error("io error: {0:?}")]
    Io(std::io::Error),
}

impl<R, S, D, CX> Service<(R, CX)> for DetectService<D, S>
where
    D: Detect<R>,
    S: Service<(D::DetOut, D::IOOut, CX)>,
{
    type Response = S::Response;
    type Error = DetectError<S::Error>;

    async fn call(&self, (io, cx): (R, CX)) -> Result<Self::Response, Self::Error> {
        let (det, io) = self.detector.detect(io).await.map_err(DetectError::Io)?;
        self.inner
            .call((det, io, cx))
            .await
            .map_err(DetectError::Svc)
    }
}

/// FixedLengthDetector detects a fixed length of bytes from the input stream.
pub struct FixedLengthDetector<const N: usize, F>(pub F);

impl<const N: usize, F, IO, DetOut> Detect<IO> for FixedLengthDetector<N, F>
where
    F: Fn(&mut [u8]) -> DetOut,
    IO: AsyncReadRent,
{
    type DetOut = DetOut;
    type IOOut = PrefixedReadIo<IO, Cursor<Vec<u8>>>;

    async fn detect(&self, mut io: IO) -> io::Result<(Self::DetOut, Self::IOOut)> {
        let buf = Vec::with_capacity(N).slice_mut(..N);
        let (r, buf) = io.read_exact(buf).await;
        r?;

        let mut buf = buf.into_inner();
        let r = (self.0)(&mut buf);
        Ok((r, PrefixedReadIo::new(io, Cursor::new(buf))))
    }
}

/// PrefixDetector detects a certain prefix from the input stream.
///
/// If the prefix matches, it returns true and the wrapped input stream with the prefix data.
/// Otherwise, it returns false and the input stream with the prefix data(the prefix maybe less than
/// the static str's length).
pub struct PrefixDetector(pub &'static [u8]);

impl<IO> Detect<IO> for PrefixDetector
where
    IO: AsyncReadRent,
{
    type DetOut = bool;
    type IOOut = PrefixedReadIo<IO, Cursor<Vec<u8>>>;

    async fn detect(&self, mut io: IO) -> io::Result<(Self::DetOut, Self::IOOut)> {
        let l = self.0.len();
        let mut written = 0;
        let mut buf: Vec<u8> = Vec::with_capacity(l);
        let mut eq = true;
        loop {
            // # Safety
            // The buf must have enough capacity to write the data.
            let buf_slice = unsafe { buf.slice_mut_unchecked(written..l) };
            let (result, buf_slice) = io.read(buf_slice).await;
            buf = buf_slice.into_inner();
            match result? {
                0 => {
                    break;
                }
                n => {
                    let curr = written;
                    written += n;
                    if self.0[curr..written] != buf[curr..written] {
                        eq = false;
                        break;
                    }
                }
            }
        }
        let io = PrefixedReadIo::new(io, Cursor::new(buf));
        Ok((eq && written == l, io))
    }
}
