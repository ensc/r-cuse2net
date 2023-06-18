use std::sync::atomic::AtomicU64;
use std::io::IoSlice;
use std::os::fd::AsFd;

use super::{Sequence, AsReprBytes, TIMEOUT_READ, Error, Result, AsReprBytesMut};
use super::io::{send_vectored_all, recv_exact_timeout, recv_to};
use super::endian::*;

static OP_SEQ: AtomicU64 = AtomicU64::new(1);

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RequestCode {
    Open = 1,
    Release = 2,
}

impl RequestCode {
    pub fn as_u8(&self) -> u8 {
	*self as u8
    }

    pub fn try_from_u8(v: u8) -> Option<Self> {
	Some(match v {
	    1	=> Self::Open,
	    2	=> Self::Release,
	    _	=> return None,
	})
    }
}

#[derive(Debug)]
pub enum Request {
    Open(Open, Sequence),
    Release(Sequence),
}

impl Request {
    const MAX_SZ: usize = 0x1_0000;

    #[instrument(level="trace", skip(r), ret)]
    pub fn recv<R: AsFd + std::io::Read>(r: R) -> Result<Self> {
	let mut hdr = Header::uninit();

	let hdr = recv_exact_timeout(&r, &mut hdr, None, Some(TIMEOUT_READ))?;
	let len = hdr.len();

	if len > Self::MAX_SZ {
	    return Err(Error::PayloadTooLarge(len));
	}

	let op = hdr.op();
	let op = RequestCode::try_from_u8(op)
	    .ok_or(Error::BadOp(op))?;
	let seq = hdr.seq()?;

	Ok(match op {
	    RequestCode::Open		=> Self::Open(recv_to(r, Open::uninit())?, seq),
	    RequestCode::Release	=> Self::Release(seq),
	})
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Header {
    op:		be8,
    _pad:	[u8;3],
    len:	be32,
    seq:	be64,
}

impl Header{
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

    pub fn seq(&self) -> Result<Sequence> {
	match self.seq.as_native() {
	    0	=> Err(Error::BadSequence),
	    v	=> Ok(Sequence(v))
	}
    }
}

unsafe impl AsReprBytes for Header {}
unsafe impl AsReprBytesMut for Header {}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Open {
    pub flags:	be32,
    _pad:	[u8;4]
}

unsafe impl AsReprBytes for Open {}
unsafe impl AsReprBytesMut for Open {}

impl Request {
    #[instrument(level="trace", skip(w), ret)]
    pub fn send_open<W: AsFd + std::io::Write>(w: W, flags: u32) -> Result<Sequence> {
	let info = Open {
	    flags: flags.into(),
	    ..Default::default()
	};

	let hdr = Header::new(RequestCode::Open, &info);
	let seq = hdr.seq()?;

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(info.as_repr_bytes()) ])?;

	Ok(seq)
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Release {
}

unsafe impl AsReprBytes for Release {}
unsafe impl AsReprBytesMut for Release {}

impl Request {
    #[instrument(level="trace", skip(w), ret)]
    pub fn send_release<W: AsFd + std::io::Write>(w: W) -> Result<Sequence> {
	let info = Release {
	};

	let hdr = Header::new(RequestCode::Open, &info);
	let seq = hdr.seq()?;

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(info.as_repr_bytes()) ])?;

	Ok(seq)
    }
}

mod compile_test {
    #![allow(dead_code)]
    use super::*;

    fn test_00() {
	use core::mem::size_of;

	const _: () = assert!(size_of::<Header>() == 16);
	const _: () = assert!(size_of::<Open>() == 8);
    }
}