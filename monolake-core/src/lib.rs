#![feature(impl_trait_in_assoc_type)]

pub mod config;
pub mod http;
pub mod listener;
pub mod service;
pub mod tls;
pub mod util;

mod error;
pub use error::{Error, Result};

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
