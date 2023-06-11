#[macro_use]
extern crate tracing;

pub mod ffi;
mod error;
mod io;

use std::{os::fd::{AsRawFd, RawFd}, io::IoSlice};

pub use error::Error;
pub use io::ReadBuf;

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
    pub unique:	u64,
    pub nodeid:	u64,
    pub uid:	u32,
    pub gid:	u32,
    pub pid:	u32,
}

impl OpInInfo {
    fn send(&self, fd: RawFd, data: &[IoSlice], total_len: usize) -> Result<(), Error> {
	let sent_len = nix::sys::uio::writev(fd, data)?;

	if sent_len != total_len {
	    return Err(Error::BadSend(sent_len, total_len));
	}

	Ok(())
    }


    pub fn send_error<W: AsRawFd>(&self, w: W, rc: nix::libc::c_int) -> Result<(), Error>
    {
	let hdr = ffi::fuse_out_header {
	    len:	core::mem::size_of::<ffi::fuse_out_header>() as u32,
	    error:	rc,
	    unique:	self.unique
	};

	let iov = [
	    IoSlice::new(hdr.as_bytes())
	];

	self.send(w.as_raw_fd(), &iov, hdr.len as usize)
    }

    pub fn send_response<W: AsRawFd>(&self, w: W, data: &[&[u8]]) -> Result<(), Error>
    {
	let len = data.iter().fold(core::mem::size_of::<ffi::fuse_out_header>(),
				   |acc, a| acc + a.len());

	let hdr = ffi::fuse_out_header {
	    len:	len as u32,
	    error:	0,
	    unique:	self.unique
	};

	let mut iov = Vec::with_capacity(data.len() + 1);

	iov.push(IoSlice::new(hdr.as_bytes()));

	for d in data {
	    iov.push(IoSlice::new(d));
	}

	self.send(w.as_raw_fd(), &iov, len)
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
pub enum OpIn {
    Unknown,
    CuseInit{ version: KernelVersion, flags: ffi::flags },
    FuseOpen{ flags: u32, open_flags: ffi::open_flags },
}

impl OpIn {
    pub fn read(iter: &mut crate::io::ReadBufIter) -> Result<(OpInInfo, Self), crate::Error> {
	let hdr: &ffi::fuse_in_header = iter.next()?.ok_or(Error::Eof)?;

	iter.truncate(hdr.len as usize)?;

	let res = match hdr.opcode {
	    ffi::fuse_opcode::CUSE_INIT	=> {
		let opdata: &ffi::cuse_init_in = iter.next()?.ok_or(Error::Eof)?;

		Self::CuseInit {
		    version: KernelVersion {
			major:	opdata.major,
			minor:	opdata.minor,
		    },
		    flags: opdata.flags,
		}
	    }

	    ffi::fuse_opcode::FUSE_OPEN	=> {
		let opdata: &ffi::fuse_open_in = iter.next()?.ok_or(Error::Eof)?;

		Self::FuseOpen {
		    flags:	opdata.flags,
		    open_flags:	opdata.open_flags
		}
	    },

	    _		=> Self::Unknown,
	};

	if !iter.is_empty() {
	    warn!("excess elements in op-in data");
	}

	Ok((hdr.into(), res))
    }
}
