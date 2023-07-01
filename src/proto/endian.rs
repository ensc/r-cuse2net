macro_rules! declare_endian {
    ($name:ident, $type:ty, $conv:path) => {
	#[derive(Default, Copy, Clone, PartialEq, Eq)]
	#[repr(transparent)]
	#[allow(non_camel_case_types)]
	pub struct $name($type);

	impl $name {
	    pub const fn from_native(v:$type) -> Self {
		Self($conv(v))
	    }

	    pub const fn as_native(&self) -> $type {
		$conv(self.0)
	    }

	    pub fn bit_or_assign(&mut self, rhs: Self) {
		self.0 |= rhs.0;
	    }

	    pub fn bit_and_assign(&mut self, rhs: Self) {
		self.0 &= rhs.0;
	    }

	    pub const fn bit_or(self, rhs: Self) -> Self {
		Self(self.0 | rhs.0)
	    }

	    pub const fn bit_and(self, rhs: Self) -> Self {
		Self(self.0 & rhs.0)
	    }
	}

	impl From<$type> for $name {
	    fn from(value: $type) -> Self {
		Self::from_native(value)
	    }
	}

	impl From<$name> for $type {
	    fn from(value: $name) -> Self {
		value.as_native()
	    }
	}

	impl std::ops::BitAnd for $name {
	    type Output = Self;

	    fn bitand(self, rhs: Self) -> Self::Output {
		Self(self.0 & rhs.0)
	    }
	}

	impl std::ops::BitOr for $name {
	    type Output = Self;

	    fn bitor(self, rhs: Self) -> Self::Output {
		Self(self.0 | rhs.0)
	    }
	}

	impl std::fmt::Debug for $name {
	    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.as_native().fmt(f)
	    }
	}

	unsafe impl $crate::proto::AsReprBytes for $name {}
	unsafe impl $crate::proto::AsReprBytesMut for $name {}
    };
}

declare_endian!(le8,  u8,  u8::to_le);
declare_endian!(le16, u16, u16::to_le);
declare_endian!(le32, u32, u32::to_le);
declare_endian!(le64, u64, u64::to_le);

declare_endian!(be8,  u8,  u8::to_be);
declare_endian!(be16, u16, u16::to_be);
declare_endian!(be32, u32, u32::to_be);
declare_endian!(be64, u64, u64::to_be);
