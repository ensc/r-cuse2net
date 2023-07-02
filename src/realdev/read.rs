use std::mem::MaybeUninit;
use std::os::fd::{OwnedFd, AsRawFd, RawFd, FromRawFd};
use std::collections::VecDeque;

use nix::fcntl::OFlag;
use nix::poll::{PollFlags, PollFd};
use parking_lot::RwLock;

use crate::proto;
use crate::proto::Sequence;

use super::Device;

const BUF_SZ: usize = 4096;

type ReadRequest = (Sequence, usize);

pub struct ReadInner<'a> {
    device:		&'a super::Device,
    fd_rx:		Option<OwnedFd>,
    fd_tx:		Option<OwnedFd>,
    read_ops:		VecDeque<ReadRequest>,
    pending_request:	Option<ReadRequest>,
}

pub struct Read<'a>(RwLock<ReadInner<'a>>);

impl <'a> ReadInner<'a> {
    pub fn new(dev: &'a Device) -> nix::Result<Self> {
	let pipe = nix::unistd::pipe2(OFlag::O_CLOEXEC)?;

	Ok(Self {
	    device:		dev,
	    fd_rx:		Some(unsafe { OwnedFd::from_raw_fd(pipe.0) }),
	    fd_tx:		Some(unsafe { OwnedFd::from_raw_fd(pipe.1) }),
	    read_ops:		VecDeque::new(),
	    pending_request:	None,
	})
    }

    pub fn register_pending(&mut self, req: ReadRequest) {
	assert!(self.pending_request.is_none());
	self.pending_request = Some(req);
    }

    pub fn take_pending(&mut self) -> Option<ReadRequest> {
	self.pending_request.take()
    }

    fn next_request(&mut self) -> Option<ReadRequest> {
	self.read_ops.pop_front()
    }
}

impl <'a> Read<'a> {
    pub fn new(dev: &'a Device) -> nix::Result<Self> {
	ReadInner::new(dev).map(|d| Self(RwLock::new(d)))
    }
}

impl std::ops::Drop for Read<'_> {
    fn drop(&mut self) {
        self.close_internal()
    }
}

