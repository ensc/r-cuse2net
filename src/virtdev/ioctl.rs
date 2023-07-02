use std::os::fd::AsFd;

use ensc_cuse_ffi::IoctlParams;
use ensc_cuse_ffi::ffi as cuse_ffi;
use cuse_ffi::ioctl_flags;

use ensc_ioctl_ffi::BadIoctl;

use crate::Result;
use crate::CuseDevice;

pub fn cuse_complete_ioctl<F: AsFd>(
    dev: &CuseDevice<F>, unique: cuse_ffi::unique_t,
    IoctlParams{ flags, cmd, arg, in_size, out_size, .. }: &IoctlParams,
    _data: &[u8]) -> Result<bool>
{
    use ensc_cuse_ffi::AsBytes;

    if !flags.intersects(ioctl_flags::UNRESTRICTED) {
	return Ok(true);
    }

    let cmd = BadIoctl::new((*cmd).into());
    let in_size = *in_size as usize;
    let out_size = *out_size as usize;
    let flags = *flags;
    let arg = *arg;

    if in_size > 0 || out_size > 0 {
	return Ok(true)
    }

    let info_in = match cmd.is_write() {
	true		=> Some((arg, cmd.get_size())),
	false		=> None,
    };

    let info_out = match cmd.is_read() {
	true		=> Some((arg, cmd.get_size())),
	false		=> None,
    };

    trace!("ioctl-retry: cmd={cmd:?}, in={info_in:?}, out={info_out:?}");

    if info_in.is_none() && info_out.is_none() {
	return Ok(true);
    }

    let info_in = info_in.map(|(base, len)| cuse_ffi::fuse_ioctl_iovec {
	base:		base,
	len:		len as u64,
    });

    let info_out = info_out.map(|(base, len)| cuse_ffi::fuse_ioctl_iovec {
	base:		base,
	len:		len as u64,
    });

    let hdr = cuse_ffi::fuse_ioctl_out {
	result:		0,
	flags:		flags | ioctl_flags::RETRY,
	in_iovs:	info_in.as_ref().map(|_| 1).unwrap_or(0),
	out_iovs:	info_out.as_ref().map(|_| 1).unwrap_or(0),
    };

    let mut iov: [&[u8];3] = Default::default();
    let mut pos = 0;

    iov[pos] = hdr.as_bytes();
    pos += 1;


    if let Some(info) = &info_in {
	iov[pos] = info.as_bytes();
	pos += 1;
    }

    if let Some(info) = &info_out {
	iov[pos] = info.as_bytes();
	pos += 1;
    }

    let iov = &iov[..pos];

    debug!("retry {hdr:?} + {iov:?}");

    dev.send_response(unique, iov)?;

    Ok(false)
}
