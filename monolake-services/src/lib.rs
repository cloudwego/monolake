#![feature(impl_trait_in_assoc_type)]
#![feature(let_chains)]

pub mod common;
pub mod http;
pub mod tcp;
pub mod tls;

#[cfg(feature = "proxy-protocol")]
pub mod proxy_protocol;