impl Read<'_> {
    fn is_alive(&self) -> bool {
	self.0.read().fd_tx.is_some()
    }

    fn next_request(&self) -> Option<ReadRequest> {
	self.0.write().next_request()
    }

    fn take_pending(&self) -> Option<ReadRequest> {
	self.0.write().take_pending()
    }

    fn send_sync(fd: RawFd) {
	#[allow(clippy::single_match)]
	match nix::unistd::write(fd, &[ b'R' ]) {
	    // TODO: what todo in error case?
	    Err(e)	=> error!("failed to send sync signal: {e:?}"),
	    _		=> (),
	}
    }

    pub fn push_request(&self, req: (Sequence, usize)) {
	let fd_sync = self.0.read().fd_tx.as_ref().unwrap().as_raw_fd();

	self.0.write().read_ops.push_back(req);
	Self::send_sync(fd_sync);
    }

    pub fn read_nonblock(&self, req: (Sequence, usize)) {
	#[allow(invalid_value, clippy::uninit_assumed_init)]
	let mut buf: [u8; BUF_SZ] = unsafe {
	    MaybeUninit::uninit().assume_init()
	};
	let fd_ser = self.0.read().device.fd.as_raw_fd();
	let l = req.1.min(buf.len());

	match nix::unistd::read(fd_ser, &mut buf[..l]) {
	    Ok(read_len)	=> self.send_data(req, &buf[..read_len]),
	    Err(e)		=> self.send_err(req.0, e),
	}
    }

    fn send_data(&self, req: ReadRequest, buf: &[u8]) {
	trace!("sending #{} bytes of data @{:?}", buf.len(), req.0);
	let _ = proto::Response::send_read(&self.0.read().device.conn, req.0, buf)
	    .map_err(|e| error!("failed to send data: {e:?}"));
    }

    fn send_err(&self, seq: Sequence, rc: nix::Error) {
	trace!("sending error {rc}@{seq:?}");

	let _ = proto::Response::send_err(&self.0.read().device.conn, seq, rc)
	    .map_err(|e| error!("failed to send err -{rc} response: {e:?}"));
    }

    fn consume_sync(&self, fd: RawFd) {
	#[allow(invalid_value, clippy::uninit_assumed_init)]
	let mut tmp: [u8;1] = unsafe {
	    MaybeUninit::uninit().assume_init()
	};

	match nix::unistd::read(fd, &mut tmp) {
	    Ok(1)	=> trace!("received sync char {tmp:?}"),
	    Ok(c)	=> warn!("unexpected number {c} of chars received"),
	    Err(e)	=> warn!("sync rx failed: {e:?}"),
	}

    }

    pub fn do_intr(&self, seq: Option<Sequence>) {
	let fd_sync = self.0.read().fd_tx.as_ref().unwrap().as_raw_fd();

	match seq {
	    None	=> {
		while let Some(req) = self.next_request() {
		    trace!("sending INTR to {req:?}");
		    self.send_err(req.0, nix::Error::EINTR);
		}

		if let Some(req) = self.take_pending() {
		    trace!("sending INTR to {req:?}");
		    self.send_err(req.0, nix::Error::EINTR);
		}
	    },

	    Some(seq)	=> {
		let mut this = self.0.write();

		match &this.pending_request {
		    Some((req_seq, _)) if *req_seq == seq	=> {
			trace!("sending INTR to {seq:?}");
			self.send_err(seq, nix::Error::EINTR);
			this.pending_request.take();
		    }

		    _		=> {
			let mut req = this.read_ops.iter()
			    .enumerate()
			    .filter(|(_, (req_seq, _))| *req_seq == seq);

			if let Some((pos, _)) = req.next() {
			    trace!("sending INTR to {req:?}");
			    self.send_err(seq, nix::Error::EINTR);

			    assert!(req.next().is_none());

			    drop(req);
			    this.read_ops.remove(pos);
			}
		    }
		}
	    }
	}

	Self::send_sync(fd_sync);
    }

    fn close_internal(&mut self) {
	let fd_sync = self.0.write().fd_tx.take().unwrap().as_raw_fd();

	self.do_intr(None);
	Self::send_sync(fd_sync);
    }

    fn handle_request(&self, fd_ser: RawFd, fd_sync: RawFd,
		      buf: &mut [u8], req: ReadRequest)
		      -> std::result::Result<Option<ReadRequest>, (nix::Error, Option<ReadRequest>)> {
	let l = req.1.min(buf.len());

	match nix::unistd::read(fd_ser, &mut buf[..l]) {
	    Ok(read_len)		=> {
		assert!(read_len <= l);
		self.send_data(req, &buf[..read_len]);
		Ok(None)
	    },

	    Err(e) if e == nix::Error::EAGAIN	=> {
		let mut fds = [
		    PollFd::new(fd_sync, PollFlags::POLLIN),
		    PollFd::new(fd_ser, PollFlags::POLLIN),
		];

		// register the pending request so that it can be seen by do_intr()
		self.0.write().register_pending(req);

		// wait either for synchronization event (new request) or data
		// on the serial device
		let rc = nix::poll::poll(&mut fds, -1);

		// do_intr() might have happen in the meantime which sent INTR
		// to the pending request which was consumed in this process
		let req = self.0.write().take_pending();

		rc.map_err(|e| (e, req))?;

		if fds[0].revents().map(|v| v.intersects(PollFlags::POLLIN)).unwrap_or(true) {
		    self.consume_sync(fd_sync)
		}

		Ok(req)
	    },

	    Err(e)				=> {
		warn!("failed to read from device: {e:?}");
		Err((e, Some(req)))
	    }
	}
    }

    pub fn run(&self) -> crate::Result<()> {
	let fd_ser = self.0.read().device.fd.as_raw_fd();

	// own the RX side of the sync pipe
	let fd_sync = self.0.write().fd_rx.take().unwrap();

	#[allow(invalid_value, clippy::uninit_assumed_init)]
	let mut buf: [u8; BUF_SZ] = unsafe {
	    MaybeUninit::uninit().assume_init()
	};

	while self.is_alive() {
	    let fd_sync = fd_sync.as_raw_fd();

	    match self.next_request() {
		None		=> self.consume_sync(fd_sync),
		Some(req)	=> match self.handle_request(fd_ser, fd_sync, &mut buf, req) {
		    Ok(Some(req))	=> {
			debug!("rescheduling read request {req:?}");
			self.0.write().read_ops.push_front(req);
		    }

		    Ok(None)		=>
			trace!("handled read request"),

		    Err((e, Some(req)))	=> {
			warn!("error while handling request {req:?}");
			self.send_err(req.0, e);
		    }

		    Err((e, None))	=>
			warn!("error while handling request: {e:?}"),
		}
	    }
	}

	Ok(())
    }
}
