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

	    pub const fn as_ffi(self) -> $type {
		self.0
	    }

	    pub const fn empty() -> Self {
		Self(0)
	    }

	    pub const fn is_empty(self) -> bool {
		self.0 == 0
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

	    pub const fn intersects(self, other: Self) -> bool {
		self.0 & other.0 != 0
	    }

	    pub const fn contains(self, other: Self) -> bool {
		self.0 & other.0 == other.0 && other.0 != 0
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

		#[allow(clippy::redundant_closure_call)]
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

pub struct FmtVecLen<'a, T>(pub &'a[T]);

impl <'a, T> std::fmt::Debug for FmtVecLen<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
	f.write_fmt(format_args!("#{}", self.0.len()))
    }
}
