use crate::ffi::{ioctl};
use crate::ffi;

use nix::libc;

macro_rules! declare_bad {
    ({$( $ioctl:ident => $dir:tt($type:ty),)* })	=> {
	$( const $ioctl:	ioctl = ioctl::$dir::<$type>(ioctl::$ioctl.get_type(), ioctl::$ioctl.get_nr()); )*

	pub const fn new(cmd: ioctl) -> Self {
	    match cmd {
		$( ioctl::$ioctl	=> Self::Fixup(cmd, Self::$ioctl), )*
		_			=> Self::Native(cmd),
	    }
	}
    };
}

#[derive(Debug, Clone)]
pub enum BadIoctl {
    Native(ioctl),
    Fixup(ioctl, ioctl),
}

impl BadIoctl {
    declare_bad!({
	TCGETS		=> IOR(ffi::termios),
	TCSETS		=> IOW(ffi::termios),
	TCSETSW		=> IOW(ffi::termios),
	TCSETSF		=> IOW(ffi::termios),

	TIOCGLCKTRMIOS	=> IOR(ffi::termios),
	TIOCSLCKTRMIOS	=> IOW(ffi::termios),

	TIOCGSOFTCAR	=> IOR(libc::c_int),
	TIOCSSOFTCAR	=> IOW(libc::c_int),

	TIOCMGET	=> IOR(libc::c_int),
	TIOCMBIS	=> IOW(libc::c_int),
	TIOCMBIC	=> IOW(libc::c_int),
	TIOCMSET	=> IOW(libc::c_int),
    });
}

impl BadIoctl {
    pub const fn get_good(&self) -> ioctl {
	match self {
	    BadIoctl::Native(cmd)	=> *cmd,
	    BadIoctl::Fixup(_, cmd)	=> *cmd,
	}
    }

    pub const fn get_native(&self) -> ioctl {
	match self {
	    BadIoctl::Native(cmd)	=> *cmd,
	    BadIoctl::Fixup(cmd, _)	=> *cmd,
	}
    }

    pub const fn is_io(&self) -> bool {
	self.get_good().is_io()
    }

    pub const fn is_write(&self) -> bool {
	self.get_good().is_write()
    }

    pub const fn is_read(&self) -> bool {
	self.get_good().is_write()
    }

    pub const fn get_size(&self) -> usize {
	self.get_good().get_size()
    }
}
