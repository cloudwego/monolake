use std::{collections::HashMap, fmt::Debug, fs::File, io::BufReader, path::Path, sync::RwLock};

use lazy_static::lazy_static;
use rustls::server::ResolvesServerCert;

use std::{io::Cursor, sync::Arc};

use rustls::sign::CertifiedKey;

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
        tracing::info!("certificate lookup succeed: {}", item.is_some());
        item.map(|item| item.to_owned())
    }
}

pub fn read_pem_chain_file(path: impl AsRef<Path> + Debug) -> anyhow::Result<Vec<Vec<u8>>> {
    let f = File::open(path)?;
    let mut reader = BufReader::new(f);
    let pems = rustls_pemfile::certs(&mut reader)?;
    Ok(pems)
}

pub fn read_pem_chain<R>(read: R) -> anyhow::Result<Vec<Vec<u8>>>
where
    R: std::io::Read,
{
    let mut reader = BufReader::new(read);
    let pems = rustls_pemfile::certs(&mut reader)?;
    tracing::info!("read pem chain length: {}", pems.len());
    Ok(pems)
}

// /// read only one pem
// pub fn read_pem_file(path: impl AsRef<Path> + Debug) -> Result<Vec<u8>> {
//     let f = File::open(path)?;
//     read_pem_certificate(f)
// }

// pub fn read_pem_certificate<R>(read: R) -> Result<Vec<u8>>
// where
//     R: std::io::Read,
// {
//     let mut reader = BufReader::new(read);
//     let mut pems = rustls_pemfile::certs(&mut reader)?;
//     match pems.pop() {
//         Some(pem) => Ok(pem),
//         None => bail!("pem file validate failed"),
//     }
// }

// pub fn read_private_key_file(
//     path: impl AsRef<Path> + Debug,
// ) -> Result<Vec<u8>> {
//     let f = File::open(path)?;
//     read_private_key(f)
// }

pub fn read_private_key<R>(read: R) -> anyhow::Result<Vec<Vec<u8>>>
where
    R: std::io::Read,
{
    let mut reader = BufReader::new(read);
    let keys = rustls_pemfile::pkcs8_private_keys(&mut reader)?;
    tracing::info!("read pem private key size: {}", keys.len());
    Ok(keys)
}

/// update certificate to global certificate map
// TODO: expose Result
pub fn update_certificate(server_name: String, chain_content: Vec<u8>, key_content: Vec<u8>) {
    tracing::info!("updating ssl certificate for {}", server_name);
    {
        let identity = native_tls::Identity::from_pkcs8(&chain_content, &key_content).unwrap();
        IDENTITY_MAP
            .write()
            .unwrap()
            .insert(server_name.clone(), identity);
    }
    {
        let (keys, chain) = (
            read_private_key(Cursor::new(key_content)),
            read_pem_chain(Cursor::new(chain_content)),
        );
        let (key, chain) = match (keys, chain) {
            (Ok(mut keys), Ok(c)) => match keys.pop() {
                Some(key) => (key, c),
                None => {
                    tracing::warn!("update ssl for {server_name} failed because key is empty.");
                    return;
                }
            },
            (kr, cr) => {
                tracing::warn!(
                    "update ssl for {server_name} failed. chain: {}, key: {}",
                    kr.is_ok(),
                    cr.is_ok()
                );
                return;
            }
        };

        let key = rustls::sign::any_supported_type(&rustls::PrivateKey(key));
        let certs = chain.into_iter().map(rustls::Certificate).collect();
        let certified_key = CertifiedKey::new(certs, key.unwrap());
        CERTIFICATE_MAP
            .write()
            .unwrap()
            .insert(server_name, Arc::new(certified_key));
    }
}
