mod endian;

pub use endian::*;

pub trait AsBytes {
    fn as_bytes(&self) -> &[u8] {
	unsafe {
	    core::slice::from_raw_parts(self as * const _ as * const u8,
					core::mem::size_of_val(self))
	}
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OpCode {
    Open = 1,
}

impl OpCode {
    pub fn as_u8(&self) -> u8 {
	*self as u8
    }

    pub fn try_from_u8(v: u8) -> Option<Self> {
	Some(match v {
	    1	=> Self::Open,
	    _	=> return None,
	})
    }
}

pub enum Op {
    Open(Open),
}

impl Op {
//    pub fn recv(
}

#[repr(C)]
#[derive(Default)]
pub struct Header {
    op:		be8,
    _pad:	[u8;3],
    len:	be32,
    id:		be64,
}

impl AsBytes for Header {}

#[repr(C)]
#[derive(Default)]
pub struct Open {
    flags:	be32,
    _pad:	[u8;4]
}

mod compile_test {
    #![allow(dead_code)]
    use super::*;

    fn test_00() {
	use core::mem::size_of;

	const _: () = assert!(size_of::<Header>() % 8 == 0);
	const _: () = assert!(size_of::<Open>() % 8 == 0);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_00() {
    }
}
