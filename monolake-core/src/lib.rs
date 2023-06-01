#![feature(impl_trait_in_assoc_type)]

#[macro_use]
mod error;
pub use error::{AnyError, AnyResult};

pub mod config;
pub mod http;
pub mod listener;
pub mod service;
pub mod tls;
pub mod util;

use figlet_rs::FIGfont;

pub trait Builder<Config> {
    fn build_with_config(config: Config) -> Self;
}

pub fn print_logo() {
    let standard_font = FIGfont::standard().unwrap();
    if let Some(figure) = standard_font.convert("Monolake") {
        println!("{}", figure);
    }
}
