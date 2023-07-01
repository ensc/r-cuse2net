use std::mem::MaybeUninit;
use std::sync::atomic::AtomicU64;
use std::io::IoSlice;
use std::os::fd::AsFd;

use ensc_cuse_ffi::ffi as cuse_ffi;
use ensc_cuse_ffi::{WriteParams, ReadParams, PollParams};
use ensc_ioctl_ffi::ffi::ioctl;

use super::ioctl::Arg;
use super::{Sequence, AsReprBytes, TIMEOUT_READ, Error, Result, AsReprBytesMut};
use super::io::{send_vectored_all, recv_exact_timeout, recv_to};
use super::endian::*;

static OP_SEQ: AtomicU64 = AtomicU64::new(1);

#[path = "request_flags.rs"]
mod flags;

use flags::*;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RequestCode {
    Open = 1,
    Release = 2,
    Write = 3,
    Read = 4,
    Ioctl = 5,
    Poll = 6,
}

impl RequestCode {
    pub fn as_u8(&self) -> u8 {
	*self as u8
    }

    pub fn try_from_u8(v: u8) -> Option<Self> {
	Some(match v {
	    1	=> Self::Open,
	    2	=> Self::Release,
	    3	=> Self::Write,
	    4	=> Self::Read,
	    5	=> Self::Ioctl,
	    6	=> Self::Poll,
	    _	=> return None,
	})
    }
}

#[derive(Debug)]
pub enum Request<'a> {
    Open(Open, Sequence),
    Release(Sequence),
    Write(Sequence, Write, &'a[u8]),
    Read(Sequence, Read),
    Ioctl(Sequence, Ioctl, Arg),
    Poll(Sequence, Poll),
}

fn sub_slice(buf: &mut [MaybeUninit<u8>], sz: usize) -> MaybeUninit<&mut [u8]> {
    let buf = &mut buf[..sz];

    unsafe {
	core::mem::transmute(buf)
    }
}

impl <'a> Request<'a> {
    const MAX_SZ: usize = 0x1_0000;

    //#[instrument(level="trace", skip(r, tmp_buf), ret)]
    pub fn recv<R: AsFd + std::io::Read>(r: R, tmp_buf: &'a mut [MaybeUninit<u8>]) -> Result<Self> {
	let mut hdr = Header::uninit();

	let hdr = recv_exact_timeout(&r, &mut hdr, &mut None, None, Some(TIMEOUT_READ))?;
	debug!("hdr={hdr:?}");
	let len = hdr.len();

	if len > Self::MAX_SZ {
	    return Err(Error::PayloadTooLarge(len));
	}

	let mut rx_len = Some(len);

	let op = hdr.op();
	let op = RequestCode::try_from_u8(op)
	    .ok_or(Error::BadOp(op))?;
	let seq = hdr.seq()?;

	let res = match op {
	    RequestCode::Open		=> Self::Open(recv_to(r, Open::uninit(), &mut rx_len)?, seq),
	    RequestCode::Release	=> Self::Release(seq),
	    RequestCode::Write		=> {
		let wrinfo = recv_to(&r, Write::uninit(), &mut rx_len)?;
		let rxbuf = sub_slice(tmp_buf, *rx_len.as_ref().unwrap());

		Self::Write(seq, wrinfo, recv_to(&r, rxbuf, &mut rx_len)?)
	    }
	    RequestCode::Read		=> {
		let rdinfo = recv_to(&r, Read::uninit(), &mut rx_len)?;
		Self::Read(seq, rdinfo)
	    }
	    RequestCode::Ioctl		=> {
		let ioinfo = recv_to(&r, Ioctl::uninit(), &mut rx_len)?;
		let rxbuf  = sub_slice(tmp_buf, *rx_len.as_ref().unwrap());
		let arg = recv_to(&r, rxbuf, &mut rx_len)?;
		let arg = Arg::from_raw(ioinfo.arg_type.into(), arg)?;

		Self::Ioctl(seq, ioinfo, arg)
	    }
	    RequestCode::Poll		=> {
		let pollinfo = recv_to(&r, Poll::uninit(), &mut rx_len)?;
		Self::Poll(seq, pollinfo)
	    }
	};

	match rx_len.unwrap() {
	    0		=>
		Ok(res),
	    l		=> {
		warn!("{l} octets not consumed for {op:?}");
		Err(super::Error::BadLength)
	    }
	}
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
	Self::with_payload(op, payload, &[])
    }

