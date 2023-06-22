use http::{HeaderValue, Response, StatusCode};
use monoio_http::h1::payload::Payload;

pub use self::core::HttpCoreService;
pub mod handlers;

mod core;
mod util;

pub const CLOSE: &str = "close";
pub const KEEPALIVE: &str = "Keep-Alive";
#[allow(clippy::declare_interior_mutable_const)]
pub const CLOSE_VALUE: HeaderValue = HeaderValue::from_static(CLOSE);
#[allow(clippy::declare_interior_mutable_const)]
pub const KEEPALIVE_VALUE: HeaderValue = HeaderValue::from_static(KEEPALIVE);

fn generate_response(status_code: StatusCode, close: bool) -> Response<Payload> {
    let mut resp = Response::builder();
    resp = resp.status(status_code);
    let headers = resp.headers_mut().unwrap();
    if close {
        headers.insert(http::header::CONNECTION, CLOSE_VALUE);
    }
    headers.insert(http::header::CONTENT_LENGTH, HeaderValue::from_static("0"));
    resp.body(Payload::None).unwrap()
}
