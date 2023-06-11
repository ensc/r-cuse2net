#![allow(clippy::redundant_field_names)]

#[macro_use]
extern crate tracing;

mod error;
mod virtdev;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub use crate::virtdev::DeviceRegistry;
