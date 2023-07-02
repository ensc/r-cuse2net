#![allow(clippy::redundant_field_names)]

#[macro_use]
extern crate tracing;

#[macro_use]
mod util;

pub mod ffi;
mod error;
mod io;

use std::os::fd::AsFd;
use std::io::IoSlice;

pub use error::Error;
pub use io::ReadBuf;

pub struct CuseDevice<F: AsFd> {
    dev:	F,
}

impl <F: AsFd + std::fmt::Debug> std::fmt::Debug for CuseDevice<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CuseDevice").field("dev", &self.dev).finish()
    }
}

impl <F: AsFd + std::io::Read> CuseDevice<F> {
    pub fn reader(&self) -> &F {
	&self.dev
    }

    pub fn reader_mut(&mut self) -> &mut F {
	&mut self.dev
    }
}

impl <F: AsFd> CuseDevice<F> {
    pub fn new(dev: F) -> Self {
	Self {
	    dev: dev
	}
    }

    fn send(&self, data: &[IoSlice], total_len: usize) -> Result<(), Error> {
	use std::os::fd::AsRawFd;

	let fd = self.dev.as_fd().as_raw_fd();
	let sent_len = nix::sys::uio::writev(fd, data)?;

	if sent_len != total_len {
	    return Err(Error::BadSend(sent_len, total_len));
	}

	Ok(())
    }

    pub fn send_notify(&self, code: ffi::fuse_notify_code, data: &[u8]) -> Result<(), Error>
    {
	trace!("send_notify({code:?}, #{})", data.len());

	let len = core::mem::size_of::<ffi::fuse_out_header>() + data.len();
	let hdr = ffi::fuse_out_header {
	    len:	len as u32,
	    error:	code.as_native(),
	    unique:	ffi::unique::notify(),
	};

	self.send(&[ IoSlice::new(hdr.as_bytes()),
		     IoSlice::new(data) ], len)
    }

    pub fn send_error(&self, unique: ffi::unique, rc: u32) -> Result<(), Error>
    {
	trace!("send_error({unique:?}, {rc})");

	let hdr = ffi::fuse_out_header {
	    len:	core::mem::size_of::<ffi::fuse_out_header>() as u32,
	    error:	-(rc as i32),
	    unique:	unique
	};

	let iov = [
	    IoSlice::new(hdr.as_bytes())
	];

	self.send(&iov, hdr.len as usize)
    }

    pub fn send_response(&self, unique: ffi::unique, data: &[&[u8]]) -> Result<(), Error>
    {
	trace!("send_response({unique:?})");

	let len = data.iter().fold(core::mem::size_of::<ffi::fuse_out_header>(),
				   |acc, a| acc + a.len());

	let hdr = ffi::fuse_out_header {
	    len:	len as u32,
	    error:	0,
	    unique:	unique
	};

	let mut iov = Vec::with_capacity(data.len() + 1);

	iov.push(IoSlice::new(hdr.as_bytes()));

	for d in data {
	    iov.push(IoSlice::new(d));
	}

	self.send(&iov, len)
    }
}

pub trait AsBytes {
    fn as_bytes(&self) -> &[u8] {
	unsafe {
	    core::slice::from_raw_parts(self as * const _ as * const u8,
					core::mem::size_of_val(self))
	}
    }
}

impl AsBytes for ffi::fuse_out_header {}
impl AsBytes for ffi::fuse_open_out {}
impl AsBytes for ffi::fuse_write_out {}
impl AsBytes for ffi::fuse_ioctl_out {}
impl AsBytes for ffi::fuse_ioctl_iovec {}
impl AsBytes for ffi::fuse_notify_poll_wakeup_out {}
impl AsBytes for ffi::fuse_poll_out {}
impl AsBytes for ffi::cuse_init_out {}

#[derive(Debug, Clone)]
pub struct KernelVersion {
    pub major:	u32,
    pub minor:	u32,
}

impl Default for KernelVersion {
    fn default() -> Self {
        Self {
	    major:	ffi::FUSE_KERNEL_VERSION,
	    //	    minor:	ffi::FUSE_KERNEL_MINOR_VERSION,
	    minor:	31,
	}
    }
}

#[derive(Debug, Clone)]
pub struct OpInInfo {
    pub opcode: ffi::fuse_opcode,
    pub unique:	ffi::unique,
    pub nodeid:	u64,
    pub uid:	u32,
    pub gid:	u32,
    pub pid:	u32,
}

impl OpInInfo {
    pub fn send_ok<W: AsFd>(&self, w: &CuseDevice<W>) -> Result<(), Error>
    {
	self.send_error(w, nix::Error::from_i32(0))
    }

    pub fn send_error<W: AsFd>(&self, w: &CuseDevice<W>, rc: nix::Error) -> Result<(), Error>
    {
	w.send_error(self.unique, rc as u32)
    }

    pub fn send_response<W: AsFd>(&self, w: &CuseDevice<W>, data: &[&[u8]]) -> Result<(), Error>
    {
	w.send_response(self.unique, data)
    }
}

impl From<&ffi::fuse_in_header> for OpInInfo {
    fn from(value: &ffi::fuse_in_header) -> Self {
        Self {
	    opcode:	value.opcode,
	    unique:	value.unique,
	    nodeid:	value.nodeid,
	    uid:	value.uid,
	    gid:	value.gid,
	    pid:	value.pid,
	}
    }
}

#[derive(Debug, Clone)]
pub struct ReleaseParams {
    pub fh:		u64,
    pub flags:		ffi::fh_flags,
    pub release_flags:	ffi::release_flags,
    pub lock_owner:	u64,
}

