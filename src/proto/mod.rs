mod endian;
mod errors;
mod io;

use std::{os::fd::AsFd, mem::MaybeUninit, time::Duration};

pub use endian::*;

pub use errors::Error;

const TIMEOUT_READ: Duration = Duration::from_secs(3);

use crate::proto::io::recv_exact_timeout;
type Result<T> = std::result::Result<T, Error>;

pub unsafe trait AsReprBytes {
    fn uninit() -> MaybeUninit<Self>
    where
	Self: Sized
    {
	MaybeUninit::uninit()
    }

    fn as_repr_bytes(&self) -> &[u8] {
	unsafe {
	    core::slice::from_raw_parts(self as * const _ as * const u8,
					core::mem::size_of_val(self))
	}
    }
}

pub unsafe trait AsReprBytesMut: AsReprBytes {
    fn as_repr_bytes_mut(&mut self) -> &mut [u8] {
	unsafe {
	    core::slice::from_raw_parts_mut(self as * mut _ as * mut u8,
					    core::mem::size_of_val(self))
	}
    }

    fn update_repr(&mut self, buf: &[u8]) {
	debug_assert_eq!(self as * const _ as * const u8, buf.as_ptr());
	debug_assert_eq!(core::mem::size_of_val(self), buf.len());
    }
}

unsafe impl <T: AsReprBytes> AsReprBytes for MaybeUninit<T> {}
unsafe impl <T: AsReprBytesMut> AsReprBytesMut for MaybeUninit<T> {}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OpCode {
    Open = 1,
}

impl OpCode {
    pub fn as_u8(&self) -> u8 {
	*self as u8
    }

    pub fn try_from_u8(v: u8) -> Option<Self> {
	Some(match v {
	    1	=> Self::Open,
	    _	=> return None,
	})
    }
}

pub enum Op {
    Open(Open, u64),
}

impl Op {
    const MAX_SZ: usize = 0x1_0000;

    pub fn recv<R: AsFd + std::io::Read>(r: R) -> Result<Self> {
	let mut hdr = Header::uninit();

	let hdr = recv_exact_timeout(&r, &mut hdr, None, Some(TIMEOUT_READ))?;
	let len = hdr.len();

	if len > Self::MAX_SZ {
	    return Err(Error::PayloadTooLarge(len));
	}

	let op = hdr.op();
	let op = OpCode::try_from_u8(op).
	    ok_or(Error::BadOp(op))?;

	fn recv_to<R, B>(fd: R, mut buf: MaybeUninit<B>) -> std::io::Result<B>
	where
	    R: AsFd,
	    B: AsReprBytesMut + Sized,
	{
	    recv_exact_timeout(fd, &mut buf, Some(TIMEOUT_READ), Some(TIMEOUT_READ))?;

	    Ok(unsafe { buf.assume_init() })
	}

	let res = match op {
	    OpCode::Open	=>  Self::Open(recv_to(r, Open::uninit())?, hdr.seq()),
	};

	todo!()
    }
}

pub struct RawBuffer {
    data:	Vec<u8>,
}

impl RawBuffer {
    pub fn new(sz: usize) -> MaybeUninit<Self> {
	MaybeUninit::new(Self {
	    data:	Vec::with_capacity(sz),
	})
    }

    pub fn into_inner(self) -> Vec<u8> {
	self.data
    }
}

unsafe impl AsReprBytes for RawBuffer {
    fn as_repr_bytes(&self) -> &[u8] {
	&self.data
    }
}

unsafe impl AsReprBytesMut for RawBuffer {
    fn as_repr_bytes_mut(&mut self) -> &mut [u8] {
	self.data.as_mut()
    }

    fn update_repr(&mut self, buf: &[u8]) {
	debug_assert_eq!(self.data.as_ptr(), buf.as_ptr());
	debug_assert!(self.data.capacity() >= buf.len());

	unsafe {
	    self.data.set_len(buf.len());
	}
    }

}

#[repr(C)]
#[derive(Default)]
pub struct Header {
    op:		be8,
    _pad:	[u8;3],
    len:	be32,
    seq:	be64,
}

impl Header{
    pub fn op(&self) -> u8 {
	self.op.as_native()
    }

    pub fn len(&self) -> usize {
	self.len.as_native() as usize
    }

    pub fn seq(&self) -> u64 {
	self.seq.as_native()
    }
}

unsafe impl AsReprBytes for Header {}
unsafe impl AsReprBytesMut for Header {}

#[repr(C)]
#[derive(Default)]
pub struct Open {
    flags:	be32,
    _pad:	[u8;4]
}

unsafe impl AsReprBytes for Open {}
unsafe impl AsReprBytesMut for Open {}

mod compile_test {
    #![allow(dead_code)]
    use super::*;

    fn test_00() {
	use core::mem::size_of;

	const _: () = assert!(size_of::<Header>() % 8 == 0);
	const _: () = assert!(size_of::<Open>() % 8 == 0);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_00() {
    }
}
