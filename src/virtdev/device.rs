use std::collections::HashMap;
use std::net::{TcpStream, SocketAddr};
use std::sync::Arc;
use std::thread::JoinHandle;

use parking_lot::RwLock;

use ensc_cuse_ffi::ffi::{self as cuse_ffi, ioctl_flags, fh_flags};
use ensc_cuse_ffi::{IoctlParams, OpInInfo, WriteParams, ReadParams, PollParams};

use ensc_ioctl_ffi::ffi as ioctl_ffi;
use ioctl_ffi::ioctl;

use crate::proto::Sequence;
use crate::proto::ioctl::Arg;
use crate::{CuseFileDevice, Error, proto};

use super::CONNECT_TIMEOUT;

#[derive(Clone, Debug)]
enum Request {
    Release,
    Write,
    Read,
    Ioctl(ioctl),
    Poll,
}

#[derive(Clone, Debug)]
enum Pending {
    Release,
    Write(WriteParams, Vec<u8>),
    Read(ReadParams),
    Ioctl{cmd: ioctl, arg: Arg},
    Poll(PollParams),
    Interrupt(Sequence),
}

#[derive(Default)]
struct State {
    closed:		bool,
    requests:		HashMap<Sequence, (Request, OpInInfo)>,
}

pub struct DeviceInner {
    cuse:		Arc<CuseFileDevice>,
    rx_hdl:		Option<JoinHandle<()>>,
    conn:		TcpStream,
    state:		RwLock<State>,
}

impl DeviceInner {
    fn is_closed(&self) -> bool {
	self.state.read().closed
    }

    fn remove_request(&self, seq: Sequence) -> Option<(Request, OpInInfo)> {
	self.state.write().requests.remove(&seq)
    }

    fn handle_ioctl(&self, info: OpInInfo, cmd: ioctl, retval: u64, arg: Arg) -> crate::Result<()> {
	use ensc_cuse_ffi::AsBytes;

	debug!("IOCTL: {cmd:?}, {retval:?}, {arg:?}");

	let data = arg.cuse_response(cmd)?;

	let hdr = cuse_ffi::fuse_ioctl_out {
	    result:		0,
	    flags:		ioctl_flags::UNRESTRICTED,
	    in_iovs:		0,
	    out_iovs:		1,
	};

	let mut resp_data: [&[u8];2] = [
	    hdr.as_bytes(),
	    &[],
	];

	let mut pos = 1;

	if let Some(data) = &data {
	    resp_data[pos] = data.as_ref();
	    pos += 1;
	}

	debug!("IOCTL: iov={resp_data:?}");

	info.send_response(&self.cuse, &resp_data[..pos])?;

	Ok(())
    }


    fn handle_error(&self, seq: Sequence, rc: i32) -> crate::Result<()> {
	let (_, info) = match self.remove_request(seq) {
	    None	=> {
		warn!("no such request {seq:?}");
		return Ok(());
	    }

	    Some(req)	=> req
	};

	info.send_error(&self.cuse, nix::Error::from_i32(rc))?;

	Ok(())
    }

    fn handle_response(&self, seq: Sequence, resp: proto::Response) -> crate::Result<()> {
	use ensc_cuse_ffi::AsBytes;
	use proto::Response as R;

	debug!("got response for seq {seq:?}");

	let (req, info) = match self.remove_request(seq) {
	    None	=> {
		warn!("no such request {seq:?}");
		return Ok(());
	    }

	    Some(req)	=> req
	};

	if let proto::Response::Err(err) = &resp {
	    info.send_error(&self.cuse, *err)?;
	    return Ok(());
	}

	match (req, resp) {
	    (Request::Release, R::Ok)		=> {
		self.state.write().closed = true;
		info.send_ok(&self.cuse)?;
	    }

	    (Request::Write, R::Write(sz))	=> {
		let write_resp = cuse_ffi::fuse_write_out {
		    size:	sz,
		    _padding:	0
		};

		info.send_response(&self.cuse, &[ write_resp.as_bytes() ])?;
	    }

	    (Request::Read, R::Read(data))	=>
		info.send_response(&self.cuse, &[ &data ])?,

	    (Request::Ioctl(cmd), R::Ioctl(retval, arg)) =>
		self.handle_ioctl(info, cmd, retval, arg)?,

	    (Request::Poll, R::Poll(ev)) => {
		let poll_resp = cuse_ffi::fuse_poll_out {
		    revents:	cuse_ffi::poll_events::from_ffi(ev),
		    padding:	0,
		};

		info.send_response(&self.cuse, &[ poll_resp.as_bytes() ])?;
	    }

	    (req, resp)				=> {
		warn!("unexpected response {resp:?} for {req:?}");
		return Err(proto::Error::BadResponse.into());
	    }
	}

	Ok(())
    }

