mod endian;
mod errors;
mod io;
mod asrepr;
mod rawbuffer;
pub mod request;
pub mod response;
pub mod ioctl;

use std::{time::Duration, os::fd::AsFd};

const TIMEOUT_READ: Duration = Duration::from_secs(3);
pub const MAX_MSG_SIZE: usize = 128 * 1024;

pub use endian::*;

pub use errors::Error;


pub type Result<T> = std::result::Result<T, Error>;

pub use asrepr::{ AsReprBytes, AsReprBytesMut };
pub use rawbuffer::RawBuffer;
pub use request::Request;
pub use response::Response;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sequence(u64);

impl Sequence {
    pub fn send_err<W: AsFd + std::io::Write>(self, w: W, err: nix::Error) -> Result<()> {
	Response::send_err(w, self, err)
    }

    pub fn send_ok<W: AsFd + std::io::Write>(self, w: W) -> Result<()> {
	Response::send_ok(w, self)
    }
}
