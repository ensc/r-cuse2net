use crate::ffi::{ioctl, termios};

struct DirSize(u8, u16);

impl DirSize {
    pub const fn ioc(dir: u8, sz: usize) -> Self {
	Self(dir, sz as u16)
    }

    pub const fn IOR<T: Sized>() -> Self {
	Self::IOC(Self::DIR_READ, tp, nr, core::mem::size_of::<T>())
    }

    pub const fn IOW<T: Sized>(tp: u8, nr: u32) -> Self {
	Self::IOC(Self::DIR_WRITE, tp, nr, core::mem::size_of::<T>())
    }

    pub const fn IOWR<T: Sized>(tp: u8, nr: u32) -> Self {
	Self::IOC(Self::DIR_WRITE | Self::DIR_READ, tp, nr, core::mem::size_of::<T>())
    }

}

#[derive(Debug, Clone)]
pub enum BadIoctl {
    Native(ioctl),
    Fixup(ioctl, ioctl),
}

impl BadIoctl {
    const TCGETS: ioctl = ioctl::IOR::<termios>(b'T', 0x01);

    pub const fn new(cmd: ioctl) -> Self {
	match cmd {
	    ioctl::TCGETS	=> Self::Fixup(cmd, Self::TCGETS),
	    cmd			=> Self::Native(cmd),
	}
    }

    const fn get_good(&self) -> ioctl {
	match self {
	    BadIoctl::Native(cmd)	=> *cmd,
	    BadIoctl::Fixup(_, cmd)	=> *cmd,
	}
    }

    pub const fn is_io(self) -> bool {
	self.get_good().is_io()
    }

    pub const fn is_write(self) -> bool {
	self.get_good().is_write()
    }

    pub const fn is_read(self) -> bool {
	self.get_good().is_write()
    }

    pub const fn get_size(self) -> usize {
	self.get_good().get_size()
    }
}