#[derive(Debug, Clone)]
pub struct OpenParams {
    pub flags:		ffi::fh_flags,
    pub open_flags:	ffi::open_in_flags,
}

#[derive(Debug, Clone)]
pub struct IoctlParams {
    pub fh:		u64,
    pub flags:		ffi::ioctl_flags,
    pub cmd:		u32,
    pub arg:		u64,
    pub in_size:	u32,
    pub out_size:	u32,
}

#[derive(Debug, Clone)]
pub struct WriteParams {
    pub fh:		u64,
    pub offset:		u64,
    pub flags:		ffi::fh_flags,
    pub write_flags:	ffi::write_flags,
    pub lock_owner:	u64,
}

#[derive(Debug, Clone)]
pub struct ReadParams {
    pub fh:		u64,
    pub offset:		u64,
    pub size:		u32,
    pub read_flags:	ffi::read_flags,
    pub lock_owner:	u64,
    pub flags:		ffi::fh_flags,
}

#[derive(Debug, Clone)]
pub struct PollParams {
    pub fh:		u64,
    pub kh:		u64,
    pub flags:		ffi::poll_flags,
    pub events:		ffi::poll_events,
}

#[derive(Debug, Clone)]
pub enum OpIn<'a> {
    Unknown,
    CuseInit{ version: KernelVersion, flags: ffi::cuse_flags },
    FuseOpen(OpenParams),
    FuseRelease(ReleaseParams),
    FuseWrite(WriteParams, &'a[u8]),
    FuseRead(ReadParams),
    FuseIoctl(IoctlParams, &'a [u8]),
    FusePoll(PollParams),
    FuseInterrupt { unique: ffi::unique },
}

impl <'a> OpIn<'a> {
    pub fn read(iter: &'a mut crate::io::ReadBufIter) -> Result<(OpInInfo, Self), crate::Error> {
	let hdr: &ffi::fuse_in_header = iter.next()?.ok_or(Error::Eof)?;

	iter.truncate(hdr.len as usize)?;

	let res = match hdr.opcode {
	    ffi::fuse_opcode::CUSE_INIT		=> {
		let opdata: &ffi::cuse_init_in = iter.next()?.ok_or(Error::Eof)?;

		Self::CuseInit {
		    version: KernelVersion {
			major:	opdata.major,
			minor:	opdata.minor,
		    },
		    flags: opdata.flags,
		}
	    }

	    ffi::fuse_opcode::FUSE_OPEN		=> {
		let opdata: &ffi::fuse_open_in = iter.next()?.ok_or(Error::Eof)?;

		Self::FuseOpen(OpenParams {
		    flags:	opdata.flags,
		    open_flags:	opdata.open_flags
		})
	    },

	    ffi::fuse_opcode::FUSE_RELEASE	=> {
		let opdata: &ffi::fuse_release_in = iter.next()?.ok_or(Error::Eof)?;

		Self::FuseRelease(ReleaseParams {
		    fh:			opdata.fh,
		    flags:		opdata.flags,
		    release_flags:	opdata.release_flags,
		    lock_owner:		opdata.lock_owner,
		})
	    }

	    ffi::fuse_opcode::FUSE_WRITE	=> {
		let opdata: &ffi::fuse_write_in = iter.next()?.ok_or(Error::Eof)?;
		let wdata: &[u8] = iter.next_slice(opdata.size as usize)?.ok_or(Error::Eof)?;

		Self::FuseWrite(WriteParams {
		    fh:			opdata.fh,
		    offset:		opdata.offset,
		    write_flags:	opdata.write_flags,
		    lock_owner:		opdata.lock_owner,
		    flags:		opdata.flags,
		}, wdata)
	    }

	    ffi::fuse_opcode::FUSE_READ	=> {
		let opdata: &ffi::fuse_read_in = iter.next()?.ok_or(Error::Eof)?;

		Self::FuseRead(ReadParams {
		    fh:			opdata.fh,
		    size:		opdata.size,
		    offset:		opdata.offset,
		    read_flags:	opdata.read_flags,
		    lock_owner:		opdata.lock_owner,
		    flags:		opdata.flags,
		})
	    }

	    ffi::fuse_opcode::FUSE_INTERRUPT => {
		let opdata: &ffi::fuse_interrupt_in = iter.next()?.ok_or(Error::Eof)?;

		Self::FuseInterrupt {
		    unique:	opdata.unique,
		}
	    }

	    ffi::fuse_opcode::FUSE_IOCTL => {
		let opdata: &ffi::fuse_ioctl_in = iter.next()?.ok_or(Error::Eof)?;
		debug!("FUSE_IOCTL: {opdata:?}");
		let indata: &[u8] = iter.next_slice(opdata.in_size as usize)?.ok_or(Error::Eof)?;

		Self::FuseIoctl(IoctlParams {
		    fh:			opdata.fh,
		    flags:		opdata.flags,
		    cmd:		opdata.cmd,
		    arg:		opdata.arg,
		    in_size:		opdata.in_size,
		    out_size:		opdata.out_size,
		}, indata)
	    }

	    ffi::fuse_opcode::FUSE_POLL		=> {
		let opdata: &ffi::fuse_poll_in = iter.next()?.ok_or(Error::Eof)?;

		Self::FusePoll(PollParams {
		    fh:		opdata.fh,
		    kh:		opdata.kh,
		    flags:	opdata.flags,
		    events:	opdata.events,
		})
	    },

	    _		=> Self::Unknown,
	};

	if !iter.is_empty() {
	    warn!("excess elements in op-in data for {res:?} ({hdr:?})");
	}

	Ok((hdr.into(), res))
    }
}
