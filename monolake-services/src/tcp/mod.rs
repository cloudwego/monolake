//! Tcp specific Services(Under progress)
pub mod echo;
pub mod proxy;

pub type Accept<Stream, Ctx> = (Stream, Ctx);
