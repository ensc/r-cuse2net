use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd};
use std::sync::atomic::AtomicU64;

use nix::sys::socket::MsgFlags;

use crate::proto::io::recv_exact_timeout;

use super::TIMEOUT_READ;
use super::{ Result, Error, AsReprBytesMut, AsReprBytes };
use super::endian::*;



#[derive(Debug)]
pub enum Response {
    Result(i32),
}

impl Response {
    pub fn recv<R: AsFd + std::io::Read>(r: R) -> Result<Self> {
	let mut hdr = RequestHeader::uninit();
    }
}


