#[macro_use]
mod error;
pub use error::{AnyError, AnyResult};

pub mod config;
pub mod context;
pub mod http;
pub mod listener;
pub mod server;
pub mod thrift;
pub mod util;

pub(crate) mod sealed {
    #[allow(dead_code)]
    pub trait Sealed {}
    #[allow(dead_code)]
    pub trait SealedT<T> {}
}
