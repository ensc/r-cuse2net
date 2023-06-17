use std::{sync::atomic::AtomicU64, os::fd::AsFd, io::IoSlice};

use super::{Sequence, AsReprBytes, TIMEOUT_READ, Error, Result, AsReprBytesMut};
use super::io::{send_all, recv_exact_timeout, recv_to};
use super::endian::*;

static OP_SEQ: AtomicU64 = AtomicU64::new(1);

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RequestCode {
    Open = 1,
}

impl RequestCode {
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

#[derive(Debug)]
pub enum Request {
    Open(Open, Sequence),
}

impl Request {
    const MAX_SZ: usize = 0x1_0000;

    pub fn recv<R: AsFd + std::io::Read>(r: R) -> Result<Self> {
	let mut hdr = RequestHeader::uninit();

	let hdr = recv_exact_timeout(&r, &mut hdr, None, Some(TIMEOUT_READ))?;
	let len = hdr.len();

	if len > Self::MAX_SZ {
	    return Err(Error::PayloadTooLarge(len));
	}

	let op = hdr.op();
	let op = RequestCode::try_from_u8(op).
	    ok_or(Error::BadOp(op))?;


	Ok(match op {
	    RequestCode::Open	=>  Self::Open(recv_to(r, Open::uninit())?, hdr.seq()),
	})
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct RequestHeader {
    op:		be8,
    _pad:	[u8;3],
    len:	be32,
    seq:	be64,
}

impl RequestHeader{
    pub fn new<T: Sized>(op: RequestCode, payload: &T) -> Self {
	let len = core::mem::size_of_val(payload) as u32;

	Self {
	    op:		op.as_u8().into(),
	    seq:	OP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed).into(),
	    len:	len.into(),
	    .. Default::default()
	}
    }

    pub fn op(&self) -> u8 {
	self.op.as_native()
    }

    pub fn len(&self) -> usize {
	self.len.as_native() as usize
    }

    pub fn seq(&self) -> Sequence {
	Sequence(self.seq.as_native())
    }
}

unsafe impl AsReprBytes for RequestHeader {}
unsafe impl AsReprBytesMut for RequestHeader {}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Open {
    flags:	be32,
    _pad:	[u8;4]
}

unsafe impl AsReprBytes for Open {}
unsafe impl AsReprBytesMut for Open {}

impl Request {
    pub fn send_open<W: AsFd + std::io::Write>(w: W, flags: u32) -> Result<Sequence> {
	let info = Open {
	    flags: flags.into(),
	    ..Default::default()
	};

	let hdr = RequestHeader::new(RequestCode::Open, &info);
	let seq = hdr.seq();

	send_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
		       IoSlice::new(info.as_repr_bytes()) ])?;

	Ok(seq)
    }
}

mod compile_test {
    #![allow(dead_code)]
    use super::*;

    fn test_00() {
	use core::mem::size_of;

	const _: () = assert!(size_of::<RequestHeader>() % 8 == 0);
	const _: () = assert!(size_of::<Open>() % 8 == 0);
    }
}
