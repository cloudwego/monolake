mod delay;
mod timeout;

pub use delay::DelayService;
pub use timeout::TimeoutService;

pub type Accept<Stream, SocketAddr> = (Stream, SocketAddr);
