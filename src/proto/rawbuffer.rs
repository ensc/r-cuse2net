use std::mem::MaybeUninit;

use super::{AsReprBytes, AsReprBytesMut};

pub struct RawBuffer {
    data:	Vec<u8>,
}

impl RawBuffer {
    pub fn new(sz: usize) -> MaybeUninit<Self> {
	MaybeUninit::new(Self {
	    data:	Vec::with_capacity(sz),
	})
    }

    pub fn into_inner(self) -> Vec<u8> {
	self.data
    }
}

unsafe impl AsReprBytes for RawBuffer {
    fn as_repr_bytes(&self) -> &[u8] {
	&self.data
    }
}

unsafe impl AsReprBytesMut for RawBuffer {
    fn as_repr_bytes_mut(&mut self) -> &mut [u8] {
	self.data.as_mut()
    }

    fn update_repr(&mut self, buf: &[u8]) {
	debug_assert_eq!(self.data.as_ptr(), buf.as_ptr());
	debug_assert!(self.data.capacity() >= buf.len());

	unsafe {
	    self.data.set_len(buf.len());
	}
    }

}
