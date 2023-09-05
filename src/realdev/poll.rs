//

use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::os::fd::{OwnedFd, FromRawFd, AsRawFd, BorrowedFd, AsFd};

use nix::sys::epoll::{self, EpollEvent, EpollFlags};
use nix::poll::{PollFlags, PollFd};
use parking_lot::RwLock;

use crate::proto::{ Sequence, response::PollEvent as ProtoEvent, self };

use super::Device;

type Kh = u64;

const TOK_SYNC: u64 = 1;
const TOK_SER: u64 = 2;

pub fn proto_to_poll(ev: ProtoEvent) -> PollFlags {
    fn map(ev: ProtoEvent, bit_proto: i16, bit_poll: PollFlags) -> PollFlags {
	match ev & (bit_proto as u32) {
	    0		=> PollFlags::empty(),
	    _		=> bit_poll,
	}
    }

    map(ev, nix::libc::POLLIN,     PollFlags::POLLIN) |
    map(ev, nix::libc::POLLOUT,    PollFlags::POLLOUT) |
    map(ev, nix::libc::POLLPRI,    PollFlags::POLLPRI) |
    map(ev, nix::libc::POLLRDBAND, PollFlags::POLLRDBAND) |
    map(ev, nix::libc::POLLWRBAND, PollFlags::POLLWRBAND)
}

pub fn poll_to_epoll(ev: PollFlags) -> EpollFlags {
    fn map(ev: PollFlags, bit_poll: PollFlags, bit_epoll: EpollFlags) -> EpollFlags {
	match ev.contains(bit_poll) {
	    true	=> bit_epoll,
	    false	=> EpollFlags::empty(),
	}
    }

    map(ev, PollFlags::POLLIN,     EpollFlags::EPOLLIN) |
    map(ev, PollFlags::POLLOUT,    EpollFlags::EPOLLOUT) |
    map(ev, PollFlags::POLLPRI,    EpollFlags::EPOLLPRI) |
    map(ev, PollFlags::POLLRDBAND, EpollFlags::EPOLLRDBAND) |
    map(ev, PollFlags::POLLWRBAND, EpollFlags::EPOLLWRBAND)
}

#[cfg(test)]
mod test_conv {
    use super::*;

    #[test]
    fn test_00() {
	assert_eq!(proto_to_poll(nix::libc::POLLIN as u32),  PollFlags::POLLIN);
	assert_eq!(proto_to_poll(nix::libc::POLLOUT as u32), PollFlags::POLLOUT);
	assert_eq!(proto_to_poll((nix::libc::POLLOUT | nix::libc::POLLIN) as u32),
		   PollFlags::POLLOUT | PollFlags::POLLIN);
    }

    #[test]
    fn test_01() {
	assert_eq!(poll_to_epoll(PollFlags::POLLIN),  EpollFlags::EPOLLIN);
	assert_eq!(poll_to_epoll(PollFlags::POLLOUT), EpollFlags::EPOLLOUT);
	assert_eq!(poll_to_epoll(PollFlags::POLLIN | PollFlags::POLLOUT),
		   EpollFlags::EPOLLIN | EpollFlags::EPOLLOUT);
    }
}

pub struct PollInner<'a> {
    device:		&'a Device,
    fd_rx:		Option<OwnedFd>,
    fd_tx:		Option<OwnedFd>,

    khs:		HashMap<Kh, EpollFlags>,

    fd_epoll:		epoll::Epoll,
}

impl <'a> PollInner<'a> {
    pub fn new(dev: &'a Device) -> nix::Result<Self> {

	let pipe = nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC)?;
	let efd = epoll::Epoll::new(epoll::EpollCreateFlags::EPOLL_CLOEXEC)?;

