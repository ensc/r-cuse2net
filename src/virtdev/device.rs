#![allow(unused_variables)]


use std::collections::{HashMap, VecDeque};
use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc};
use std::thread::JoinHandle;

use parking_lot::{Condvar, RwLock, Mutex};

use ensc_cuse_ffi::ffi as cuse_ffi;
use ensc_cuse_ffi::{IoctlParams, OpInInfo};

use ensc_ioctl_ffi::ffi as ioctl_ffi;
use ioctl_ffi::ioctl;

use crate::proto::Sequence;
use crate::proto::ioctl::TermIOs;
use crate::{CuseFileDevice, Error, proto};

use super::CONNECT_TIMEOUT;

#[derive(Clone, Debug)]
pub struct WriteInfo {
    pub offset:		u64,
    pub write_flags:	cuse_ffi::write_flags,
    pub flags:		u32,
    pub data:		Vec<u8>,
}

#[derive(Clone, Debug)]
enum Request {
    Release,
    Write,
    IoctlGeneric,
    IoctlTermiosGet(ioctl),
}

#[derive(Clone, Debug)]
enum Pending {
    Release,
    Write(Box<WriteInfo>),
    IoctlGeneric{ cmd: ioctl, arg: u64 },
    IoctlGenericRW{ cmd: ioctl, data: Option<Vec<u8>> },
    IoctlTermiosGet{ cmd: ioctl },
    IoctlTermiosSet{ cmd: ioctl, ios: crate::proto::ioctl::TermIOs },
}

#[derive(Default)]
struct State {
    closing:		bool,
    closed:		bool,
    requests:		HashMap<Sequence, (Request, OpInInfo)>,
    pending:		VecDeque<(Pending, OpInInfo)>,
}

struct DeviceInner {
    cuse:		Arc<CuseFileDevice>,
    rx_hdl:		Option<JoinHandle<()>>,
    tx_hdl:		Option<JoinHandle<()>>,
    fuse_hdl:		u64,
    conn:		TcpStream,
    flags:		u32,
    state:		RwLock<State>,
    state_change:	Condvar,
    state_mutex:	Mutex<()>,
}

impl DeviceInner {
    fn is_closed(&self) -> bool {
	self.state.read().closed
    }

    fn is_closing(&self) -> bool {
	self.state.read().closing
    }

    fn remove_request(&self, seq: Sequence) -> Option<(Request, OpInInfo)> {
	self.state.write().requests.remove(&seq)
    }

    #[instrument(level="trace", skip(self), ret)]
    fn handle_response(&self, seq: Sequence, resp: proto::Response) -> crate::Result<()> {
	use ensc_cuse_ffi::AsBytes;

	debug!("got response for seq {seq:?}");

	let (req, info) = match self.remove_request(seq) {
	    None	=> {
		warn!("no such request {seq:?}");
		return Ok(());
	    }

	    Some(req)	=> req
	};

	match req {
	    Request::Release	=> {
		self.state.write().closed = true;
		info.send_error(&self.cuse, 0)?;
	    }

	    Request::Write	=> {
		let sz = match resp {
		    proto::Response::Write(sz) => sz,
		    r				=> {
			warn!("unexpected response {r:?}");
			return Err(proto::Error::BadResponse.into());
		    }
		};

		let write_resp = cuse_ffi::fuse_write_out {
		    size:	sz,
		    _padding:	0
		};

		info.send_response(&self.cuse, &[ write_resp.as_bytes() ])?;
	    }

	    Request::IoctlTermiosGet(_cmd)	=> {
		//let termios = resp.
		//		match cmd {
		//		    ioctl::TCGETS	=>

		    todo!();
	    }

	    Request::IoctlGeneric	=> todo!(),

	}

	Ok(())
    }

    fn rx_thread(self: Arc<Self>) {
	info!("rx_thread running");

	while !self.is_closed() {
	    let op = proto::Response::recv(&self.conn);
	    debug!("rx: got {op:?}");

	    match op {
		Ok((Some(seq), resp))	=> {
		    if let Err(e) = self.handle_response(seq, resp) {
			warn!("failed to process request: {e:?}");
		    }
		},

		Ok((None, _))		=> warn!("no sequence received"),
		Err(e)			=> {
		    warn!("error {e:?}");
		    break;
		}
	    };
	}

	let _ = self.conn.shutdown(std::net::Shutdown::Both);

	info!("rx_thread terminated");
    }

