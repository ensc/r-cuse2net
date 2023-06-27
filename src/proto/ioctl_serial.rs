use ensc_ioctl_ffi::ffi as ioctl_ffi;

use crate::proto::{endian::*, AsReprBytes, AsReprBytesMut};

use super::{ Error, Result };

#[repr(C,packed)]
#[derive(Debug, Clone)]
pub struct TermIOs {
    iflag:	be32,
    oflag:	be32,
    cflag:	be32,
    lflag:	be32,
    line:	be8,
    cc:		[be8;31],
    ispeed:	be32,
    ospeed:	be32,
    _pad:	u64,
}

unsafe impl AsReprBytes for TermIOs {}
unsafe impl AsReprBytesMut for TermIOs {}

const _: () = assert!(core::mem::size_of::<TermIOs>() == 0x40);

impl From<be32> for ioctl_ffi::c_iflag {
    fn from(value: be32) -> Self {
        Self(value.into())
    }
}

impl From<be32> for ioctl_ffi::c_oflag {
    fn from(value: be32) -> Self {
        Self(value.into())
    }
}

impl From<be32> for ioctl_ffi::c_cflag {
    fn from(value: be32) -> Self {
        Self(value.into())
    }
}

impl From<be32> for ioctl_ffi::c_lflag {
    fn from(value: be32) -> Self {
        Self(value.into())
    }
}

impl TermIOs {
    pub fn try_from_os(raw: &[u8]) -> Result<Self> {
	if raw.len() < core::mem::size_of::<ioctl_ffi::termios>() {
	    warn!("os termios param too short");
	    return Err(Error::BadIoctlParam);
	}

	let params = unsafe {
	    (raw as * const _ as * const ioctl_ffi::termios).read_unaligned()
	};

	let mut res = Self {
	    iflag:	params.c_iflag.0.into(),
	    oflag:	params.c_oflag.0.into(),
	    cflag:	params.c_cflag.0.into(),
	    lflag:	params.c_lflag.0.into(),
	    line:	params.c_line.into(),
	    cc:		Default::default(),
	    ispeed:	0.into(),
	    ospeed:	0.into(),

	    _pad:	0,
	};

	for (idx, v) in params.c_cc.into_iter().enumerate() {
	    res.cc[idx] = v.into();
	}

	Ok(res)
    }

    pub fn try_from_raw_os2(raw: &[u8]) -> Result<Self> {
	if raw.len() < core::mem::size_of::<ioctl_ffi::termios2>() {
	    warn!("os termios2 param too short");
	    return Err(Error::BadIoctlParam);
	}

	let params = unsafe {
	    (raw as * const _ as * const ioctl_ffi::termios2).read_unaligned()
	};

	Ok(Self::from_os2(&params))
    }

    pub fn from_os2(params: &ioctl_ffi::termios2) -> Self {
	let mut res = Self {
	    iflag:	params.c_iflag.0.into(),
	    oflag:	params.c_oflag.0.into(),
	    cflag:	params.c_cflag.0.into(),
	    lflag:	params.c_lflag.0.into(),
	    line:	params.c_line.into(),
	    cc:		Default::default(),
	    ispeed:	params.c_ispeed.into(),
	    ospeed:	params.c_ospeed.into(),
	    _pad:	0,
	};

	for (idx, v) in params.c_cc.iter().enumerate() {
	    res.cc[idx] = (*v).into();
	}

	res
    }

    pub fn into_os(self) -> ioctl_ffi::termios {
	let mut res = ioctl_ffi::termios {
	    c_iflag:	self.iflag.into(),
	    c_oflag:	self.oflag.into(),
	    c_cflag:	self.cflag.into(),
	    c_lflag:	self.lflag.into(),
	    c_line:	self.line.into(),
	    c_cc:	Default::default(),
	};

	for (idx, v) in res.c_cc.iter_mut().enumerate() {
	    *v = self.cc[idx].into();
	}

	res
    }

    pub fn into_os2(self) -> ioctl_ffi::termios2 {
	let mut res = ioctl_ffi::termios2 {
	    c_iflag:	self.iflag.into(),
	    c_oflag:	self.oflag.into(),
	    c_cflag:	self.cflag.into(),
	    c_lflag:	self.lflag.into(),
	    c_line:	self.line.into(),
	    c_ospeed:	self.ospeed.into(),
	    c_ispeed:	self.ispeed.into(),
	    c_cc:	Default::default(),
	};

	for (idx, v) in res.c_cc.iter_mut().enumerate() {
	    *v = self.cc[idx].into();
	}

	res
    }
}
