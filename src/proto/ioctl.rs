#[path = "ioctl_serial.rs"]
mod serial;

use std::mem::MaybeUninit;

use super::{ Error, Result, AsReprBytes };

use ensc_ioctl_ffi::{ffi::ioctl, BadIoctl};
use ensc_ioctl_ffi::ffi as ioctl_ffi;
pub use serial::*;
use crate::proto::endian::*;

#[derive(Copy, Clone, Debug)]
pub enum Source {
    Cuse,
    Device,
}

impl Source {
    pub fn is_cuse(self) -> bool {
	matches!(self, Self::Cuse)
    }

    pub fn is_device(self) -> bool {
	matches!(self, Self::Device)
    }
}

pub union OsArg {
    termios:	core::mem::ManuallyDrop<ioctl_ffi::termios>,
    termios2:	core::mem::ManuallyDrop<ioctl_ffi::termios2>,
    int:	nix::libc::c_int,
    uint:	nix::libc::c_uint,
    raw:	[u8; 64*1024],
}

impl OsArg {
    pub const SZ: usize = core::mem::size_of::<Self>();

    pub fn as_u64_ptr(&mut self) -> u64 {
	self as * mut _ as u64
    }

    pub fn as_slice(&self) -> &[u8] {
	unsafe { &self.raw }
    }

    pub fn as_termios_slice(&self) -> &[u8] {
	&self.as_slice()[..core::mem::size_of_val(unsafe { &self.termios })]
    }

    pub fn as_termios2_slice(&self) -> &[u8] {
	&self.as_slice()[..core::mem::size_of_val(unsafe { &self.termios2 })]
    }
}

#[derive(Debug, Clone)]
pub enum Arg {
    None,
    Arg(be64),
    Raw(Vec<u8>),
    RawArg(be64),
    TermIOs(TermIOs),
    Int(be32),
    UInt(be32),
}

impl Arg {
    pub fn new_os_arg() -> OsArg
    {
	unsafe { MaybeUninit::<OsArg>::uninit().assume_init() }
    }

    pub fn code(&self) -> be8 {
	match self {
	    Self::None		=> 0,
	    Self::Arg(_)	=> 1,
	    Self::Raw(_)	=> 2,
	    Self::RawArg(_)	=> 3,
	    Self::TermIOs(_)	=> 4,
	    Self::Int(_)	=> 5,
	    Self::UInt(_)	=> 6,
	}.into()
    }

    pub fn from_raw(code: u8, buf: &[u8]) -> Result<Self> {
	Ok(match code {
	    0	=> {
		if buf.len() > 0 {
		    warn!("Arg:None with non-empty data");
		}
		Self::None
	    },

	    1	=> Self::Arg(Self::try_as_object(buf)?),
	    2	=> Self::Raw(buf.to_vec()),
	    3	=> Self::RawArg(Self::try_as_object(buf)?),
	    4	=> Self::TermIOs(Self::try_as_object(buf)?),
	    5	=> Self::Int(Self::try_as_object(buf)?),
	    6	=> Self::UInt(Self::try_as_object(buf)?),

	    c	=> {
		warn!("bad raw ioctl code {c}");
		return Err(Error::BadIoctlParam);
	    }
	})
    }

    fn try_as_object<T: Sized>(buf: &[u8]) -> Result<T> {
	let sz = core::mem::size_of::<T>();
	if buf.len() < sz {
	    return Err(Error::BadLength);
	}

	let mut tmp = MaybeUninit::<T>::uninit();

	unsafe { (tmp.as_mut_ptr() as * mut u8).copy_from_nonoverlapping(buf.as_ptr(), sz) };

	Ok(unsafe { tmp.assume_init() })
    }

    fn try_as_be64(buf: &[u8]) -> Result<be64> {
	match buf.len() {
	    l if l < 8	=> Err(Error::BadLength),
	    _		=> Ok(u64::from_be_bytes([buf[0], buf[1], buf[2], buf[3],
						  buf[4], buf[5], buf[6], buf[7]]).into()),
	}
    }

