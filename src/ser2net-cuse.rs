#![allow(clippy::redundant_field_names)]

#[macro_use]
extern crate tracing;

use std::sync::Arc;
use std::net::SocketAddr;
use ensc_cuse_ffi::{OpIn, KernelVersion};

use r_ser2net::{ Result, CuseFileDevice, virtdev };
use r_ser2net::virtdev::DeviceRegistry;

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
	.map(|d| Arc::new(CuseFileDevice::new(d)))?;

    let devices = DeviceRegistry::new(cuse.clone());
    let addr = args.server;

    let f = cuse.as_ref();

    let mut msg = ensc_cuse_ffi::ReadBuf::new();
    let mut is_init = true;

    info!("running ser2net-cuse");

    r_ser2net::deadlock_detect();

    loop {
	let mut iter = msg.read(&mut f.reader())?;

	let (info, op) = ensc_cuse_ffi::OpIn::read(&mut iter).unwrap();

	debug!("info={info:?}, op={op:?}");

	match op {
	    OpIn::CuseInit { flags, .. } if is_init	=> {
		let hdr = ensc_cuse_ffi::ffi::cuse_init_out {
		    major:	KernelVersion::default().major,
		    minor:	KernelVersion::default().minor,
		    flags:	flags, // ensc_cuse_ffi::ffi::cuse_flags::empty(),
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

	    OpIn::FuseOpen(params)			=>
		devices.create(addr, info, params)?,

	    OpIn::FuseRelease(params)			=>
		devices.release(params.fh, info),

	    OpIn::FuseWrite(params, data)		=>
		devices.for_fh(params.fh, |dev| dev.write(info, params, data)),

	    OpIn::FuseRead(params)			=>
		devices.for_fh(params.fh, |dev| dev.read(info, params)),

	    OpIn::FuseIoctl(args, data)			=>
		if virtdev::ioctl::cuse_complete_ioctl(f, info.unique, &args, data)? {
		    devices.for_fh(args.fh, |dev| dev.ioctl(info, args, data));
		}

	    OpIn::FuseInterrupt { unique }		=>
		devices.interrupt(info, unique),

	    OpIn::FusePoll(params)			=>
		devices.for_fh(params.fh, |dev| dev.poll(info, params)),

	    op						=> {
		warn!("unimplemented op {op:?}");
		let _ = info.send_error(f, nix::Error::ENOSYS);
	    }
	}
    }
}
