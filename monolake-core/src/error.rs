pub type AnyError = anyhow::Error;

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
