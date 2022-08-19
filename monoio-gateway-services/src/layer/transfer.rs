use std::future::Future;

use log::info;
use monoio::net::{
    tcp::{TcpOwnedReadHalf, TcpOwnedWriteHalf},
    TcpStream,
};
use monoio_gateway_core::{
    error::GError,
    service::Service,
    transfer::{copy_data, copy_stream_sink},
};
use monoio_http::h1::codec::{
    decoder::{RequestDecoder, ResponseDecoder},
    encoder::GenericEncoder,
};

#[derive(Default, Clone)]
pub struct HttpTransferService;

#[derive(Default, Clone)]
pub struct TcpTransferService;

pub type TcpTransferParams = (TcpStream, TcpStream);

pub type HttpTransferParams = (
    GenericEncoder<TcpOwnedWriteHalf>,
    RequestDecoder<TcpOwnedReadHalf>,
    GenericEncoder<TcpOwnedWriteHalf>,
    ResponseDecoder<TcpOwnedReadHalf>,
);

pub type HttpsTransferParams = (
    GenericEncoder<monoio_rustls::ServerTlsStreamWriteHalf<TcpStream>>,
    RequestDecoder<monoio_rustls::ServerTlsStreamReadHalf<TcpStream>>,
    GenericEncoder<monoio_rustls::ClientTlsStreamWriteHalf<TcpStream>>,
    ResponseDecoder<monoio_rustls::ClientTlsStreamReadHalf<TcpStream>>,
);

impl Service<TcpTransferParams> for TcpTransferService {
    type Response = ();

    type Error = GError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>>
    where
        Self: 'cx;

    fn call(&mut self, req: TcpTransferParams) -> Self::Future<'_> {
        async {
            info!("transfer data");
            let mut local_io = req.0;
            let mut remote_io = req.1;
            let (mut local_read, mut local_write) = local_io.split();
            let (mut remote_read, mut remote_write) = remote_io.split();
            let _ = monoio::join!(
                copy_data(&mut local_read, &mut remote_write),
                copy_data(&mut remote_read, &mut local_write)
            );
            Ok(())
        }
    }
}

impl Service<HttpTransferParams> for HttpTransferService {
    type Response = ();

    type Error = GError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>>
    where
        Self: 'cx;

    fn call(&mut self, req: HttpTransferParams) -> Self::Future<'_> {
        async {
            info!("transfer data");
            let (mut local_read, mut local_write) = (req.1, req.0);
            let (mut remote_read, mut remote_write) = (req.3, req.2);
            let _ = monoio::join!(
                copy_stream_sink(&mut local_read, &mut remote_write),
                copy_stream_sink(&mut remote_read, &mut local_write)
            );
            Ok(())
        }
    }
}

impl Service<HttpsTransferParams> for HttpTransferService {
    type Response = ();

    type Error = GError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>>
    where
        Self: 'cx;

    fn call(&mut self, req: HttpsTransferParams) -> Self::Future<'_> {
        async {
            info!("transfer data");
            let (mut local_read, mut local_write) = (req.1, req.0);
            let (mut remote_read, mut remote_write) = (req.3, req.2);
            let _ = monoio::join!(
                copy_stream_sink(&mut local_read, &mut remote_write),
                copy_stream_sink(&mut remote_read, &mut local_write)
            );
            Ok(())
        }
    }
}
