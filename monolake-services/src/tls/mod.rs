//! Unified TLS service module for supporting multiple TLS implementations.
//!
//! This module provides a unified interface for TLS services, supporting both Rustls
//! and Native TLS implementations, as well as a non-TLS option. It is designed to work
//! seamlessly with the service_async framework and provides flexibility in TLS configuration.
//!
//! # Key Components
//!
//! - [`UnifiedTlsService`]: The main service component that encapsulates different TLS
//!   implementations.
//! - [`UnifiedTlsFactory`]: Factory for creating `UnifiedTlsService` instances.
//! - [`TlsConfig`]: Configuration enum for specifying TLS settings.
//!
//! # Features
//!
//! - Support for both Rustls and Native TLS implementations
//! - Option for non-TLS (passthrough) connections
//! - Integration with `service_async` for easy composition in service stacks
//! - Unified error handling across different TLS implementations
//! - ALPN support for protocol negotiation (e.g., HTTP/2)
//!
//! # Usage
//!
//! This service is typically used as part of a larger service stack. Here's a basic example:
//!
//! ```ignore
//! use service_async::{layer::FactoryLayer, stack::FactoryStack};
//!
//! let tls_config = TlsConfig::Rustls(rustls_config);
//! let stack = FactoryStack::new(config)
//!     .push(UnifiedTlsFactory::layer())
//!     // ... other layers ...
//!     ;
//!
//! let service = stack.make_async().await.unwrap();
//! // Use the service to handle incoming connections
//! ```
//!
//! # Performance Considerations
//!
//! - The unified interface adds minimal overhead to the underlying TLS implementations
//! - Choice between Rustls and Native TLS allows for optimizing based on specific requirements
use std::io::Cursor;

use monolake_core::AnyError;
use native_tls::Identity;
use service_async::{
    AsyncMakeService, MakeService, Param, Service,
    layer::{FactoryLayer, layer_fn},
};

pub use self::{nativetls::NativeTlsService, rustls::RustlsService};
use self::{nativetls::NativeTlsServiceFactory, rustls::RustlsServiceFactory};
use crate::tcp::Accept;

mod nativetls;
mod rustls;

pub const APLN_PROTOCOLS: [&[u8]; 2] = [b"h2", b"http/1.1"];

/// Unified TLS service supporting multiple TLS implementations.
///
/// This enum encapsulates different TLS service implementations, allowing for
/// a unified interface while supporting various TLS backends.
pub enum UnifiedTlsService<T> {
    /// Rustls-based TLS service.
    Rustls(RustlsService<T>),
    /// Native TLS-based service.
    Native(NativeTlsService<T>),
    None(T),
}

impl<T> UnifiedTlsService<T> {
    fn as_rustls(this: Option<&Self>) -> Option<&RustlsService<T>> {
        this.and_then(|s| match s {
            UnifiedTlsService::Rustls(inner) => Some(inner),
            _ => None,
        })
    }

    fn as_native(this: Option<&Self>) -> Option<&NativeTlsService<T>> {
        this.and_then(|s| match s {
            UnifiedTlsService::Native(inner) => Some(inner),
            _ => None,
        })
    }

    fn as_none(this: Option<&Self>) -> Option<&T> {
        this.and_then(|s| match s {
            UnifiedTlsService::None(inner) => Some(inner),
            _ => None,
        })
    }
}

pub enum UnifiedResponse<A, B, C> {
    Rustls(A),
    Native(B),
    None(C),
}

impl<A> UnifiedResponse<A, A, A> {
    pub fn into_inner(self) -> A {
        match self {
            UnifiedResponse::Rustls(inner) => inner,
            UnifiedResponse::Native(inner) => inner,
            UnifiedResponse::None(inner) => inner,
        }
    }
}

impl<T, S, CX> Service<Accept<S, CX>> for UnifiedTlsService<T>
where
    RustlsService<T>: Service<Accept<S, CX>>,
    NativeTlsService<T>: Service<Accept<S, CX>>,
    <RustlsService<T> as Service<Accept<S, CX>>>::Error: Into<AnyError>,
    <NativeTlsService<T> as Service<Accept<S, CX>>>::Error: Into<AnyError>,
    T: Service<Accept<S, CX>>,
    T::Error: Into<AnyError>,
{
    type Response = UnifiedResponse<
        <RustlsService<T> as Service<Accept<S, CX>>>::Response,
        <NativeTlsService<T> as Service<Accept<S, CX>>>::Response,
        T::Response,
    >;

    type Error = AnyError;

    async fn call(&self, req: Accept<S, CX>) -> Result<Self::Response, Self::Error> {
        match self {
            UnifiedTlsService::Rustls(inner) => inner
                .call(req)
                .await
                .map(UnifiedResponse::Rustls)
                .map_err(Into::into),
            UnifiedTlsService::Native(inner) => inner
                .call(req)
                .await
                .map(UnifiedResponse::Native)
                .map_err(Into::into),
            UnifiedTlsService::None(inner) => inner
                .call(req)
                .await
                .map(UnifiedResponse::None)
                .map_err(Into::into),
        }
    }
}

