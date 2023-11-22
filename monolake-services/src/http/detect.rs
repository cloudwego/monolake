use std::io::Cursor;

use monoio::{
    buf::IoBufMut,
    io::{AsyncReadRent, AsyncWriteRent, PrefixedReadIo},
};
use monolake_core::{http::HttpAccept, AnyError};
use service_async::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Service,
};

use crate::tcp::Accept;

const PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

#[derive(Clone)]
pub struct HttpVersionDetect<T> {
    inner: T,
}

impl<F> MakeService for HttpVersionDetect<F>
where
    F: MakeService,
{
    type Service = HttpVersionDetect<F::Service>;
    type Error = F::Error;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(HttpVersionDetect {
            inner: self.inner.make_via_ref(old.map(|o| &o.inner))?,
        })
    }
}

impl<F> HttpVersionDetect<F> {
    pub fn layer<C>() -> impl FactoryLayer<C, F, Factory = Self> {
        layer_fn(|_: &C, inner| HttpVersionDetect { inner })
    }
}

impl<T, Stream, CX> Service<Accept<Stream, CX>> for HttpVersionDetect<T>
where
    Stream: AsyncReadRent + AsyncWriteRent,
    T: Service<HttpAccept<PrefixedReadIo<Stream, Cursor<Vec<u8>>>, CX>>,
    T::Error: Into<AnyError>,
{
    type Response = T::Response;
    type Error = AnyError;

    async fn call(
        &self,
        incoming_stream: Accept<Stream, CX>,
    ) -> Result<Self::Response, Self::Error> {
        let (mut stream, addr) = incoming_stream;
        let mut buf = vec![0; PREFACE.len()];
        let mut pos = 0;
        let mut h2_detect = false;

        loop {
            let buf_slice = unsafe { buf.slice_mut_unchecked(pos..PREFACE.len()) };
            let (result, buf_slice) = stream.read(buf_slice).await;
            buf = buf_slice.into_inner();
            match result {
                Ok(0) => {
                    break;
                }
                Ok(n) => {
                    if PREFACE[pos..pos + n] != buf[pos..pos + n] {
                        break;
                    }
                    pos += n;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }

            if pos == PREFACE.len() {
                h2_detect = true;
                break;
            }
        }

        let preface_buf = std::io::Cursor::new(buf);
        let rewind_io = monoio::io::PrefixedReadIo::new(stream, preface_buf);

        self.inner
            .call((h2_detect, rewind_io, addr))
            .await
            .map_err(Into::into)
    }
}
