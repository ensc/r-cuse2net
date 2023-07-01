//

use std::{os::fd::{OwnedFd, FromRawFd, AsRawFd, RawFd}, collections::HashMap, mem::MaybeUninit};

use nix::{sys::epoll::{self, EpollEvent, EpollFlags}, poll::{PollFlags, PollFd}};
use parking_lot::RwLock;

use crate::proto::{ Sequence, response::PollEvent as ProtoEvent, self };

use super::Device;

type ReadRequest = (Sequence, usize);
type Kh = u64;

const TOK_SYNC: u64 = 1;
const TOK_SER: u64 = 2;

pub fn proto_to_poll(ev: ProtoEvent) -> PollFlags {
    fn map(ev: ProtoEvent, bit_proto: i16, bit_poll: PollFlags) -> PollFlags {
	match ev & (bit_proto as u32) {
	    0		=> PollFlags::empty(),
	    bit_proto	=> bit_poll,
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

pub struct PollInner<'a> {
    device:		&'a Device,
    fd_rx:		Option<OwnedFd>,
    fd_tx:		Option<OwnedFd>,

    khs:		HashMap<Kh, EpollFlags>,

    fd_epoll:		OwnedFd,
}

impl <'a> PollInner<'a> {
    pub fn new(dev: &'a Device) -> nix::Result<Self> {

	let pipe = nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC)?;
	let efd = epoll::epoll_create1(epoll::EpollCreateFlags::EPOLL_CLOEXEC)?;

	Ok(Self {
	    device:	dev,
	    fd_rx:	Some(unsafe { OwnedFd::from_raw_fd(pipe.0) }),
	    fd_tx:	Some(unsafe { OwnedFd::from_raw_fd(pipe.1) }),
	    fd_epoll:	unsafe { OwnedFd::from_raw_fd(efd) },

	    khs:	HashMap::new()
	})
    }

    pub fn signal(&self, ev: EpollFlags) {
	let khs: Vec<_> = self.khs.iter()
	    .filter(|(kh, kh_ev)| {
		ev.contains((**kh_ev) | EpollFlags::EPOLLERR | EpollFlags::EPOLLHUP)
	    })
	    .map(|(kh, kh_ev)| *kh)
	    .collect();

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

	if ev.is_empty() {
	    self.khs.remove(&kh);
	} else {
	    let ev = poll_to_epoll(ev);

	    self.khs.insert(kh, ev);
	}
    }

    pub fn poll(&self, req: (Sequence, ProtoEvent)) -> nix::Result<()> {
	let fd_ser = self.device.fd.as_raw_fd();
	let mut pfd = [
	    PollFd::new(fd_ser, proto_to_poll(req.1))
	];

	match nix::poll::poll(&mut pfd, 0) {
	    Ok(0)	=> self.send_events(req.0, PollFlags::empty()),
	    Ok(1)	=> self.send_events(req.0, pfd[0].revents().unwrap_or(PollFlags::empty())),
	    Ok(_)	=> panic!("unexpected value from poll()"),
	    Err(e)	=> return Err(e),
	}

	Ok(())
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
	let mut this = self.0.write();

	match this.poll((req.0, req.2)) {
	    Ok(_)	=> this.register_kh(req.1, proto_to_poll(req.2)),
	    Err(e)	=> this.send_err(req.0, e),
	}
    }

    pub fn poll_once(&self, req: (Sequence, ProtoEvent)) {
	let this = self.0.read();

	match this.poll(req) {
	    Ok(_)	=> {},
	    Err(e)	=> this.send_err(req.0, e),
	}
    }

    // TODO: move to super::
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

    pub fn run(&self) -> crate::Result<()> {
	let fd_ser = self.0.read().device.fd.as_raw_fd();
	let fd_sync = self.0.write().fd_rx.take().unwrap();
	let fd_sync = fd_sync.as_raw_fd();
	let efd = self.0.read().fd_epoll.as_raw_fd();

	let mut ev_sync = EpollEvent::new(EpollFlags::EPOLLIN, TOK_SYNC);
	let mut ev_ser  = EpollEvent::new(EpollFlags::EPOLLIN | EpollFlags::EPOLLOUT |
					  EpollFlags::EPOLLPRI | EpollFlags::EPOLLET, TOK_SER);

	epoll::epoll_ctl(efd, epoll::EpollOp::EpollCtlAdd, fd_sync, &mut ev_sync)?;
	epoll::epoll_ctl(efd, epoll::EpollOp::EpollCtlAdd, fd_ser,  &mut ev_ser)?;

	while self.is_alive() {
	    #[allow(invalid_value, clippy::uninit_assumed_init)]
	    let mut events: [EpollEvent;2] = unsafe {
		core::mem::MaybeUninit::uninit().assume_init()
	    };

	    let cnt = epoll::epoll_wait(efd, &mut events, -1)?;

	    for e in &events[..cnt] {
		match e.data() {
		    TOK_SYNC	=> self.consume_sync(fd_sync),
		    TOK_SER	=> self.0.read().signal(e.events()),
		    t		=> {
			error!("unexpected token {t}");
		    }
		}
	    }
	}

	todo!()
    }
}