    fn try_as_i32(buf: &[u8]) -> Result<be32> {
	match buf.len() {
	    l if l < 4	=> Err(Error::BadLength),
	    _		=> Ok(u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]).into()),
	}
    }

    fn try_as_u32(buf: &[u8]) -> Result<be32> {
	match buf.len() {
	    l if l < 4	=> Err(Error::BadLength),
	    _		=> Ok(u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]).into()),
	}
    }

    pub fn encode<'a>(self, cmd: u32, buf: &'a mut OsArg) -> Result<(u32, u64, &'a [u8])>
    {
	let cmd = BadIoctl::new(cmd.into());

	let size = cmd.get_size();

	if size < OsArg::SZ {
	    error!("not enough space for {:x}; required {size}, have {}",
		   cmd.get_native().as_numeric(), OsArg::SZ);
	    return Err(Error::BadLength);
	}

	let code = cmd.get_native().as_numeric();

	let (arg, buf): (u64, &'a [u8]) = match cmd.get_native() {
	    ioctl::TIOCSLCKTRMIOS |
	    ioctl::TCSETSW |
	    ioctl::TCSETSF |
	    ioctl::TCSETS		=> match self {
		Self::TermIOs(ios)	=> {
		    unsafe { *buf.termios = ios.into_os() };
		    (buf.as_u64_ptr(), &[])
		},
		_			=> return Err(Error::BadIoctlParam),
	    }

	    ioctl::TCSETSW2 |
	    ioctl::TCSETSF2 |
	    ioctl::TCSETS2		=> match self {
		Self::TermIOs(ios)	=> {
		    unsafe { *buf.termios2 = ios.into_os2() };
		    (buf.as_u64_ptr(), &[])
		},
		_			=> return Err(Error::BadIoctlParam),
	    }

	    ioctl::TIOCGLCKTRMIOS |
	    ioctl::TCGETS		=> match self {
		Self::None		=> {
		    (buf.as_u64_ptr(), buf.as_termios_slice())
		}
		_			=> return Err(Error::BadIoctlParam),
	    }

	    _ => todo!()
	};

	Ok((code, arg, buf))
    }

    pub fn decode(cmd: u32, arg: u64, buf: &[u8], src: Source) -> Result<Self> {
	let cmd = BadIoctl::new(cmd.into());
	let size = cmd.get_size();

	if buf.len() > 0 && size < buf.len() {
	    warn!("excess data in ioctl param ({size} < {})", buf.len());
	}

	Ok(match cmd.get_native() {
	    ioctl::TIOCSLCKTRMIOS |
	    ioctl::TCSETSW |
	    ioctl::TCSETSF |
	    ioctl::TCSETS		=> match src {
		Source::Cuse		=> Self::TermIOs(TermIOs::try_from_os(buf)?),
		Source::Device		=> Self::None,
	    },

	    ioctl::TCSETSW2 |
	    ioctl::TCSETSF2 |
	    ioctl::TCSETS2		=> match src {
		Source::Cuse		=> Self::TermIOs(TermIOs::try_from_raw_os2(buf)?),
		Source::Device		=> Self::None,
	    },

	    ioctl::TIOCSWINSZ		=> match src {
		Source::Cuse		=> todo!(),
		Source::Device		=> Self::None,
	    },

	    ioctl::TIOCGWINSZ		=> match src {
		Source::Cuse		=> Self::None,
		Source::Device		=> todo!(),
	    },

	    ioctl::TIOCGLCKTRMIOS |
	    ioctl::TCGETS		=> match src {
		Source::Cuse		=> Self::None,
		Source::Device		=> Self::TermIOs(TermIOs::try_from_os(buf)?),
	    },

	    ioctl::TCGETS2		=> match src {
		Source::Cuse		=> Self::None,
		Source::Device		=> Self::TermIOs(TermIOs::try_from_raw_os2(buf)?),
	    },

	    ioctl::TIOCSSOFTCAR |
	    ioctl::TIOCMSET |
	    ioctl::TIOCMBIC |
	    ioctl::TIOCMBIS		=> match src {
		Source::Cuse		=> Self::Int(Self::try_as_i32(buf)?),
		Source::Device		=> Self::None,
	    },

	    _ if cmd.is_write()		=> match src {
		Source::Cuse		=> Self::Raw(buf.to_vec()),
		Source::Device		=> Self::None,
	    },

	    _ if cmd.is_read()		=> match src {
		Source::Cuse		=> Self::None,
		Source::Device		=> Self::Raw(buf.to_vec()),
	    },

	    _				=> Self::RawArg(arg.into()),
	})
    }
}

unsafe impl AsReprBytes for Arg {
    fn as_repr_bytes(&self) -> &[u8] {
	match self {
	    Arg::None		=> &[],
	    Arg::Arg(arg)	=> arg.as_repr_bytes(),
	    Arg::Raw(raw)	=> raw.as_repr_bytes(),
	    Arg::RawArg(arg)	=> arg.as_repr_bytes(),
	    Arg::TermIOs(ios)	=> ios.as_repr_bytes(),
	    Arg::Int(i)		=> i.as_repr_bytes(),
	    Arg::UInt(u)	=> u.as_repr_bytes()
	}
    }
}
