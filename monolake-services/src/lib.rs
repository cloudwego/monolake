#![feature(let_chains)]
#![feature(impl_trait_in_assoc_type)]

pub mod common;
pub mod http;
pub mod tcp;
pub mod thrift;

#[cfg(feature = "proxy-protocol")]
pub mod proxy_protocol;

#[cfg(feature = "tls")]
pub mod tls;

#[cfg(feature = "hyper")]
pub mod hyper;
