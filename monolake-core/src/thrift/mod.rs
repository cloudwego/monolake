//! Thrift protocol handling for asynchronous services.
//!
//! This module provides traits and types for implementing Thrift handlers
//! that can be used with asynchronous services. It defines a common interface
//! for processing Thrift requests and generating responses, with support for
//! context-aware handling.
//!
//! # Key Components
//!
//! - [`ThriftHandler`]: A trait for implementing Thrift request handlers.
//! - [`ThriftRequest`]: A type alias for Thrift requests using TTHeader protocol.
//! - [`ThriftResponse`]: A type alias for Thrift responses using TTHeader protocol.
//! - [`ThriftBody`]: A type alias for the payload of Thrift requests and responses.

use std::future::Future;

use monoio_thrift::codec::ttheader::TTHeaderPayload;
use service_async::Service;

use crate::sealed::SealedT;

/// Type alias for the Thrift request/response body.
///
/// Currently uses `bytes::Bytes` for efficient memory management.
/// TODO: Support discontinuous memory in the future.
pub type ThriftBody = bytes::Bytes;

/// Type alias for a Thrift request using TTHeader protocol.
pub type ThriftRequest<T> = TTHeaderPayload<T>;

/// Type alias for a Thrift response using TTHeader protocol.
pub type ThriftResponse<T> = TTHeaderPayload<T>;

struct ThriftSeal;

/// A trait for Thrift request handlers.
///
/// This trait defines the interface for processing Thrift requests and generating responses.
/// It is designed to work with asynchronous services and supports context-aware handling.
///
/// # Type Parameters
///
/// - `CX`: The context type for additional request processing information.
///
/// # Associated Types
///
/// - `Error`: The error type that may occur during request handling.
///
/// # Examples
///
/// ```ignore
/// use your_crate::{ThriftHandler, ThriftRequest, ThriftResponse, ThriftBody};
///
/// struct MyThriftHandler;
///
/// impl ThriftHandler<()> for MyThriftHandler {
///     type Error = std::io::Error;
///
///     async fn handle(&self, request: ThriftRequest<ThriftBody>, ctx: ())
///         -> Result<ThriftResponse<ThriftBody>, Self::Error> {
///         // Process the Thrift request and generate a response
///         let response = ThriftResponse::new(/* ... */);
///         Ok(response)
///     }
/// }
/// ```
///
/// The `ThriftHandler` trait is automatically implemented for types that implement the `Service`
/// trait with request type `(ThriftRequest<ThriftBody>, CX)` and response type
/// `ThriftResponse<ThriftBody>`.

#[allow(private_bounds)]
pub trait ThriftHandler<CX>: SealedT<ThriftSeal, CX> {
    type Error;

    fn handle(
        &self,
        request: ThriftRequest<ThriftBody>,
        ctx: CX,
    ) -> impl Future<Output = Result<ThriftResponse<ThriftBody>, Self::Error>>;
}

impl<T, CX> SealedT<ThriftSeal, CX> for T where
    T: Service<(ThriftRequest<ThriftBody>, CX), Response = ThriftResponse<ThriftBody>>
{
}

impl<T, CX> ThriftHandler<CX> for T
where
    T: Service<(ThriftRequest<ThriftBody>, CX), Response = ThriftResponse<ThriftBody>>,
{
    type Error = T::Error;

    async fn handle(
        &self,
        req: ThriftRequest<ThriftBody>,
        ctx: CX,
    ) -> Result<ThriftResponse<ThriftBody>, Self::Error> {
        self.call((req, ctx)).await
    }
}
