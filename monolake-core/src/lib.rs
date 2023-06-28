#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]

#[macro_use]
mod error;
pub use error::{AnyError, AnyResult};

pub mod config;
pub mod context;
pub mod http;
pub mod listener;
pub mod server;
pub mod util;

pub(crate) mod sealed {
    pub trait Sealed {}
    pub trait SealedT<T> {}
}
