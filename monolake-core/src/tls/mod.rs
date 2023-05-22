use std::{collections::HashMap, fmt::Debug, fs::File, io::BufReader, path::Path, sync::RwLock};

use anyhow::bail;
use lazy_static::lazy_static;
use rustls::server::ResolvesServerCert;

use std::{io::Cursor, sync::Arc};

use rustls::sign::CertifiedKey;

use crate::service::ServiceError;

lazy_static! {
    /// ssl
    pub static ref CERTIFICATE_MAP: Arc<RwLock<HashMap<String, Arc<rustls::sign::CertifiedKey>>>> = Arc::new(RwLock::new(HashMap::new()));
    pub static ref IDENTITY_MAP: Arc<RwLock<HashMap<String, native_tls::Identity>>> = Arc::new(RwLock::new(HashMap::new()));
}

pub struct CertificateResolver {
    server_name: String,
}

impl CertificateResolver {
    pub fn new(server_name: String) -> Self {
        CertificateResolver { server_name }
    }
}

impl ResolvesServerCert for CertificateResolver {
    fn resolve(
        &self,
        _client_hello: rustls::server::ClientHello,
    ) -> Option<std::sync::Arc<rustls::sign::CertifiedKey>> {
        let map = CERTIFICATE_MAP.read().unwrap();
        let server_name = self.server_name.as_str();
        let item = map.get(server_name);
        log::info!("certificate lookup succeed: {}", item.is_some());
        item.map(|item| item.to_owned())
    }
}

pub fn read_pem_chain_file(
    path: impl AsRef<Path> + Debug,
) -> Result<Vec<Vec<u8>>, ServiceError> {
    let f = File::open(path)?;
    let mut reader = BufReader::new(f);
    let pems = rustls_pemfile::certs(&mut reader)?;
    Ok(pems)
}

pub fn read_pem_chain<R>(read: R) -> Result<Vec<Vec<u8>>, ServiceError>
where
    R: std::io::Read,
{
    let mut reader = BufReader::new(read);
    let pems = rustls_pemfile::certs(&mut reader)?;
    log::info!("read pem chain length: {}", pems.len());
    Ok(pems)
}

/// read only one pem
pub fn read_pem_file(path: impl AsRef<Path> + Debug) -> Result<Vec<u8>, ServiceError> {
    let f = File::open(path)?;
    read_pem_certificate(f)
}

pub fn read_pem_certificate<R>(read: R) -> Result<Vec<u8>, ServiceError>
where
    R: std::io::Read,
{
    let mut reader = BufReader::new(read);
    let mut pems = rustls_pemfile::certs(&mut reader)?;
    match pems.pop() {
        Some(pem) => Ok(pem),
        None => bail!("pem file validate failed"),
    }
}

pub fn read_private_key_file(
    path: impl AsRef<Path> + Debug,
) -> Result<Vec<u8>, ServiceError> {
    let f = File::open(path)?;
    read_private_key(f)
}

pub fn read_private_key<R>(read: R) -> Result<Vec<u8>, ServiceError>
where
    R: std::io::Read,
{
    let mut reader = BufReader::new(read);
    let mut pems = rustls_pemfile::pkcs8_private_keys(&mut reader)?;
    if pems.is_empty() {
        bail!("no private key read");
    }
    match pems.pop() {
        Some(pem) => Ok(pem),
        None => bail!("private key validate failed"),
    }
}

/// update certificate to global certificate map
pub fn update_certificate(server_name: String, chain: Vec<u8>, key: Vec<u8>) {
    log::info!("updating ssl certificate for {}", server_name);
    {
        let identity = native_tls::Identity::from_pkcs8(&chain, &key).unwrap();
        IDENTITY_MAP
            .write()
            .unwrap()
            .insert(server_name.clone(), identity);
    }
    {
        let (key, chain) = (
            read_private_key(Cursor::new(key)),
            read_pem_chain(Cursor::new(chain)),
        );
        if key.is_err() || chain.is_err() {
            log::warn!(
                "update ssl for {} failed. chain: {}, key: {}",
                server_name,
                chain.is_ok(),
                key.is_ok()
            );
            return;
        }

        let (key, chain) = (key.unwrap(), chain.unwrap());

        let key = rustls::sign::any_supported_type(&rustls::PrivateKey(key));
        let mut certs = vec![];
        for cert in chain.into_iter() {
            let cert = rustls::Certificate(cert);
            certs.push(cert);
        }
        let certified_key = CertifiedKey::new(certs, key.unwrap());
        CERTIFICATE_MAP
            .write()
            .unwrap()
            .insert(server_name, Arc::new(certified_key));
    }
}
