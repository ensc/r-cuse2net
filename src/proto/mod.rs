mod endian;
mod errors;
mod io;
mod asrepr;
mod rawbuffer;
mod request;
mod response;

use std::time::Duration;

const TIMEOUT_READ: Duration = Duration::from_secs(3);

pub use endian::*;

pub use errors::Error;


pub type Result<T> = std::result::Result<T, Error>;

pub use asrepr::{ AsReprBytes, AsReprBytesMut };
pub use rawbuffer::RawBuffer;
pub use request::Request;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Sequence(u64);
