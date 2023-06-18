use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc};
use std::thread::JoinHandle;

use ensc_cuse_ffi::{OpInInfo, CuseDevice};
use parking_lot::{Condvar, RwLock, Mutex};

use crate::proto::Sequence;
use crate::{Error, proto};

use super::CONNECT_TIMEOUT;

#[derive(Copy, Clone, Debug)]
enum Request {
    Release,
}

#[derive(Default)]
struct State {
    closing:		bool,
    closed:		bool,
    requests:		HashMap<Sequence, (Request, OpInInfo)>,
    pending:		VecDeque<(Request, OpInInfo)>,
}

struct DeviceInner {
    cuse:		Arc<CuseDevice<File>>,
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
	debug!("got response for seq {seq:?}");

	match self.remove_request(seq) {
	    None				=> warn!("no such request {seq:?}"),
	    Some((Request::Release, info))	=> {
		self.state.write().closed = true;
		info.send_error(&self.cuse, 0)?;
	    }
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
    fn handle_cuse(&self, req: Request, info: OpInInfo) -> Result<(), (OpInInfo, Error)> {
	debug!("tx thread: handle {req:?}");

	let mut state = self.state.write();

	trace!("got state");

	let seq = match req {
	    Request::Release	=> {
		state.closing = true;
		proto::Request::send_release(&self.conn)
	    }
	};

	trace!("seq = {seq:?}");

	match seq {
	    Err(e)	=> Err((info, e.into())),
	    Ok(seq)	=> {
		state.requests.insert(seq, (req, info));
		Ok(())
	    }
	}
    }

    fn next_pending(&self) -> Option<(Request, OpInInfo)> {
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
			warn!("failed to handle request {req:?}: {e:?}");
			let _ = info.send_error(&self.cuse, nix::libc::EIO);
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
    pub cuse:		Arc<CuseDevice<File>>,
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

    pub fn release(self, info: OpInInfo) -> Result<(), Error>
    {
	info!("closing device");

	self.0.state.write().pending.push_back((Request::Release, info));
	self.0.state_change.notify_all();

	Ok(())
    }
}
