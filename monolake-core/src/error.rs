/// A type alias for `anyhow::Error`, representing any error type.
///
/// This type is used throughout the crate to represent errors that can be of any type,
/// leveraging the flexibility of the `anyhow` crate for error handling.
pub type AnyError = anyhow::Error;

/// A type alias for `Result<T, E>` where `E` defaults to [`AnyError`](AnyError).
///
/// This type provides a convenient way to return results that can contain any error type,
/// defaulting to [`AnyError`] if no specific error type is specified.
///
/// # Type Parameters
///
/// * `T` - The type of the successful result.
/// * `E` - The error type, defaulting to [`AnyError`].
pub type AnyResult<T, E = AnyError> = std::result::Result<T, E>;
#[macro_export]
macro_rules! bail_into {
    ($msg:literal $(,)?) => {
        return Err(::anyhow::anyhow!($msg).into())
    };
    ($err:expr $(,)?) => {
        return Err(::anyhow::anyhow!($err).into())
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err(::anyhow::anyhow!($fmt, $($arg)*).into())
    };
}
