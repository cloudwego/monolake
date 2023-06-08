#![feature(impl_trait_in_assoc_type)]

#[macro_use]
mod error;
pub use error::{AnyError, AnyResult};

pub mod config;
pub mod http;
pub mod listener;
pub mod tls;
pub mod util;

use figlet_rs::FIGfont;

pub fn print_logo() {
    let standard_font = FIGfont::standard().unwrap();
    if let Some(figure) = standard_font.convert("Monolake") {
        println!("{}", figure);
    }
}

pub(crate) mod sealed {
    pub trait Sealed {}
}