    #[instrument(level="trace", skip(self), ret)]
    fn handle_cuse(&self, req: Pending, info: OpInInfo) -> Result<(), (OpInInfo, Error)> {
	debug!("tx thread: handle {req:?}");

	let mut state = self.state.write();

	trace!("got state");

	let res = match &req {
	    Pending::Release	=> {
		state.closing = true;
		proto::Request::send_release(&self.conn)
		    .map(|seq| (seq, Request::Release))
	    },

	    Pending::Write(wrinfo)	=>
		proto::Request::send_write(&self.conn, wrinfo.offset, &wrinfo.data)
		.map(|seq| (seq, Request::Write)),


	    Pending::IoctlTermiosGet { cmd }	=>
		proto::Request::send_termios_get(&self.conn)
		.map(|seq| (seq, Request::IoctlTermiosGet(*cmd))),

	    Pending::IoctlGeneric { cmd, arg }	=> todo!(),
	    Pending::IoctlGenericRW { cmd, data }	=> todo!(),
	    Pending::IoctlTermiosSet { cmd, ios }	=> todo!(),


	};

	match res {
	    Err(e)		=> Err((info, e.into())),
	    Ok((seq, pending))	=> {
		state.requests.insert(seq, (pending, info));
		Ok(())
	    }
	}
    }

    fn next_pending(&self) -> Option<(Pending, OpInInfo)> {
	self.state.write().pending.pop_front()
    }

    fn tx_thread(self: Arc<Self>) {
	info!("tx_thread running");

	loop {
	    trace!("tx: processing pending commands");

	    while let Some((req, info)) = self.next_pending() {
		match self.handle_cuse(req, info) {
		    Ok(_)		=> {},
		    Err((info, e))	=> {
			warn!("failed to handle request: {e:?}");
			let _ = info.send_error(&self.cuse, nix::libc::EIO as u32);
		    }
		}
	    }

	    if self.is_closing() {
		break;
	    }

	    trace!("tx: waiting for new data");

	    let mut lock = self.state_mutex.lock();
	    self.state_change.wait(&mut lock);
	}

	info!("tx_thread terminated");
    }
}

pub struct Device(Arc<DeviceInner>);

#[derive(Debug)]
pub(super) struct OpenArgs {
    pub addr:		SocketAddr,
    pub cuse:		Arc<CuseFileDevice>,
    pub fuse_hdl:	u64,
    pub flags:		u32,
}

impl Device {
    fn run_remote_open(conn: &TcpStream, flags: u32) -> Result<(), Error> {
	let seq = proto::Request::send_open(conn, flags)?;

	match proto::Response::recv_to(conn) {
	    Err(proto::Error::RemoteError(r_seq, _)) |
	    Ok((r_seq, _)) if r_seq != Some(seq)	=> {
		warn!("bad protocol sequence: {r_seq:?} vs. {seq:?}");
		return Err(proto::Error::BadSequence.into());
	    },

	    Ok((_, proto::Response::Ok))		=> {
		debug!("remote side opened device");
	    },

	    #[allow(unreachable_patterns)]
	    Ok((_, resp))				=> {
		warn!("unexpected response {resp:?}");
		return Err(proto::Error::BadResponse.into());
	    }

	    Err(proto::Error::RemoteError(_, err))	=> {
		warn!("remote side failed to open device: {err}");
		return Err(Error::Remote(err));
	    }

	    Err(e)					=> {
		warn!("failed to receive response for OPEN: {e:?}");
		return Err(e.into());
	    }
	}

	Ok(())
    }

