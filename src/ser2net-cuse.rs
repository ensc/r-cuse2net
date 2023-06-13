#![allow(clippy::redundant_field_names)]

#[macro_use]
extern crate tracing;

use std::{sync::Arc, os::fd::AsRawFd, path::PathBuf, net::SocketAddr};
use ensc_cuse_ffi::{OpIn, KernelVersion};

use nix::sys::socket::SockAddr;
use r_ser2net::{ Result, DeviceRegistry };

#[derive(clap::ValueEnum)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LogFormat {
    Default,
    Compact,
    Full,
    Json,
}

#[derive(clap::Parser, Debug)]
#[clap(author, version, about)]
struct CliOpts {
    #[clap(long, value_parser, value_name("FMT"), default_value("default"))]
    /// log format
    log_format:		LogFormat,

    #[clap(short,long, value_parser, value_name("server:port"))]
    /// device major number
    server:		SocketAddr,

    #[clap(short('m'), long, value_parser(1..=511), value_name("node-major"))]
    /// device major number
    major:		Option<i64>,

    #[clap(long, value_parser, value_parser(1..=255), value_name("node-minor"))]
    /// device minor number
    minor:		Option<i64>,

    #[clap(short, long, value_parser)]
    /// device name (without /dev)
    device:		String,
}

fn main() -> Result<()> {
    use clap::Parser;

    let mut args = CliOpts::parse();

    if args.log_format == LogFormat::Default {
	args.log_format = LogFormat::Full;
    }

    let fmt = tracing_subscriber::fmt()
	.with_env_filter(tracing_subscriber::EnvFilter::from_default_env());

    match args.log_format {
	LogFormat::Compact		=> fmt.without_time().init(),
	LogFormat::Json			=> fmt.json().init(),
	LogFormat::Full			=> fmt.init(),
	LogFormat::Default		=> unreachable!(),
    }

    use ensc_cuse_ffi::AsBytes;

    let cuse = std::fs::File::options()
	.write(true)
	.read(true)
	.open("/dev/cuse")
	.map(Arc::new)?;

    let devices = DeviceRegistry::new(cuse.clone());
    let addr = args.server;

    let mut f = cuse.as_ref();

    let mut msg = ensc_cuse_ffi::ReadBuf::new();
    let mut is_init = true;

    loop {
	let mut iter = msg.read(&mut f)?;

	let (info, op) = ensc_cuse_ffi::OpIn::read(&mut iter).unwrap();

	println!("info={info:?}, op={op:?}");

	match op {
	    OpIn::CuseInit { flags, .. } if is_init	=> {
		let hdr = ensc_cuse_ffi::ffi::cuse_init_out {
		    major:	KernelVersion::default().major,
		    minor:	KernelVersion::default().minor,
		    flags:	flags,
		    max_read:	msg.buf_size() as u32,
		    max_write:	msg.buf_size() as u32 - 0x1000,
		    dev_major:	*args.major.as_ref().unwrap_or(&0) as u32,
		    dev_minor:	*args.minor.as_ref().unwrap_or(&0) as u32,

		    _unused:	Default::default(),
		    _spare:	Default::default(),
		};

		info.send_response(f, &[
		    hdr.as_bytes(),
		    format!("DEVNAME={}\0", args.device).as_bytes()
		])?;

		is_init = false;
	    },

	    OpIn::FuseOpen { flags, open_flags }	=>
		devices.create(addr, info, flags, open_flags)?,

	    _	=> todo!(),
	}
    }

    Ok(())
}
