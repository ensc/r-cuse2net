#![allow(clippy::redundant_field_names)]
#![allow(dead_code)]

#[macro_use]
extern crate tracing;

mod error;
pub mod virtdev;
pub mod realdev;
pub mod proto;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
