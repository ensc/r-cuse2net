use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::fd::AsFd;
use std::time::Duration;

use super::io::{recv_to, recv_exact_timeout, send_all, send_vectored_all};
use super::ioctl::{TermIOs, Arg};
use super::{Sequence, Result, AsReprBytes, AsReprBytesMut, TIMEOUT_READ, Error};
use super::endian::*;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResponseCode {
    Result = 1,
    Write = 2,
    Ioctl = 3,
}

impl ResponseCode {
    pub fn as_u8(&self) -> u8 {
	*self as u8
    }

    pub fn try_from_u8(v: u8) -> Option<Self> {
	Some(match v {
	    1	=> Self::Result,
	    2	=> Self::Write,
	    3	=> Self::Ioctl,
	    _	=> return None,
	})
    }
}

#[derive(Debug)]
pub enum Response {
    Ok,
    Write(u32),
    Ioctl(u8, Arg),
}

impl Response {
    const MAX_SZ: usize = 0x1_0000;

    #[instrument(level="trace", skip(w))]
    pub fn send_ioctl<W: AsFd + std::io::Write>(w: W, seq: Sequence, arg: Arg) -> Result<()> {
	let arg_type = arg.code().as_repr_bytes();
	let data = arg.as_repr_bytes();
	let hdr = Header {
	    op:		ResponseCode::Ioctl.as_u8().into(),
	    err:	0.into(),
	    len:	((arg_type.len() + data.len()) as u32).into(),
	    seq:	seq.0.into(),
	    ..Default::default()
	};

	send_vectored_all(w, &[ IoSlice::new(hdr.as_repr_bytes()),
				IoSlice::new(arg_type),
				IoSlice::new(data) ])?;

	Ok(())
    }

    #[instrument(level="trace", skip(w))]
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


    #[instrument(level="trace", skip(w))]
    pub fn send_err<W: AsFd + std::io::Write>(w: W, seq: Sequence, err: i32) -> Result<()> {
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

    #[instrument(level="trace", skip(w))]
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

	    ResponseCode::Ioctl				=> {
		let rc = recv_to(&r, be64::uninit(), &mut rx_len)?.into();
		let data = match *rx_len.as_ref().unwrap() {
		    0	=> Vec::new(),
		    len	=> {
			let mut data = Vec::with_capacity(len);
			let buf: &mut [u8] = unsafe {
			    std::slice::from_raw_parts_mut(data.as_mut_ptr(), len)
			};
			let buf = MaybeUninit::new(buf);

			recv_to(&r, buf, &mut rx_len)?;

			unsafe {
			    data.set_len(len);
			}

			data
		    }
		};


		Self::IoctlData(rc, data)
	    },

	    ResponseCode::IoctlTermios			=>
		Self::IoctlTermios(recv_to(&r, TermIOs::uninit(), &mut rx_len)?.into()),

	    ResponseCode::Result			=> {
		warn!("bad response {hdr:?}");
		return Err(Error::BadResponse);
	    },

	}))
    }

    #[instrument(level="trace", skip(r), ret)]
    pub fn recv_to<R: AsFd + std::io::Read>(r: R) -> Result<(Option<Sequence>, Self)> {
	Self::recv_internal(r, Some(TIMEOUT_READ))
    }

    #[instrument(level="trace", skip(r), ret)]
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

mod compile_test {
    #![allow(dead_code)]
    use super::*;

    fn test_00() {
	use core::mem::size_of;

	const _: () = assert!(size_of::<Header>() == 16);
    }
}
