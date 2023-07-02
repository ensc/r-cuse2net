#![allow(non_camel_case_types)]

pub const FUSE_KERNEL_VERSION: u32		= 7;
pub const FUSE_KERNEL_MINOR_VERSION: u32	= 36;

pub const FUSE_MIN_READ_BUFFER: usize = 8192;

macro_rules! declare_flags {
    ($id:ident, $type:ty, { $( $flag:ident = $bit:expr, )* })	=> {
	declare_flags!($id, $type, { $( $flag = $bit, )* }, special_map = |_| Option::<&str>::None, extra_all = 0);
    };

    ($id:ident, $type:ty, { $( $flag:ident = $bit:expr, )* }, special_map = $map:expr, extra_all = $extra_all:expr)	=> {
	#[repr(transparent)]
	#[derive(Clone, Copy, Default)]
	pub struct $id($type);

	impl $id {
	    $( pub const $flag: Self = Self(1 << $bit); )*

	    pub const fn from_ffi(v: $type) -> Self {
		Self(v)
	    }

	    pub const fn empty() -> Self {
		Self(0)
	    }

	    pub const fn all() -> Self {
		Self($extra_all $( | (1 << $bit) )*)
	    }

	    pub const fn bit_to_name(bit: u32) -> Option<&'static str> {
		match bit {
		    $( $bit	=> Some(stringify!($flag)), )*
		    _		=> None,
		}
	    }

	    pub const fn is_empty(self) -> bool {
		self.0 == 0
	    }

	    pub const fn intersects(self, other: Self) -> bool {
		self.0 & other.0 != 0
	    }

	    pub const fn as_ffi(self) -> $type {
		self.0
	    }
	}

	impl std::ops::BitAnd for $id {
	    type Output = Self;

	    fn bitand(self, rhs: Self) -> Self::Output {
		Self(self.0.bitand(rhs.0))
	    }
	}

	impl std::ops::BitOr for $id {
	    type Output = Self;

	    fn bitor(self, rhs: Self) -> Self::Output {
		Self(self.0.bitor(rhs.0))
	    }
	}

	impl std::fmt::Debug for $id {
	    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		use std::borrow::Cow;

		let unknown = self.0 & !Self::all().0;

		let mut v = self.0 & !unknown & !$extra_all;
		let mut bit = 0;
		let mut res = Vec::new();

		if let Some(info) = $map(&self) {
		    res.push(Cow::Borrowed(info));
		}

		while v != 0 {
		    if v & 1 != 0 {
			res.push(Cow::Borrowed(Self::bit_to_name(bit).unwrap()));
		    }

		    v >>= 1;
		    bit += 1;
		}

		if unknown != 0 {
		    res.push(Cow::Owned(format!("0x{unknown:x}")));
		}

		if res.is_empty() {
		    res.push(Cow::Borrowed("0"))
		}

		struct RawString(String);
		impl std::fmt::Debug for RawString {
		    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			f.write_str(&self.0)
		    }
		}

		f.write_str(&res.join(&"|"))
	    }
	}

    }
}

declare_flags!(fh_flags, u32, {
    CREAT = 6,
    EXCL = 7,
    NOCTTY = 8,
    TRUNC = 9,
    APPEND = 10,
    NONBLOCK = 11,
    DSYNC = 12,
    FASYNC = 13,
    DIRECT = 14,
    LARGEFILE = 15,
    DIRECTORY = 16,
    FOLLOW = 17,
    NOATIME = 18,
    CLOEXEC = 19,
},
	       special_map = Self::decode_acc,
	       extra_all = 3
);

 mod fh_flags_test {
     use super::fh_flags as F;
     use nix::libc;

     // TODO: bit positions above are arch dependent; generalize it
     const _: () = assert!(F::CREAT.as_ffi() as i32 == libc::O_CREAT);
     const _: () = assert!(F::NONBLOCK.as_ffi() as i32 == libc::O_NONBLOCK);
     const _: () = assert!(F::CLOEXEC.as_ffi() as i32 == libc::O_CLOEXEC);
}

impl fh_flags {
    fn decode_acc(&self) -> Option<&'static str> {
	match self.0 & 3 {
	    0	=> Some("RDONLY"),
	    1	=> Some("WRONLY"),
	    2	=> Some("RDWR"),
	    _	=> Some("BAD_ACC"),
	}
    }

    pub const fn is_rdonly(self) -> bool {
	(self.0 & 3) == 0
    }

    pub const fn is_wronly(self) -> bool {
	(self.0 & 3) == 1
    }

    pub const fn is_rdwr(self) -> bool {
	(self.0 & 3) == 2
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

declare_flags!(write_flags, u32, {
    CACHE = 0,
    LOCKOWNER = 1,
    KILL_SUIDGID = 2,
});

declare_flags!(read_flags, u32, {
    LOCKOWNER = 1,
});

declare_flags!(ioctl_flags, u32, {
    COMPAT = 0,
    UNRESTRICTED = 1,
    RETRY = 2,
    X32BIT = 3,
    DIR = 4,
    COMPAT_X32 = 5,
});

declare_flags!(poll_flags, u32, {
    SCHEDULE_NOTIFY = 0,
});

declare_flags!(poll_events, u32, {
    IN = 0,
    PRI = 1,
    OUT = 2,
    ERR = 3,
    HUP = 4,
    NVAL = 5,
    RDNORM = 6,
    RDBAND = 7,
    WRNORM = 8,
    WRBAND = 9,
    MSG = 10,
    REMOVE = 11,
    RDHUP = 12,
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

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct fuse_notify_code(i32);

impl fuse_notify_code {
    pub const FUSE_NOTIFY_POLL: Self = Self(1);

    pub const fn as_native(self) -> i32 {
	assert!(self.0 > 0);
	self.0
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_open_in {
    pub flags:		fh_flags,
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
    pub flags:		fh_flags,
    pub release_flags:	release_flags,
    pub lock_owner:	u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_write_in {
    pub fh:		u64,
    pub offset:		u64,
    pub size:		u32,
    pub write_flags:	write_flags,
    pub lock_owner:	u64,
    pub flags:		fh_flags,
    pub _padding:	u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_read_in {
    pub fh:		u64,
    pub offset:		u64,
    pub size:		u32,
    pub read_flags:	read_flags,
    pub lock_owner:	u64,
    pub flags:		fh_flags,
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
pub struct fuse_ioctl_in {
    pub fh:		u64,
    pub flags:		ioctl_flags,
    pub cmd:		u32,
    pub arg:		u64,
    pub in_size:	u32,
    pub out_size:	u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_ioctl_iovec {
    pub base:		u64,
    pub len:		u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_ioctl_out {
    pub result:		i32,
    pub flags:		ioctl_flags,
    pub in_iovs:	u32,
    pub out_iovs:	u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_poll_in {
    pub fh:		u64,
    pub kh:		u64,
    pub flags:		poll_flags,
    pub events:		poll_events,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_poll_out {
    pub revents:	poll_events,
    pub padding:	u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_notify_poll_wakeup_out {
    pub kh:		u64,
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
