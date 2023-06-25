#![allow(clippy::redundant_field_names)]

#[macro_use]
extern crate tracing;

pub mod ffi;
mod error;
mod bad;

pub use bad::BadIoctl;