/// Factory for creating UnifiedTlsService instances.
///
/// This enum provides a unified factory interface for creating TLS services
/// with different backends.
pub enum UnifiedTlsFactory<F> {
    Rustls(RustlsServiceFactory<F>),
    Native(NativeTlsServiceFactory<F>),
    None(F),
}

impl<F> MakeService for UnifiedTlsFactory<F>
where
    RustlsServiceFactory<F>: MakeService<Service = RustlsService<F::Service>>,
    NativeTlsServiceFactory<F>:
        MakeService<Service = NativeTlsService<F::Service>, Error = AnyError>,
    <RustlsServiceFactory<F> as MakeService>::Error: Into<AnyError>,
    <NativeTlsServiceFactory<F> as MakeService>::Error: Into<AnyError>,
    F: MakeService,
    F::Error: Into<AnyError>,
{
    type Service = UnifiedTlsService<F::Service>;
    type Error = AnyError;

    fn make_via_ref(&self, old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        match self {
            UnifiedTlsFactory::Rustls(inner) => inner
                .make_via_ref(UnifiedTlsService::as_rustls(old))
                .map(UnifiedTlsService::Rustls)
                .map_err(Into::into),
            UnifiedTlsFactory::Native(inner) => inner
                .make_via_ref(UnifiedTlsService::as_native(old))
                .map(UnifiedTlsService::Native)
                .map_err(Into::into),
            UnifiedTlsFactory::None(inner) => inner
                .make_via_ref(UnifiedTlsService::as_none(old))
                .map(UnifiedTlsService::None)
                .map_err(Into::into),
        }
    }
}

impl<F> AsyncMakeService for UnifiedTlsFactory<F>
where
    RustlsServiceFactory<F>: AsyncMakeService<Service = RustlsService<F::Service>>,
    NativeTlsServiceFactory<F>:
        AsyncMakeService<Service = NativeTlsService<F::Service>, Error = AnyError>,
    <RustlsServiceFactory<F> as AsyncMakeService>::Error: Into<AnyError>,
    <NativeTlsServiceFactory<F> as AsyncMakeService>::Error: Into<AnyError>,
    F: AsyncMakeService,
    F::Error: Into<AnyError>,
{
    type Service = UnifiedTlsService<F::Service>;
    type Error = AnyError;

    async fn make_via_ref(
        &self,
        old: Option<&Self::Service>,
    ) -> Result<Self::Service, Self::Error> {
        match self {
            UnifiedTlsFactory::Rustls(inner) => inner
                .make_via_ref(UnifiedTlsService::as_rustls(old))
                .await
                .map(UnifiedTlsService::Rustls)
                .map_err(Into::into),
            UnifiedTlsFactory::Native(inner) => inner
                .make_via_ref(UnifiedTlsService::as_native(old))
                .await
                .map(UnifiedTlsService::Native)
                .map_err(Into::into),
            UnifiedTlsFactory::None(inner) => inner
                .make_via_ref(UnifiedTlsService::as_none(old))
                .await
                .map(UnifiedTlsService::None)
                .map_err(Into::into),
        }
    }
}

/// Configuration enum for specifying TLS settings.
///
/// This enum allows for flexible configuration of TLS services,
/// supporting both Rustls and Native TLS implementations, as well as a non-TLS option.
#[derive(Clone)]
pub enum TlsConfig<A = ::rustls::ServerConfig, B = ::native_tls::Identity> {
    Rustls(A),
    Native(B),
    None,
}

impl<A, B> std::fmt::Debug for TlsConfig<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rustls(_) => write!(f, "Rustls"),
            Self::Native(_) => write!(f, "NativeTls"),
            Self::None => write!(f, "None"),
        }
    }
}

impl<F> UnifiedTlsFactory<F> {
    pub fn layer<C, A, B>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<TlsConfig<A, B>>,
        A: Param<::rustls::ServerConfig>,
        B: Param<Identity>,
    {
        layer_fn(|c: &C, inner| match &c.param() {
            TlsConfig::Rustls(i) => Self::Rustls(RustlsServiceFactory::layer().layer(i, inner)),
            TlsConfig::Native(i) => Self::Native(NativeTlsServiceFactory::layer().layer(i, inner)),
            TlsConfig::None => Self::None(inner),
        })
    }
}

impl TryFrom<TlsConfig<(Vec<u8>, Vec<u8>), (Vec<u8>, Vec<u8>)>> for TlsConfig {
    type Error = anyhow::Error;

    fn try_from(
        value: TlsConfig<(Vec<u8>, Vec<u8>), (Vec<u8>, Vec<u8>)>,
    ) -> Result<Self, Self::Error> {
        match value {
            TlsConfig::Rustls((chain, key)) => {
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
                let mut scfg = ::rustls::ServerConfig::builder()
                    .with_safe_defaults()
                    .with_no_client_auth()
                    .with_single_cert(chain, key)?;
                scfg.alpn_protocols = APLN_PROTOCOLS.map(|proto| proto.to_vec()).to_vec();
                Ok(TlsConfig::Rustls(scfg))
            }
            TlsConfig::Native((chain, key)) => Ok(TlsConfig::Native(
                native_tls::Identity::from_pkcs8(&chain, &key)?,
            )),
            TlsConfig::None => Ok(TlsConfig::None),
        }
    }
}