    fn handle_event(&self, resp: proto::Response) -> crate::Result<()> {
	use ensc_cuse_ffi::AsBytes;
	use proto::Response as R;

	match resp {
	    R::PollWakeup(khs)	=> {
		for kh in khs {
		    let notify = cuse_ffi::fuse_notify_poll_wakeup_out {
			kh:	kh
		    };

		    self.cuse.send_notify(cuse_ffi::fuse_notify_code::FUSE_NOTIFY_POLL,
					  notify.as_bytes())?;
		}
	    }

	    R::PollWakeup1(kh)	=> {
		    let notify = cuse_ffi::fuse_notify_poll_wakeup_out {
			kh:	kh
		    };

		    self.cuse.send_notify(cuse_ffi::fuse_notify_code::FUSE_NOTIFY_POLL,
					  notify.as_bytes())?;
	    }

	    r			=> {
		warn!("unexpected event {r:?}");
		return Err(proto::Error::BadResponse.into());
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
		Ok((Some(seq), resp))	=>
		    if let Err(e) = self.handle_response(seq, resp) {
			warn!("failed to process request: {e:?}");
		    }

		Ok((None, ev))		=>
		    if let Err(e) = self.handle_event(ev) {
			warn!("failed to handle event: {e:?}");
		    }

		Err(proto::Error::RemoteError(Some(seq), rc))	=>
		    if let Err(e) = self.handle_error(seq, rc) {
			warn!("failed to handle error: {rc}@{seq:?}: {e:?}");
		    }

		Err(e)		=> {
		    warn!("error {e:?}");
		    break;
		}
	    };
	}

	let _ = self.conn.shutdown(std::net::Shutdown::Both);

	for info in self.state.write().requests.values() {
	    debug!("sending INTR to pending request {info:?}");
	    self.send_error(&info.1, nix::Error::EINTR);
	}

	info!("rx_thread terminated");
    }

    fn handle_cuse_internal(&self, req: Pending, info: OpInInfo) -> Result<(), (OpInInfo, Error)> {
	debug!("tx thread: handle {req:?}");

	let mut state = self.state.write();

	trace!("got state");

	let res = match req {
	    Pending::Release	=> {
		proto::Request::send_release(&self.conn)
		    .map(|seq| (seq, Request::Release))
	    },

	    Pending::Write(wrinfo, data)	=>
		proto::Request::send_write(&self.conn, wrinfo, &data)
		.map(|seq| (seq, Request::Write)),

	    Pending::Read(rdinfo)	=>
		proto::Request::send_read(&self.conn, rdinfo)
		.map(|seq| (seq, Request::Read)),

	    Pending::Ioctl { cmd, arg }	=>
		proto::Request::send_ioctl(&self.conn, cmd, arg)
		.map(|seq| (seq, Request::Ioctl(cmd))),

	    Pending::Poll(pollinfo)		=>
		proto::Request::send_poll(&self.conn, pollinfo)
		.map(|seq| (seq, Request::Poll)),

	    Pending::Interrupt(unique)		=> {
		proto::Request::send_interrupt(&self.conn, unique)
		    .map_err(|e| (info, e.into()))?;
		return Ok(())
	    }
	};

	match res {
	    Err(e)		=> Err((info, e.into())),
	    Ok((seq, pending))	=> {
		state.requests.insert(seq, (pending, info));
		Ok(())
	    }
	}
    }