    pub fn with_payload<T: Sized>(op: RequestCode, payload: &T, data: &[u8]) -> Self {
	let len = (core::mem::size_of_val(payload) + data.len()) as u32;

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
    pub flags:	FhFlags,
    _pad:	[u8;4]
}

unsafe impl AsReprBytes for Open {}
unsafe impl AsReprBytesMut for Open {}

impl Request<'_> {
    //#[instrument(level="trace", skip(w), ret)]
    pub fn send_open<W: AsFd + std::io::Write>(w: W, flags: cuse_ffi::fh_flags) -> Result<Sequence> {
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

impl Request<'_> {
    //#[instrument(level="trace", skip(w), ret)]
    pub fn send_release<W: AsFd + std::io::Write>(w: W) -> Result<Sequence> {
	let info = Release {
	};

	let hdr = Header::new(RequestCode::Release, &info);
	let seq = hdr.seq()?;

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(info.as_repr_bytes()) ])?;

	Ok(seq)
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Write {
    pub offset:		be64,
    pub fh_flags:	FhFlags,
    _pad:		[be8;4],
}

unsafe impl AsReprBytes for Write {}
unsafe impl AsReprBytesMut for Write {}

impl Request<'_> {
    //#[instrument(level="trace", skip(w), ret)]
    pub fn send_write<W: AsFd + std::io::Write>(w: W, wrinfo: WriteParams, data: &[u8]) -> Result<Sequence> {
	let info = Write {
	    offset:	wrinfo.offset.into(),
	    fh_flags:	wrinfo.flags.into(),
	    _pad:	Default::default(),
	};

	let hdr = Header::with_payload(RequestCode::Write, &info, data);
	let seq = hdr.seq()?;

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(info.as_repr_bytes()),
				IoSlice::new(data) ])?;

	Ok(seq)
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Read {
    pub offset:		be64,
    pub size:		be32,
    pub fh_flags:	FhFlags,
}

unsafe impl AsReprBytes for Read {}
unsafe impl AsReprBytesMut for Read {}

impl Request<'_> {
    //#[instrument(level="trace", skip(w), ret)]
    pub fn send_read<W: AsFd + std::io::Write>(w: W, rdinfo: ReadParams) -> Result<Sequence> {
	let info = Read {
	    offset:	rdinfo.offset.into(),
	    size:	rdinfo.size.into(),
	    fh_flags:	rdinfo.flags.into(),
	};

	let hdr = Header::new(RequestCode::Read, &info);
	let seq = hdr.seq()?;

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(info.as_repr_bytes()) ])?;

	Ok(seq)
    }
}


#[repr(C)]
#[derive(Debug, Default)]
pub struct Ioctl {
    pub cmd:		be32,
    pub arg_type:	be8,
    _pad:		[be8;3],
}

unsafe impl AsReprBytes for Ioctl {}
unsafe impl AsReprBytesMut for Ioctl {}

impl Request<'_> {
    //#[instrument(level="trace", skip(w), ret)]
    pub fn send_ioctl<W: AsFd + std::io::Write>(w: W, cmd: ioctl, arg: Arg) -> Result<Sequence> {
	let info = Ioctl {
	    cmd:	cmd.as_numeric().into(),
	    arg_type:	arg.code(),
	    _pad:	Default::default(),
	};
	let data = arg.as_repr_bytes();

	let hdr = Header::with_payload(RequestCode::Ioctl, &info, data);
	let seq = hdr.seq()?;

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(info.as_repr_bytes()),
				IoSlice::new(data) ])?;

	Ok(seq)
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Poll {
    pub kh:		be64,
    pub flags:		be32,
    pub events:		be32,
}

unsafe impl AsReprBytes for Poll {}
unsafe impl AsReprBytesMut for Poll {}

impl Poll {
    pub const FLAG_SCHEDULE_NOTIFY: u32 = 1 << 0;
}

impl Request<'_> {
    //#[instrument(level="trace", skip(w), ret)]
    pub fn send_poll<W: AsFd + std::io::Write>(w: W, parm: PollParams) -> Result<Sequence> {
	let info = Poll {
	    kh:		parm.kh.into(),
	    flags:	parm.flags.as_ffi().into(),
	    events:	parm.events.as_ffi().into(),
	};

	let hdr = Header::new(RequestCode::Poll, &info);
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
