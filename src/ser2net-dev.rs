//
#[macro_use]
extern crate tracing;

use r_ser2net::{ Result, Error };

use std::{path::{PathBuf, Path}, net::{TcpStream, TcpListener, IpAddr, SocketAddr}, sync::{RwLock, Arc}};

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
    #[clap(long, value_parser, value_name("FMT"), help("log format"),
	   default_value("default"))]
    log_format:		LogFormat,

    #[clap(short, long, value_parser, value_name("IP"), default_value("::"))]
    /// ip address to listen on
    listen:		std::net::IpAddr,

    #[clap(short, long, value_parser, default_value("8000"))]
    /// port to listen on
    port:		u16,

    #[clap(short, long, value_parser)]
    /// device
    device:		PathBuf,
}

fn run_thread(sock: TcpStream, device: PathBuf) -> Result<()> {
    use r_ser2net::proto;

    loop {
	let op = proto::Request::recv(&sock)?;

	debug!("running {op:?}");

    }
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

    let socket = TcpListener::bind(SocketAddr::new(args.listen, args.port))?;

    loop {
	let (conn, addr) = socket.accept()?;
	let device = args.device.clone();

	info!("connection from {addr:?}");

	std::thread::Builder::new()
	    .name(format!("{addr:?}"))
	    .spawn(move || {
		match run_thread(conn, device) {
		    Ok(_)	=> debug!("connection from {addr:?} finished successfully"),
		    Err(e)	=> warn!("connection from {addr:?} failed with {e:?}"),
		}
	    })?;
    }
}
