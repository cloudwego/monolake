#![feature(impl_trait_in_assoc_type)]

pub type AnyError = anyhow::Error;

pub mod common;
pub mod http;
pub mod tcp;
pub mod tls;
