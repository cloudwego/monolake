use http::HeaderValue;

pub use self::core::{HttpCoreService, HttpServerTimeout};
pub mod handlers;

pub mod core;
pub mod detect;
mod util;

pub(crate) const CLOSE: &str = "close";
pub(crate) const KEEPALIVE: &str = "Keep-Alive";
#[allow(clippy::declare_interior_mutable_const)]
pub(crate) const CLOSE_VALUE: HeaderValue = HeaderValue::from_static(CLOSE);
#[allow(clippy::declare_interior_mutable_const)]
pub(crate) const KEEPALIVE_VALUE: HeaderValue = HeaderValue::from_static(KEEPALIVE);
pub(crate) use util::generate_response;