    fn handle_cuse(&self, req: Pending, info: OpInInfo)  {
	match self.handle_cuse_internal(req, info) {
	    Ok(_)		=> {},
	    Err((info, e))	=> {
		warn!("failed to handle request: {e:?}");
		let _ = info.send_error(&self.cuse, nix::Error::EIO);
	    }
	}
    }

    pub fn try_interrupt(&self, info: OpInInfo, unique: cuse_ffi::unique_t) {
	let state = self.state.read();

	let mut request = state.requests.iter()
	    .filter(|(_, (_, info))| info.unique == unique);

	if let Some((seq, (req, _))) = request.next() {
	    trace!("interrupting active request #{seq:?} {req:?}");

	    assert!(request.next().is_none());

	    let seq = *seq;

	    drop(state);

	    self.handle_cuse(Pending::Interrupt(seq), info);
	}
    }

    pub fn ioctl(&self, info: OpInInfo, params: IoctlParams, data: &[u8])
    {
	let arg = match Arg::decode(params.cmd, params.arg, data, proto::ioctl::Source::Cuse) {
	    Err(e)	=> {
		error!("failed to decode ioctl: {e:?}");
		self.send_error(&info, nix::Error::EINVAL);
		return;
	    },

	    Ok(arg)	=> arg
	};

	if arg.is_raw() {
	    warn!("raw ioctl {params:?}/{arg:?}");
	}

	self.handle_cuse(Pending::Ioctl {
	    cmd: params.cmd.into(),
	    arg: arg
	}, info);
    }

    pub fn write(&self, info: OpInInfo, params: WriteParams, data: &[u8])
    {
	self.handle_cuse(Pending::Write(params, data.into()), info);
    }

    pub fn read(&self, info: OpInInfo, params: ReadParams)
    {
	self.handle_cuse(Pending::Read(params), info);
    }

    pub fn poll(&self, info: OpInInfo, params: PollParams)
    {
	self.handle_cuse(Pending::Poll(params), info);
    }

    fn send_error(&self, info: &OpInInfo, rc: nix::Error) {
	info.send_error(&self.cuse, rc)
	    .unwrap_or_else(|e| error!("failed to send error {rc:?}: {e:?}"));
    }
}

pub struct Device(Arc<DeviceInner>);

impl std::ops::Deref for Device {
    type Target = DeviceInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub(super) struct OpenArgs {
    pub addr:		SocketAddr,
    pub cuse:		Arc<CuseFileDevice>,
    pub flags:		fh_flags,
}

impl Device {
    fn run_remote_open(conn: &TcpStream, flags: fh_flags) -> Result<(), Error> {
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

    //#[instrument(level="trace")]
    pub(super) fn open(args: OpenArgs) -> Result<Self, Error> {
	let conn = TcpStream::connect_timeout(&args.addr, CONNECT_TIMEOUT)?;

	conn.set_nodelay(true)?;

	Self::run_remote_open(&conn, args.flags)?;

	let inner = Arc::new(DeviceInner {
	    cuse:		args.cuse,
	    conn:		conn,
	    state:		Default::default(),

	    rx_hdl:		None,
	});

	let inner = Arc::new(RwLock::new(inner));

	// hold the write lock until to end; this makes sure that 'Arc::get_mut()'
	// below sees only one instance.  Threads will do a read lock and start
	// after completing the initialization.
	let mut dev = inner.write();

	let inner_rx = inner.clone();

	let rx_hdl = std::thread::Builder::new()
	    .name("rx".to_string())
	    .spawn(move || {
		DeviceInner::rx_thread(inner_rx.read().clone())
	    })?;

	{
	    let dev = Arc::get_mut(&mut dev).unwrap();

	    dev.rx_hdl = Some(rx_hdl);
	}

	Ok(Self(dev.clone()))
    }

    pub fn release(self, info: OpInInfo)
    {
	info!("closing device");

	self.0.handle_cuse(Pending::Release, info);
    }
}
