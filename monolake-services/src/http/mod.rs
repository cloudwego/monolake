pub use detect::HttpVersionDetect;
use http::HeaderValue;

pub use self::core::{HttpCoreService, Keepalive};
pub mod handlers;

pub use h2t::{H2THandlerFactory, H2THandler, H2TConfig};

mod core;
mod detect;
mod h2t;
mod util;

pub(crate) const CLOSE: &str = "close";
pub(crate) const KEEPALIVE: &str = "Keep-Alive";
#[allow(clippy::declare_interior_mutable_const)]
pub(crate) const CLOSE_VALUE: HeaderValue = HeaderValue::from_static(CLOSE);
#[allow(clippy::declare_interior_mutable_const)]
pub(crate) const KEEPALIVE_VALUE: HeaderValue = HeaderValue::from_static(KEEPALIVE);
pub(crate) use util::generate_response;
