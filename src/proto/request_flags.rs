use super::super::endian::*;
use ensc_cuse_ffi::ffi as cuse_ffi;
use nix::fcntl::OFlag;

macro_rules! decl_flag {
    ($id:ident)		=> {
	#[repr(transparent)]
	#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
	pub struct $id(be32);

	impl $id {
	    const fn bit(pos: u8) -> Self {
		Self(be32::from_native(1 << pos))
	    }

	    pub const fn empty() -> Self {
		Self(be32::from_native(0))
	    }

	    pub const fn intersects(self, other: Self) -> bool {
		!self.bit_and(other).is_empty()
	    }

	    pub const fn is_empty(self) -> bool {
		self.0.as_native() == 0
	    }

	    pub const fn bit_and(self, rhs: Self) -> Self {
		Self(self.0.bit_and(rhs.0))
	    }

	    pub const fn bit_or(self, rhs: Self) -> Self {
		Self(self.0.bit_or(rhs.0))
	    }
	}

	impl std::ops::BitAnd for $id {
	    type Output = Self;

	    fn bitand(self, rhs: Self) -> Self::Output {
		Self(self.0 & rhs.0)
	    }
	}

	impl std::ops::BitOr for $id {
	    type Output = Self;

	    fn bitor(self, rhs: Self) -> Self::Output {
		Self(self.0 | rhs.0)
	    }
	}
    }
}

decl_flag!(FhFlags);


impl FhFlags {
    pub const RD: Self = Self::bit(0);
    pub const WR: Self = Self::bit(1);
    pub const NONBLOCK: Self = Self::bit(2);

    // TODO: make it const
    pub fn as_ffi(self) -> OFlag {
	let mut res = OFlag::empty();

	if self.intersects(Self::NONBLOCK) {
	    res |= OFlag::O_NONBLOCK;
	}

	match (self.intersects(Self::RD), self.intersects(Self::WR)) {
	    (true, true)	=> res |= OFlag::O_RDWR,
	    (false, true)	=> res |= OFlag::O_WRONLY,
	    (true, false)	=> res |= OFlag::O_RDONLY,
	    (false, false)	=> {}
	}

	res
    }

    pub const fn from_cuse(flags: cuse_ffi::fh_flags) -> Self {
	let mut res = Self::empty();

	if flags.intersects(cuse_ffi::fh_flags::NONBLOCK) {
	    res = res.bit_or(Self::NONBLOCK);
	}

	if flags.is_rdonly() || flags.is_rdwr() {
	    res = res.bit_or(Self::RD);
	}

	if flags.is_wronly() || flags.is_rdwr()  {
	    res = res.bit_or(Self::WR);
	}

	res
    }

    pub const fn is_nonblock(self) -> bool {
	(self.0.bit_and(Self::NONBLOCK.0)).as_native() != 0
    }
}

impl From<cuse_ffi::fh_flags> for FhFlags {
    fn from(value: cuse_ffi::fh_flags) -> Self {
        Self::from_cuse(value)
    }
}


#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct PollFlags(be32);

impl PollFlags {
    pub const SCHEDULE_NOTIFY: Self = Self::bit(0);

    const fn bit(pos: u8) -> Self {
	Self(be32::from_native(1 << pos))
    }

    pub const fn from_cuse(flags: cuse_ffi::poll_flags) -> Self {
	let mut res = 0;

	if flags.intersects(cuse_ffi::poll_flags::SCHEDULE_NOTIFY) {
	    res |= Self::SCHEDULE_NOTIFY.0.as_native();
	}

	Self(be32::from_native(res))
    }
}

impl From<cuse_ffi::poll_flags> for PollFlags {
    fn from(value: cuse_ffi::poll_flags) -> Self {
        Self::from_cuse(value)
    }
}