	Ok(Self {
	    device:	dev,
	    fd_rx:	Some(unsafe { OwnedFd::from_raw_fd(pipe.0) }),
	    fd_tx:	Some(unsafe { OwnedFd::from_raw_fd(pipe.1) }),
	    fd_epoll:	efd,

	    khs:	HashMap::new()
	})
    }

    pub fn signal(&mut self, ev: EpollFlags) {
	trace!("signal({ev:?}, {:?}", self.khs);

	let khs: Vec<_> = self.khs.iter()
	    .filter(|(_, kh_ev)| {
		ev.intersects((**kh_ev) | EpollFlags::EPOLLERR | EpollFlags::EPOLLHUP)
	    })
	    .map(|(kh, _)| *kh)
	    .collect();

	for kh in &khs {
	    self.khs.remove(kh);
	}

	let _ = proto::Response::send_poll_wakeup(&self.device.conn, &khs)
	    .map_err(|e| error!("failed to send wakeup: {e:?}"));
    }

    pub fn send_events(&self, seq: Sequence, ev: PollFlags) {
	let _ = proto::Response::send_poll(&self.device.conn, seq, ev.bits() as ProtoEvent)
	    .map_err(|e| error!("failed to send wakeup: {e:?}"));
    }

    fn send_err(&self, seq: Sequence, rc: nix::Error) {
	let _ = proto::Response::send_err(&self.device.conn, seq, rc)
	    .map_err(|e| error!("failed to send err -{rc} response: {e:?}"));
    }

    pub fn register_kh(&mut self, kh: Kh, ev: PollFlags) {
	trace!("{kh}, {ev:?}");

	if ev.is_empty() {
	    self.khs.remove(&kh);
	} else {
	    let ev = poll_to_epoll(ev);

	    self.khs.insert(kh, ev);
	}
    }

    pub fn poll(&self, req: (Sequence, ProtoEvent)) -> nix::Result<bool> {
	let mut pfd = [
	    PollFd::new(&self.device.fd, proto_to_poll(req.1))
	];

	let res = match nix::poll::poll(&mut pfd, 0) {
	    Ok(0)	=> {
		self.send_events(req.0, PollFlags::empty());
		false
	    }
	    Ok(1)	=> {
		self.send_events(req.0, pfd[0].revents().unwrap_or(PollFlags::empty()));
		true
	    }
	    Ok(_)	=> panic!("unexpected value from poll()"),
	    Err(e)	=> return Err(e),
	};

	Ok(res)
    }

}

pub struct Poll<'a>(RwLock<PollInner<'a>>);

impl <'a> Poll<'a> {
    pub fn new(dev: &'a Device) -> nix::Result<Self> {
	PollInner::new(dev).map(|d| Self(RwLock::new(d)))
    }
}

impl Poll<'_> {
    fn is_alive(&self) -> bool {
	self.0.read().fd_tx.is_some()
    }

    pub fn poll(&self, req: (Sequence, Kh, ProtoEvent)) {
	trace!("poll{req:?}");

	let mut this = self.0.write();

	match this.poll((req.0, req.2)) {
	    Ok(false)	=> this.register_kh(req.1, proto_to_poll(req.2)),
	    Ok(true)	=> {},
	    Err(e)	=> this.send_err(req.0, e),
	}
    }

    pub fn poll_once(&self, req: (Sequence, ProtoEvent)) {
	trace!("poll_once{req:?}");

	let this = self.0.read();

	match this.poll(req) {
	    Ok(_)	=> {},
	    Err(e)	=> this.send_err(req.0, e),
	}
    }

    // TODO: move to super::
    fn consume_sync(&self, fd: BorrowedFd) {
	#[allow(invalid_value, clippy::uninit_assumed_init)]
	let mut tmp: [u8;1] = unsafe {
	    MaybeUninit::uninit().assume_init()
	};

	match nix::unistd::read(fd.as_raw_fd(), &mut tmp) {
	    Ok(1)	=> trace!("received sync char {tmp:?}"),
	    Ok(c)	=> warn!("unexpected number {c} of chars received"),
	    Err(e)	=> warn!("sync rx failed: {e:?}"),
	}
    }

    pub fn run(&self) -> crate::Result<()> {
	let ev_sync = EpollEvent::new(EpollFlags::EPOLLIN, TOK_SYNC);
	let ev_ser  = EpollEvent::new(EpollFlags::EPOLLIN | EpollFlags::EPOLLOUT |
				      EpollFlags::EPOLLPRI | EpollFlags::EPOLLET, TOK_SER);

	let efd = &self.0.read().fd_epoll;
	let fd_sync = self.0.write().fd_rx.take().unwrap();

	efd.add(&fd_sync, ev_sync)?;
	efd.add(&self.0.read().device.fd,  ev_ser)?;

	while self.is_alive() {
	    #[allow(invalid_value, clippy::uninit_assumed_init)]
	    let mut events: [EpollEvent;2] = unsafe {
		core::mem::MaybeUninit::uninit().assume_init()
	    };

	    let cnt = efd.wait(&mut events, -1)?;

	    for e in &events[..cnt] {
		match e.data() {
		    TOK_SYNC	=> self.consume_sync(fd_sync.as_fd()),
		    TOK_SER	=> self.0.write().signal(e.events()),
		    t		=> {
			error!("unexpected token {t}");
		    }
		}
	    }
	}

	Ok(())
    }
}
