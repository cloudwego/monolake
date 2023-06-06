use http::{HeaderMap, HeaderValue, Response, StatusCode};
use monoio_http::h1::payload::Payload;

pub use self::core::HttpCoreService;

mod core;
pub mod handlers;
mod util;

const CONNECTION: &str = "Connection";
const CONN_CLOSE: &[u8] = b"close";
const CONN_KEEP_ALIVE: &[u8] = b"keep-alive";

pub const COUNTER_HEADER_NAME: &str = "counter";
pub const TIMER_HEADER_NAME: &str = "timer";

fn generate_response(status_code: StatusCode) -> Response<Payload> {
    let mut resp = Response::builder();
    resp = resp.status(status_code);
    let headers = resp.headers_mut().unwrap();
    headers.insert(CONNECTION, unsafe {
        HeaderValue::from_maybe_shared_unchecked(CONN_CLOSE)
    });
    headers.insert("Content-Length", HeaderValue::from_static("0"));
    resp.body(Payload::None).unwrap()
}

pub fn is_conn_reuse(headers: &HeaderMap<HeaderValue>, version: http::Version) -> bool {
    match headers.get(http::header::CONNECTION) {
        Some(v) => !v.as_bytes().eq_ignore_ascii_case(CONN_CLOSE),
        None => version != http::Version::HTTP_10,
    }
}
