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

    /// Updates underlaying object
    ///
    /// ```should_panic
    /// # use core::mem::MaybeUninit;
    /// # use r_ser2net::proto::{ AsReprBytes, AsReprBytesMut };
    /// #[repr(transparent)]
    /// struct U32Proto(u32);
    /// unsafe impl AsReprBytes for U32Proto {};
    /// unsafe impl AsReprBytesMut for U32Proto {};
    ///
    /// let mut dat = U32Proto(0);
    ///
    /// dat.update_repr(&0x12345678_u32.to_ne_bytes());
    /// ```
    fn update_repr(&mut self, buf: &[u8]) {
	debug_assert_eq!(self as * const _ as * const u8, buf.as_ptr());
	debug_assert_eq!(core::mem::size_of_val(self), buf.len());
    }
}

// MaybeUninit<T>

unsafe impl <T: AsReprBytes + Sized> AsReprBytes for MaybeUninit<T> {
    fn as_repr_bytes(&self) -> &[u8] {
	let tmp: &T = unsafe { core::mem::transmute(self) };

	tmp.as_repr_bytes()
    }
}

unsafe impl <T: AsReprBytesMut + Sized> AsReprBytesMut for MaybeUninit<T> {
    fn as_repr_bytes_mut(&mut self) -> &mut [u8] {
	let tmp: &mut T = unsafe { core::mem::transmute(self) };

	tmp.as_repr_bytes_mut()
    }

    /// Updates underlaying object
    ///
    /// Usually, given slice must be exactly the underlying byte representation
    /// of the object.  For some objects, it might change content to a subslice
    /// of the contained data.
    ///
    /// E.g. it is forbidden to write
    ///
    /// ```should_panic
    /// # use core::mem::MaybeUninit;
    /// # use r_ser2net::proto::{ AsReprBytes, AsReprBytesMut };
    /// #[repr(transparent)]
    /// struct U32Proto(u32);
    /// unsafe impl AsReprBytes for U32Proto {};
    /// unsafe impl AsReprBytesMut for U32Proto {};
    ///
    /// let mut dat = U32Proto::uninit();
    ///
    /// dat.update_repr(&0x12345678_u32.to_ne_bytes());
    /// ```
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

// Vec<u8>

unsafe impl AsReprBytes for Vec<u8> {
    fn as_repr_bytes(&self) -> &[u8] {
	self.as_slice()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_00() {
	let mut data: &mut [u8] = &mut [0_u8; 9];

	assert_eq!(data.as_repr_bytes(), &[0, 0, 0, 0,  0, 0, 0, 0,  0]);
	assert_eq!(data.as_repr_bytes_mut(), &mut [0, 0, 0, 0,  0, 0, 0, 0,  0]);

	data.as_repr_bytes_mut()[1] = 23;

	assert_eq!(data.as_repr_bytes(), &[0, 23, 0, 0,  0, 0, 0, 0,  0]);
    }
}
