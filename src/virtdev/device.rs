use std::collections::HashMap;
use std::fs::File;
use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;

use crate::proto::Sequence;
use crate::{Error, proto};

use super::CONNECT_TIMEOUT;

#[derive(Clone, Debug)]
enum Request {
    Release,
}

#[derive(Default)]
struct State {
    closing:	bool,
    requests:	HashMap<Sequence, Request>,
}

struct DeviceInner {
    cuse:	Arc<File>,
    rx_hdl:	Option<JoinHandle<()>>,
    tx_hdl:	Option<JoinHandle<()>>,
    fuse_hdl:	u64,
    conn:	TcpStream,
    flags:	u32,
    state:	RwLock<State>,
}

impl DeviceInner {
    fn rx_thread(self: Arc<Self>) {
	info!("rx_thread running");

	loop {
	    let op = proto::Response::recv(&self.conn);
	    debug!("rx: got {op:?}");

	    match op {
		Ok(_r)	=> {
		    todo!()
		}

		Err(e)	=> {
		    warn!("error {e:?}");
		    break;
		}
	    }
	}
    }

    fn tx_thread(self: Arc<Self>) {
	info!("tx_thread running");
    }
}

pub struct Device(Arc<DeviceInner>);

#[derive(Debug)]
pub(super) struct OpenArgs {
    pub addr:		SocketAddr,
    pub cuse:		Arc<File>,
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
	    cuse:	args.cuse,
	    fuse_hdl:	args.fuse_hdl,
	    flags:	args.flags,
	    conn:	conn,
	    state:	Default::default(),

	    rx_hdl:	None,
	    tx_hdl:	None,
	});

	let inner = Arc::new(RwLock::new(inner));

	// hold the write lock until to end; this makes sure that 'Arc::get_mut()'
	// below sees only one instance.  Threads will do a read lock and start
	// after completing the initialization.
	let mut dev = inner.write().unwrap();

	let inner_rx = inner.clone();
	let inner_tx = inner.clone();

	let rx_hdl = std::thread::Builder::new()
	    .name("rx".to_string())
	    .spawn(move || {
		DeviceInner::rx_thread(inner_rx.read().unwrap().clone())
	    })?;

	let tx_hdl = std::thread::Builder::new()
	    .name("tx".to_string())
	    .spawn(move || {
		DeviceInner::tx_thread(inner_tx.read().unwrap().clone())
	    })?;

	{
	    let dev = Arc::get_mut(&mut dev).unwrap();

	    dev.rx_hdl = Some(rx_hdl);
	    dev.tx_hdl = Some(tx_hdl);
	}

	Ok(Self(dev.clone()))
    }

    pub fn release(self) -> Result<(), Error>
    {
	let mut state = self.0.state.write().unwrap();

	info!("closing device");

	state.closing = true;

	let seq = proto::Request::send_release(&self.0.conn)?;

	state.requests.insert(seq, Request::Release);

	Ok(())
    }

}
