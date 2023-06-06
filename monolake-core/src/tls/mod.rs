use std::io::Cursor;

#[derive(Clone)]
pub enum TlsConfig<A = ::rustls::ServerConfig, B = ::native_tls::Identity> {
    Rustls(A),
    Native(B),
    None,
}

// TODO: move io to config builder.
impl TryFrom<&crate::config::TlsConfig> for TlsConfig {
    type Error = anyhow::Error;
    fn try_from(value: &crate::config::TlsConfig) -> anyhow::Result<TlsConfig> {
        let chain = std::fs::read(&value.chain)?;
        let key = std::fs::read(&value.key)?;
        match value.stack {
            crate::config::TlsStack::Rustls => {
                let chain = rustls_pemfile::certs(&mut Cursor::new(&chain))?
                    .into_iter()
                    .map(::rustls::Certificate)
                    .collect::<Vec<_>>();
                if chain.is_empty() {
                    anyhow::bail!("empty cert file");
                }
                let key = rustls_pemfile::pkcs8_private_keys(&mut Cursor::new(&key))?
                    .pop()
                    .map(::rustls::PrivateKey)
                    .ok_or_else(|| anyhow::anyhow!("empty key file"))?;
                let scfg = ::rustls::ServerConfig::builder()
                    .with_safe_defaults()
                    .with_no_client_auth()
                    .with_single_cert(chain, key)?;
                Ok(TlsConfig::Rustls(scfg))
            }
            crate::config::TlsStack::NativeTls => {
                let identity = native_tls::Identity::from_pkcs8(&chain, &key)?;
                Ok(TlsConfig::Native(identity))
            }
        }
    }
}
