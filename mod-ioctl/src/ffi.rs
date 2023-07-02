#![allow(non_camel_case_types)]

macro_rules! declare_ioctls {
    ($ns:expr, $map_fn:ident, { $( $vis:vis $ident:ident => $op:tt $data:tt ,)* })	=> {
	impl $crate::ffi::ioctl {
	    $( $vis const $ident: Self = declare_ioctls!(op => spec, $op $data); )*

	    pub(crate) const fn $map_fn(self) -> Option<&'static str> {
		#[allow(unreachable_patterns)]
		match self {
		    $( Self::$ident	=> Some(stringify!($ident)), )*
		    _			=> None,
		}
	    }
	}
    };

    (op => spec, BAD( $d: expr ))			=> { Self($d) };
    (op => spec, ALIAS( $d: expr ))			=> { $d };
    (op => spec, IOW( $id:expr, $nr:expr, $tp:ty ))	=> { Self::IOW::<$tp>($id, $nr) };
    (op => spec, IOR( $id:expr, $nr:expr, $tp:ty ))	=> { Self::IOR::<$tp>($id, $nr) };
    (op => spec, IORW( $id:expr, $nr:expr, $tp:ty ))	=> { Self::IORW::<$tp>($id, $nr) };
}

#[derive(Debug, Clone, Copy)]
struct BitGeo {
    bits:	u8,
    pos:	u8,
}

impl BitGeo {
    pub const fn new(bits: u8) -> Self {
	// we do not handle bits == 32 correctly in the bit mask operations
	// below
	assert!(bits <= 31);

	Self {
	    bits:	bits,
	    pos:	0,
	}
    }

    pub const fn next(&self, bits: u8) -> Self {
	// we do not handle bits == 32 correctly in the bit mask operations
	// below
	assert!(bits <= 31);
	assert!(self.pos + self.bits + bits <= 32);

	Self {
	    bits:	bits,
	    pos:	self.pos + self.bits,
	}
    }

    const fn mask(&self) -> u32 {
	(1 << self.bits) - 1
    }

    pub const fn encode(&self, val: u32) -> u32 {
	assert!(val < (1 << self.bits));

	val << self.pos
    }

    pub const fn decode(&self, val: u32) -> u32 {
	(val >> self.pos) & self.mask()
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ioctl(u32);

#[allow(non_snake_case)]
#[allow(dead_code)]
impl ioctl {
    const NR:   BitGeo = BitGeo::new(8);
    const TYPE: BitGeo = Self::NR.next(8);

    // TODO: size depends on arch!
    const SIZE: BitGeo = Self::TYPE.next(14);
    const DIR:  BitGeo = Self::SIZE.next(2);

    const DIR_NONE:  u32 = 0;
    const DIR_WRITE: u32 = 1;
    const DIR_READ:  u32 = 2;

    pub fn try_to_string(self) -> Option<&'static str> {
	for f in [Self::map_termios] {
	    if let Some(s) = f(self) {
		return Some(s);
	    }
	}

	None
    }

    pub(crate) const fn IOC(dir: u32, tp: u8, nr: u32, sz: usize) -> Self {
	Self(Self::DIR.encode(dir) | Self::TYPE.encode(tp as u32) |
	     Self::NR.encode(nr) | Self::SIZE.encode(sz as u32))
    }

    pub(crate) const fn IO(tp: u8, nr: u32) -> Self {
	Self::IOC(Self::DIR_NONE, tp, nr, 0)
    }

    pub(crate) const fn IOARG<T: Sized>(tp: u8, nr: u32) -> Self {
	Self::IOC(Self::DIR_NONE, tp, nr, core::mem::size_of::<T>())
    }

    pub(crate) const fn IOR<T: Sized>(tp: u8, nr: u32) -> Self {
	Self::IOC(Self::DIR_READ, tp, nr, core::mem::size_of::<T>())
    }

    pub(crate) const fn IOW<T: Sized>(tp: u8, nr: u32) -> Self {
	Self::IOC(Self::DIR_WRITE, tp, nr, core::mem::size_of::<T>())
    }

    pub(crate) const fn IOWR<T: Sized>(tp: u8, nr: u32) -> Self {
	Self::IOC(Self::DIR_WRITE | Self::DIR_READ, tp, nr, core::mem::size_of::<T>())
    }

    pub const fn as_numeric(self) -> u32 {
	self.0
    }

    pub const fn is_io(self) -> bool {
	Self::DIR.decode(self.0) != Self::DIR_NONE
    }

    pub const fn is_read(self) -> bool {
	(Self::DIR.decode(self.0) & Self::DIR_READ) != 0
    }

    pub const fn is_write(self) -> bool {
	(Self::DIR.decode(self.0) & Self::DIR_WRITE) != 0
    }

    pub const fn get_type(self) -> u8 {
	Self::TYPE.decode(self.0) as u8
    }

    pub const fn get_nr(self) -> u32 {
	Self::NR.decode(self.0)
    }

    pub const fn get_size(self) -> usize {
	Self::SIZE.decode(self.0) as usize
    }
}

impl std::fmt::Debug for ioctl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
	match self.try_to_string() {
	    None	=> f.write_fmt(format_args!("0x{:x}", self.0)),
	    Some(id)	=> f.write_str(id),
	}
    }
}

impl From<u32> for ioctl {
    fn from(value: u32) -> Self {
	Self::IOR::<u32>(1,2);
        Self(value)
    }
}

#[path = "ffi_serial.rs"]
mod serial;

pub use serial::*;


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_00() {
	#[repr(C)]
	struct TestObj([u8;17]);

	let ioctl_r  = ioctl::IOR::<TestObj>(b't', 1);
	let ioctl_w  = ioctl::IOW::<TestObj>(b't', 2);
	let ioctl_rw = ioctl::IOWR::<TestObj>(b't', 3);
	let ioctl_x  = ioctl::IO(b't', 4);

	assert!(ioctl_r.is_read());
	assert!(!ioctl_r.is_write());
	assert_eq!(ioctl_r.get_size(), core::mem::size_of::<TestObj>());
	assert_eq!(ioctl_r.get_type(), b't');

	assert!(!ioctl_w.is_read());
	assert!(ioctl_w.is_write());
	assert_eq!(ioctl_w.get_size(), core::mem::size_of::<TestObj>());
	assert_eq!(ioctl_w.get_type(), b't');

	assert!(ioctl_rw.is_read());
	assert!(ioctl_rw.is_write());
	assert_eq!(ioctl_rw.get_size(), core::mem::size_of::<TestObj>());
	assert_eq!(ioctl_rw.get_type(), b't');

	assert!(!ioctl_x.is_read());
	assert!(!ioctl_x.is_write());
	assert_eq!(ioctl_x.get_size(), 0);
	assert_eq!(ioctl_x.get_type(), b't');
    }
}
