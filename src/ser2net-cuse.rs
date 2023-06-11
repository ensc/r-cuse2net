#![allow(clippy::redundant_field_names)]

#[macro_use]
extern crate tracing;

use std::{sync::Arc, os::fd::AsRawFd};
use ensc_cuse_ffi::{OpIn, KernelVersion};

use r_ser2net::{ Result, DeviceRegistry };

fn main() -> Result<()> {
    use ensc_cuse_ffi::AsBytes;

    let addr = "127.0.0.1:6666".parse().unwrap();

    let cuse = std::fs::File::options()
	.write(true)
	.read(true)
	.open("/dev/cuse")?;
    let cuse = Arc::new(cuse);

    let devices = DeviceRegistry::new(cuse.clone());

    let mut f = cuse.as_ref();

    let mut msg = ensc_cuse_ffi::ReadBuf::new();

    loop {
	let mut iter = msg.read(&mut f)?;

	let (info, op) = ensc_cuse_ffi::OpIn::read(&mut iter).unwrap();

	println!("info={info:?}, op={op:?}");

	match op {
	    OpIn::CuseInit { flags, .. }		=> {
		let hdr = ensc_cuse_ffi::ffi::cuse_init_out {
		    major:	KernelVersion::default().major,
		    minor:	KernelVersion::default().minor,
		    flags:	flags,
		    max_read:	msg.buf_size() as u32,
		    max_write:	msg.buf_size() as u32 - 0x1000,
		    dev_major:	0,
		    dev_minor:	0,

		    _unused:	Default::default(),
		    _spare:	Default::default(),
		};

		info.send_response(f.as_raw_fd(), &[
		    hdr.as_bytes(),
		    "DEVNAME=x\0".as_bytes(),
		])?;
	    },

	    OpIn::FuseOpen { flags, open_flags }	=>
		devices.create(addr, info, flags, open_flags)?,

	    _	=> todo!(),
	}
    }

    Ok(())
}
