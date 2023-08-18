#![feature(impl_trait_in_assoc_type)]
#![feature(let_chains)]

pub mod common;
pub mod http;
pub mod tcp;

#[cfg(feature = "proxy-protocol")]
pub mod proxy_protocol;

#[cfg(feature = "tls")]
pub mod tls;
