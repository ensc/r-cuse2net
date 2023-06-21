use std::mem::MaybeUninit;

/// # Safety
///
/// Can only be applied to FFI objects without pointers or references
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

/// # Safety
///
/// Can only be applied to FFI objects without pointers or references
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

// MaybeUninit<T>

unsafe impl <T: AsReprBytes> AsReprBytes for MaybeUninit<T> {
    fn as_repr_bytes(&self) -> &[u8] {
	let tmp: &T = unsafe { core::mem::transmute(self) };

	tmp.as_repr_bytes()
    }
}

unsafe impl <T: AsReprBytesMut> AsReprBytesMut for MaybeUninit<T> {
    fn as_repr_bytes_mut(&mut self) -> &mut [u8] {
	let tmp: &mut T = unsafe { core::mem::transmute(self) };

	tmp.as_repr_bytes_mut()
    }

    fn update_repr(&mut self, buf: &[u8]) {
	let tmp: &mut T = unsafe { core::mem::transmute(self) };

	tmp.update_repr(buf)
    }
}

// &[u8]

unsafe impl AsReprBytes for &[u8] {
    fn as_repr_bytes(&self) -> &[u8] {
	self
    }
}

unsafe impl AsReprBytes for &mut [u8] {
    fn as_repr_bytes(&self) -> &[u8] {
	self
    }
}

unsafe impl AsReprBytesMut for &mut [u8] {
    fn as_repr_bytes_mut(&mut self) -> &mut [u8] {
	self
    }

    fn update_repr(&mut self, buf: &[u8]) {
	debug_assert_eq!(self.as_ptr(), buf.as_ptr());
	debug_assert_eq!(self.len(), buf.len());
    }
}
