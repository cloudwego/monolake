mod rustls;
pub use self::rustls::RustlsService;

use crate::common::Accept;
use crate::AnyError;
use ::rustls::ServerConfig;
use monolake_core::service::{
    layer::{layer_fn, FactoryLayer},
    MakeService, Param, Service,
};
use native_tls::Identity;
use std::future::Future;

mod nativetls;
pub use self::nativetls::NativeTlsService;
use self::{nativetls::NativeTlsServiceFactory, rustls::RustlsServiceFactory};

pub enum UnifiedTlsService<T> {
    Rustls(RustlsService<T>),
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

impl<T, S, A> Service<Accept<S, A>> for UnifiedTlsService<T>
where
    RustlsService<T>: Service<Accept<S, A>>,
    NativeTlsService<T>: Service<Accept<S, A>>,
    <RustlsService<T> as Service<Accept<S, A>>>::Error: Into<AnyError>,
    <NativeTlsService<T> as Service<Accept<S, A>>>::Error: Into<AnyError>,
    T: Service<Accept<S, A>>,
    T::Error: Into<AnyError>,
{
    type Response = UnifiedResponse<
        <RustlsService<T> as Service<Accept<S, A>>>::Response,
        <NativeTlsService<T> as Service<Accept<S, A>>>::Response,
        T::Response,
    >;

    type Error = AnyError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        S: 'cx,
        A: 'cx;

    fn call(&self, req: Accept<S, A>) -> Self::Future<'_> {
        async move {
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
}

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

#[derive(Clone)]
pub enum TlsConfig<A = ServerConfig, B = Identity> {
    Rustls(A),
    Native(B),
    None,
}

impl<F> UnifiedTlsFactory<F> {
    pub fn layer<C, A, B>() -> impl FactoryLayer<C, F, Factory = Self>
    where
        C: Param<TlsConfig<A, B>>,
        A: Param<ServerConfig>,
        B: Param<Identity>,
    {
        layer_fn::<C, _, _, _>(|c, inner| match &c.param() {
            TlsConfig::Rustls(i) => Self::Rustls(RustlsServiceFactory::layer().layer(i, inner)),
            TlsConfig::Native(i) => Self::Native(NativeTlsServiceFactory::layer().layer(i, inner)),
            TlsConfig::None => Self::None(inner),
        })
    }
}
