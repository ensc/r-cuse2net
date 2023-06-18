#![allow(dead_code, non_camel_case_types)]

pub const FUSE_KERNEL_VERSION: u32		= 7;
pub const FUSE_KERNEL_MINOR_VERSION: u32	= 36;

pub const FUSE_MIN_READ_BUFFER: usize = 8192;

macro_rules! declare_flags {
    ($id:ident, $type:ty, { $( $flag:ident = $bit:expr, )* })	=> {
	#[repr(transparent)]
	#[derive(Clone, Copy)]
	pub struct $id($type);

	impl $id {
	    $( const $flag: Self = Self(1 << $bit); )*

	    pub const fn empty() -> Self {
		Self(0)
	    }

	    pub const fn all() -> Self {
		Self(0 $( | (1 << $bit) )*)
	    }

	    pub const fn bit_to_name(bit: u32) -> Option<&'static str> {
		match bit {
		    $( $bit	=> Some(stringify!($flag)), )*
		    _		=> None,
		}
	    }
	}

	impl std::fmt::Debug for $id {
	    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		use std::borrow::Cow;

		let mut v = self.0;
		let mut bit = 0;
		let mut res = Vec::new();
		let mut unknown = 0;

		while v != 0 {
		    if v & 1 != 0 {
			match Self::bit_to_name(bit) {
			    Some(s)	=> res.push(Cow::Borrowed(s)),
			    None	=> unknown |= 1 << bit,
			}
		    }

		    v >>= 1;
		    bit += 1;
		}

		if unknown != 0 {
		    res.push(Cow::Owned(format!("0x{unknown:x}")));
		}

		struct RawString(String);
		impl std::fmt::Debug for RawString {
		    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			f.write_str(&self.0)
		    }
		}

		f.debug_tuple("cuse_flags")
		    .field(&RawString(res.join(&"-")))
		    .finish()
	    }
	}
    }
}

declare_flags!(cuse_flags, u32, {
    CUSE_UNRESTRICTED_IOCTL = 0,
});

declare_flags!(open_in_flags, u32, {
    KILL_SUIDGID = 0,
});

declare_flags!(open_out_flags, u32, {
    DIRECT_IO =	0,
    KEEP_CACHE = 1,
    NONSEEKABLE = 2,
    CACHE_DIR = 3,
    STREAM = 4,
    NOFLUSH = 5,
});

declare_flags!(release_flags, u32, {
    FLUSH = 0,
    FLOCK_UNLOCK = 1,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct fuse_opcode(u32);

impl fuse_opcode {
    pub const FUSE_OPEN: Self = Self(14);
    pub const FUSE_READ: Self = Self(15);
    pub const FUSE_WRITE: Self = Self(16);
    pub const FUSE_RELEASE: Self = Self(18);
    pub const FUSE_INTERRUPT: Self = Self(36);
    pub const FUSE_IOCTL: Self = Self(39);
    pub const FUSE_POLL: Self = Self(40);
    pub const CUSE_INIT: Self = Self(4096);
    pub const CUSE_INIT_BSWAP_RESERVED: Self = Self(1048576);
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_open_in {
    pub flags:		u32,
    pub open_flags:	open_in_flags,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_open_out {
    pub fh:		u64,
    pub open_flags:	open_in_flags,
    pub _padding:	u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_release_in {
    pub fh:		u64,
    pub flags:		u32,
    pub release_flags:	release_flags,
    pub lock_owner:	u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_write_in {
    pub fh:		u64,
    pub offset:		u64,
    pub size:		u32,
    pub write_flags:	u32,
    pub lock_owner:	u64,
    pub flags:		u32,
    pub _padding:	u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_write_out {
    pub size:		u32,
    pub _padding:	u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_interrupt_in {
    pub unique:		u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_in_header {
    pub len:		u32,
    pub opcode:		fuse_opcode,
    pub unique:		u64,
    pub nodeid:		u64,
    pub uid:		u32,
    pub gid:		u32,
    pub pid:		u32,
    pub _padding:	u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_out_header {
    pub len:		u32,
    pub error:		i32,
    pub unique:		u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct cuse_init_in {
    pub major:		u32,
    pub minor:		u32,
    pub _unused:	u32,
    pub flags:		cuse_flags,
}

#[repr(C)]
#[derive(Debug)]
pub struct cuse_init_out {
    pub major:		u32,
    pub minor:		u32,
    pub _unused:	u32,
    pub flags:		cuse_flags,
    pub max_read:	u32,
    pub max_write:	u32,
    pub dev_major:	u32,
    pub dev_minor:	u32,
    pub _spare:		[u32;10],
}
