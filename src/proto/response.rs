use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::fd::AsFd;
use std::time::Duration;

use super::io::{recv_to, recv_exact_timeout, send_all, send_vectored_all};
use super::ioctl::Arg;
use super::{Sequence, Result, AsReprBytes, AsReprBytesMut, TIMEOUT_READ, Error};
use super::endian::*;

pub type PollEvent = u32;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResponseCode {
    Result = 1,
    Write = 2,
    Read = 3,
    Ioctl = 4,
    Poll = 5,
    PollWakeup = 6,
    PollWakeup1 = 7,
}

impl ResponseCode {
    pub fn as_u8(&self) -> u8 {
	*self as u8
    }

    pub fn try_from_u8(v: u8) -> Option<Self> {
	Some(match v {
	    1	=> Self::Result,
	    2	=> Self::Write,
	    3	=> Self::Read,
	    4	=> Self::Ioctl,
	    5	=> Self::Poll,
	    6	=> Self::PollWakeup,
	    7	=> Self::PollWakeup1,

	    _	=> return None,
	})
    }
}

struct Alloc<T = u8> {
    buf: Vec<T>,
}

impl <T> Alloc<T> {
    pub fn new(sz: usize) -> Self {
	Self {
	    buf: Vec::with_capacity(sz)
	}
    }

    pub fn as_uninit(&mut self) -> MaybeUninit<&mut [T]> {
	let slice = unsafe {
	    core::slice::from_raw_parts_mut(self.buf.as_mut_ptr(), self.buf.capacity())
	};

	MaybeUninit::new(slice)
    }

    pub fn as_uninit_bytes(&mut self) -> MaybeUninit<&mut [u8]> {
	let slice = unsafe {
	    core::slice::from_raw_parts_mut(self.buf.as_mut_ptr() as * mut u8,
					    self.buf.capacity() * core::mem::size_of::<T>())
	};

	MaybeUninit::new(slice)
    }
}

#[derive(Debug)]
pub enum Response {
    Ok,
    Err(nix::Error),
    Write(u32),
    Read(Vec<u8>),
    Ioctl(u64, Arg),
    Poll(u32),
    PollWakeup(Vec<u64>),
    PollWakeup1(u64),
}

impl Response {
    const MAX_SZ: usize = 0x1_0000;

