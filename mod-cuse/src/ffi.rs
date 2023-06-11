#![allow(dead_code, non_camel_case_types)]

pub const FUSE_KERNEL_VERSION: u32		= 7;
pub const FUSE_KERNEL_MINOR_VERSION: u32	= 36;

pub const FUSE_MIN_READ_BUFFER: usize = 8192;

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct flags(u32);

impl flags {
    const CUSE_UNRESTRICTED_IOCTL: Self = Self(1 << 0);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct open_flags(u32);

impl open_flags {
    const FUSE_OPEN_KILL_SUIDGID: Self = Self(1 << 0);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct fuse_opcode(u32);

impl fuse_opcode {
    pub const FUSE_OPEN: Self = Self(14);
    pub const FUSE_READ: Self = Self(15);
    pub const FUSE_WRITE: Self = Self(16);
    pub const FUSE_RELEASE: Self = Self(18);
    pub const FUSE_IOCTL: Self = Self(39);
    pub const FUSE_POLL: Self = Self(40);
    pub const CUSE_INIT: Self = Self(4096);
    pub const CUSE_INIT_BSWAP_RESERVED: Self = Self(1048576);
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_open_in {
    pub flags:		u32,
    pub open_flags:	open_flags,
}

#[repr(C)]
#[derive(Debug)]
pub struct fuse_open_out {
    pub fh:		u64,
    pub open_flags:	open_flags,
    pub _padding:	u32,
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
    pub flags:		flags,
}

#[repr(C)]
#[derive(Debug)]
pub struct cuse_init_out {
    pub major:		u32,
    pub minor:		u32,
    pub _unused:	u32,
    pub flags:		flags,
    pub max_read:	u32,
    pub max_write:	u32,
    pub dev_major:	u32,
    pub dev_minor:	u32,
    pub _spare:		[u32;10],
}
