use std::mem::MaybeUninit;

pub unsafe trait AsReprBytes {
    fn uninit() -> MaybeUninit<Self>
    where
	Self: Sized
    {
	MaybeUninit::uninit()
    }

    fn as_repr_bytes(&self) -> &[u8] {
	unsafe {
	    core::slice::from_raw_parts(self as * const _ as * const u8,
					core::mem::size_of_val(self))
	}
    }
}

pub unsafe trait AsReprBytesMut: AsReprBytes {
    fn as_repr_bytes_mut(&mut self) -> &mut [u8] {
	unsafe {
	    core::slice::from_raw_parts_mut(self as * mut _ as * mut u8,
					    core::mem::size_of_val(self))
	}
    }

    fn update_repr(&mut self, buf: &[u8]) {
	debug_assert_eq!(self as * const _ as * const u8, buf.as_ptr());
	debug_assert_eq!(core::mem::size_of_val(self), buf.len());
    }
}

unsafe impl <T: AsReprBytes> AsReprBytes for MaybeUninit<T> {}
unsafe impl <T: AsReprBytesMut> AsReprBytesMut for MaybeUninit<T> {}