    pub fn send_poll<W: AsFd + std::io::Write>(w: W, seq: Sequence, ev: u32) -> Result<()> {
	let ev: be32 = ev.into();
	let ev = ev.as_repr_bytes();

	let hdr = Header {
	    op:		ResponseCode::Poll.as_u8().into(),
	    err:	0.into(),
	    len:	(ev.len() as u32).into(),
	    seq:	seq.0.into(),
	    ..Default::default()
	};

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(ev) ])?;

	Ok(())
    }

    fn send_poll_wakeup_1<W: AsFd + std::io::Write>(w: W, kh: u64) -> Result<()> {
	let kh: be64 = kh.into();
	let kh = kh.as_repr_bytes();
	let hdr = Header {
	    op:		ResponseCode::PollWakeup1.as_u8().into(),
	    err:	0.into(),
	    len:	(kh.len() as u32).into(),
	    seq:	0.into(),
	    ..Default::default()
	};

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(kh) ])?;

	Ok(())
    }

    pub fn send_poll_wakeup_n<W: AsFd + std::io::Write>(w: W, kh: &[u64]) -> Result<()> {
	let kh: Vec<be64> = kh.iter().map(|h| be64::from(*h)).collect();
	let kh: &[u8] = unsafe {
	    core::slice::from_raw_parts(kh.as_ptr() as * const u8, kh.len() * 8)
	};

	let hdr = Header {
	    op:		ResponseCode::PollWakeup.as_u8().into(),
	    err:	0.into(),
	    len:	(kh.len() as u32).into(),
	    seq:	0.into(),
	    ..Default::default()
	};

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(kh) ])?;

	Ok(())
    }

    pub fn send_poll_wakeup<W: AsFd + std::io::Write>(w: W, kh: &[u64]) -> Result<()> {
	match kh.len() {
	    0	=> Ok(()),
	    1	=> Self::send_poll_wakeup_1(w, kh[0]),
	    _	=> Self::send_poll_wakeup_n(w, kh),
	}
    }

    pub fn send_read<W: AsFd + std::io::Write>(w: W, seq: Sequence, data: &[u8]) -> Result<()> {
	let hdr = Header {
	    op:		ResponseCode::Read.as_u8().into(),
	    err:	0.into(),
	    len:	(data.len() as u32).into(),
	    seq:	seq.0.into(),
	    ..Default::default()
	};

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(data) ])?;

	Ok(())
    }

    //#[instrument(level="trace", skip(w))]
    pub fn send_ioctl<W: AsFd + std::io::Write>(w: W, seq: Sequence, rc: u64, arg: Arg) -> Result<()> {
	let ioctl = Ioctl {
	    retval:	rc.into(),
	    arg_type:	arg.code(),

	    _pad:	Default::default(),
	};
	let ioctl = ioctl.as_repr_bytes();
	let data = arg.as_repr_bytes();

	let hdr = Header {
	    op:		ResponseCode::Ioctl.as_u8().into(),
	    err:	0.into(),
	    len:	((ioctl.len() + data.len()) as u32).into(),
	    seq:	seq.0.into(),
	    ..Default::default()
	};

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(ioctl),
				IoSlice::new(data) ])?;

	Ok(())
    }

    //#[instrument(level="trace", skip(w))]
    pub fn send_write<W: AsFd + std::io::Write>(w: W, seq: Sequence, size: u32) -> Result<()> {
	let wrinfo: be32 = size.into();
	let hdr = Header {
	    op:		ResponseCode::Write.as_u8().into(),
	    err:	0.into(),
	    len:	(core::mem::size_of_val(&wrinfo) as u32).into(),
	    seq:	seq.0.into(),
	    ..Default::default()
	};

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(wrinfo.as_repr_bytes()) ])?;

	Ok(())
    }


    //#[instrument(level="trace", skip(w))]
    pub fn send_err<W: AsFd + std::io::Write>(w: W, seq: Sequence, err: nix::Error) -> Result<()> {
	let hdr = Header {
	    op:		ResponseCode::Result.as_u8().into(),
	    err:	(err as u16).into(),
	    len:	0.into(),
	    seq:	seq.0.into(),
	    ..Default::default()
	};

	send_all(w, hdr.as_repr_bytes())?;

	Ok(())
    }

    //#[instrument(level="trace", skip(w))]
    pub fn send_ok<W: AsFd + std::io::Write>(w: W, seq: Sequence) -> Result<()> {
	let hdr = Header {
	    op:		ResponseCode::Result.as_u8().into(),
	    err:	0.into(),
	    len:	0.into(),
	    seq:	seq.0.into(),
	    ..Default::default()
	};

	send_all(w, hdr.as_repr_bytes())?;

	Ok(())
    }

    fn recv_internal<R: AsFd + std::io::Read>(r: R, to: Option<Duration>) -> Result<(Option<Sequence>, Self)> {
	let mut hdr = Header::uninit();

	let hdr = recv_exact_timeout(&r, &mut hdr, &mut None, to, Some(TIMEOUT_READ))?;
	let len = hdr.len();

	if len > Self::MAX_SZ {
	    return Err(Error::PayloadTooLarge(len));
	}

	let mut rx_len = Some(len);

	let op = hdr.op();
	let op = ResponseCode::try_from_u8(op).
	    ok_or(Error::BadOp(op))?;
	let seq = hdr.seq();

	if hdr.err() != 0 {
	    return Err(Error::RemoteError(seq, hdr.err()));
	}

	Ok((seq, match op {
	    ResponseCode::Result if hdr.len() == 0	=>
		Self::Ok,

	    ResponseCode::Write				=>
		Self::Write(recv_to(&r, be32::uninit(), &mut rx_len)?.into()),

	    ResponseCode::Read				=> {
		let mut tmp = Alloc::new(*rx_len.as_ref().unwrap());
		let arg = recv_to(&r, tmp.as_uninit(), &mut rx_len)?;

		Self::Read(arg.into())
	    },

	    ResponseCode::Ioctl				=> {
		let ioctl = recv_to(&r, Ioctl::uninit(), &mut rx_len)?;
		let mut tmp = Alloc::new(*rx_len.as_ref().unwrap());
		let arg = recv_to(&r, tmp.as_uninit(), &mut rx_len)?;
		let arg = Arg::from_raw(ioctl.arg_type.into(), arg)?;

		Self::Ioctl(ioctl.retval.into(), arg)
	    },

	    ResponseCode::Poll				=> {
		let revent: PollEvent = recv_to(&r, be32::uninit(), &mut rx_len)?.into();

		Self::Poll(revent)
	    }

	    ResponseCode::PollWakeup1			=> {
		let kh: u64 = recv_to(&r, be64::uninit(), &mut rx_len)?.into();

		Self::PollWakeup1(kh)
	    }


	    ResponseCode::PollWakeup			=> {
		let len = *rx_len.as_ref().unwrap();
		if len % core::mem::size_of::<u64>() != 0 {
		    error!("len {len} not aligned");
		    return Err(Error::BadLength);
		}

		let mut tmp = Alloc::<be64>::new(len / core::mem::size_of::<u64>());
		let khs = recv_to(&r, tmp.as_uninit_bytes(), &mut rx_len)?;

		let (head, khs, tail) = unsafe {
		    khs.align_to::<be64>()
		};

		assert_eq!(head.len(), 0);
		assert_eq!(tail.len(), 0);

		Self::PollWakeup(khs.iter().map(|kh| (*kh).into()).collect())
	    }

	    ResponseCode::Result			=> {
		warn!("bad response {hdr:?}");
		return Err(Error::BadResponse);
	    },

	}))
    }

    //#[instrument(level="trace", skip(r), ret)]
    pub fn recv_to<R: AsFd + std::io::Read>(r: R) -> Result<(Option<Sequence>, Self)> {
	Self::recv_internal(r, Some(TIMEOUT_READ))
    }

    //#[instrument(level="trace", skip(r), ret)]
    pub fn recv<R: AsFd + std::io::Read>(r: R) -> Result<(Option<Sequence>, Self)> {
	Self::recv_internal(r, None)
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Header {
    op:		be8,
    _pad:	[u8;1],
    err:	be16,
    len:	be32,
    seq:	be64,
}

unsafe impl AsReprBytes for Header {}
unsafe impl AsReprBytesMut for Header {}

impl Header{
    pub fn new<T: Sized>(op: ResponseCode, seq: Sequence, payload: &T) -> Self {
	let len = core::mem::size_of_val(payload) as u32;

	Self {
	    op:		op.as_u8().into(),
	    err:	0.into(),
	    seq:	seq.0.into(),
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

    pub fn err(&self) -> i32 {
	self.err.as_native() as i32
    }

    pub fn seq(&self) -> Option<Sequence> {
	match self.seq.as_native() {
	    0	=> None,
	    v	=> Some(Sequence(v))
	}
    }
}

struct Ioctl {
    retval:	be64,
    arg_type:	be8,
    _pad:	[u8;7],
}

unsafe impl AsReprBytes for Ioctl {}
unsafe impl AsReprBytesMut for Ioctl {}

mod compile_test {
    #![allow(dead_code)]
    use super::*;

    fn test_00() {
	use core::mem::size_of;

	const _: () = assert!(size_of::<Header>() == 16);
    }
}