    #[instrument(level="trace")]
    pub(super) fn open(args: OpenArgs) -> Result<Self, Error> {
	let conn = TcpStream::connect_timeout(&args.addr, CONNECT_TIMEOUT)?;

	conn.set_nodelay(true)?;

	Self::run_remote_open(&conn, args.flags)?;

	let inner = Arc::new(DeviceInner {
	    cuse:		args.cuse,
	    fuse_hdl:		args.fuse_hdl,
	    flags:		args.flags,
	    conn:		conn,
	    state:		Default::default(),
	    state_change:	Condvar::new(),
	    state_mutex:	Mutex::new(()),

	    rx_hdl:		None,
	    tx_hdl:		None,
	});

	let inner = Arc::new(RwLock::new(inner));

	// hold the write lock until to end; this makes sure that 'Arc::get_mut()'
	// below sees only one instance.  Threads will do a read lock and start
	// after completing the initialization.
	let mut dev = inner.write();

	let inner_rx = inner.clone();
	let inner_tx = inner.clone();

	let rx_hdl = std::thread::Builder::new()
	    .name("rx".to_string())
	    .spawn(move || {
		DeviceInner::rx_thread(inner_rx.read().clone())
	    })?;

	let tx_hdl = std::thread::Builder::new()
	    .name("tx".to_string())
	    .spawn(move || {
		DeviceInner::tx_thread(inner_tx.read().clone())
	    })?;

	{
	    let dev = Arc::get_mut(&mut dev).unwrap();

	    dev.rx_hdl = Some(rx_hdl);
	    dev.tx_hdl = Some(tx_hdl);
	}

	Ok(Self(dev.clone()))
    }

    pub fn release(self, info: OpInInfo)
    {
	info!("closing device");

	self.0.state.write().pending.push_back((Pending::Release, info));
	self.0.state_change.notify_all();
    }

    pub fn write(&self, info: OpInInfo, write_info: WriteInfo)
    {
	self.0.state.write().pending.push_back((Pending::Write(Box::new(write_info)), info));
	self.0.state_change.notify_all();
    }

    pub fn ioctl(&self, info: OpInInfo, params: IoctlParams, data: &[u8])
    {
	let cmd: ioctl = params.cmd.into();

	let req = match cmd {
	    ioctl::TIOCSLCKTRMIOS |
	    ioctl::TCSETSW |
	    ioctl::TCSETSF |
	    ioctl::TCSETS		=> Pending::IoctlTermiosSet {
		cmd:	cmd,
		ios:	TermIOs::try_from_os(data).unwrap(),
	    },

	    ioctl::TCSETSW2 |
	    ioctl::TCSETSF2 |
	    ioctl::TCSETS2		=> Pending::IoctlTermiosSet {
		cmd:	cmd,
		ios:	TermIOs::try_from_os2(data).unwrap(),
	    },

	    ioctl::TCGETS |
	    ioctl::TCGETS2		=> Pending::IoctlTermiosGet {
		cmd:	cmd,
	    },

	    ioctl::TIOCSSOFTCAR |
	    ioctl::TIOCMSET |
	    ioctl::TIOCMBIC |
	    ioctl::TIOCMBIS |
	    ioctl::TIOCSWINSZ		=> Pending::IoctlGenericRW {
		cmd:	cmd,
		data:	Some(data.to_owned()),
	    },

	    ioctl::TIOCMGET |
	    ioctl::TIOCGETD |
	    ioctl::TIOCMIWAIT |
	    ioctl::TIOCGWINSZ |
	    ioctl::FIONREAD |
	    ioctl::TIOCINQ |
	    ioctl::TIOCOUTQ |
	    ioctl::TIOCGSOFTCAR		=> Pending::IoctlGenericRW {
		cmd:	cmd,
		data:	None,
	    },

	    // TODO: handle internally
	    ioctl::TIOCGPGRP |
	    ioctl::TIOCSPGRP |
	    ioctl::TIOCGSID |
	    ioctl::TIOCGEXCL |
	    ioctl::TIOCNXCL |
	    ioctl::TIOCEXCL		=> {
		todo!()
	    }

	    ioctl::TCSBRK |
	    ioctl::TCSBRKP |
	    ioctl::TIOCSBRK |
	    ioctl::TIOCCBRK |
	    ioctl::TCXONC |
	    ioctl::TCFLSH |
	    ioctl::TIOCCONS |
	    ioctl::TIOCSCTTY |
	    ioctl::TIOCNOTTY		 => Pending::IoctlGeneric {
		cmd:	cmd,
		arg:	params.arg,
		},



	    c if !c.is_io()		=> Pending::IoctlGeneric {
		cmd:	cmd,
		arg:	params.arg,
	    },

	    c if c.is_write()		=> Pending::IoctlGenericRW {
		cmd:	cmd,
		data:	Some(data.to_owned()),
	    },

	    _				=> Pending::IoctlGenericRW {
		cmd:	cmd,
		data:	None,
	    }
	};

	self.0.state.write().pending.push_back((req, info));
	self.0.state_change.notify_all();
    }
}
