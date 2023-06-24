use std::os::fd::AsFd;

use ensc_cuse_ffi::IoctlParams;
use ensc_cuse_ffi::ffi as cuse_ffi;
use cuse_ffi::ioctl_flags;

use ensc_ioctl_ffi::ffi as ioctl_ffi;
use ioctl_ffi::ioctl;


use crate::Result;
use crate::CuseDevice;

pub fn decode_ioctl<F: AsFd>(dev: &CuseDevice<F>, unique: u64,
			     IoctlParams{ flags, cmd, arg, in_size, .. }: &IoctlParams,
			     _data: &[u8]) -> Result<bool> {
    use ensc_cuse_ffi::AsBytes;

    if !flags.contains(ioctl_flags::UNRESTRICTED) {
	return Ok(true);
    }

    let cmd: ioctl = (*cmd).into();
    let in_size = *in_size as usize;
    let flags = *flags;
    let arg = *arg;

    let (base, len) = match cmd {
	ioctl::TIOCSLCKTRMIOS |
	ioctl::TCSETSW |
	ioctl::TCSETSF |
	ioctl::TCSETS			=> (arg, core::mem::size_of::<ioctl_ffi::termios>()),
	ioctl::TIOCSWINSZ		=> (arg, core::mem::size_of::<ioctl_ffi::winsize>()),

	ioctl::TIOCSSOFTCAR |
	ioctl::TIOCMSET |
	ioctl::TIOCMBIC |
	ioctl::TIOCMBIS			=> (arg, core::mem::size_of::<nix::libc::c_int>()),

	cmd if cmd.is_write() && in_size == 0	=> (arg, cmd.get_size()),

	_					=> return Ok(true),
    };

    if len <= in_size {
	if len < in_size {
	    warn!("excess data from retried ioctl ({len} < {in_size})");
	}

	return Ok(true);
    }

    let hdr = cuse_ffi::fuse_ioctl_out {
	result:		0,
	flags:		flags | ioctl_flags::RETRY,
	in_iovs:	1,
	out_iovs:	0,
    };

    let iovec = cuse_ffi::fuse_ioctl_iovec {
	base:		base,
	len:		len as u64,
    };

    dev.send_response(unique, &[ hdr.as_bytes(), iovec.as_bytes() ])?;

    Ok(false)
}
