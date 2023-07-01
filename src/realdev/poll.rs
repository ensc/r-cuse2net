//

use std::{os::fd::{OwnedFd, FromRawFd, AsRawFd, RawFd}, collections::HashMap, mem::MaybeUninit};

use nix::sys::epoll::{self, EpollEvent, EpollFlags};
use parking_lot::RwLock;

use crate::proto::{ Sequence, response::PollEvent as ProtoEvent };

use super::Device;

type ReadRequest = (Sequence, usize);
type Kh = u64;

const TOK_SYNC: u64 = 1;
const TOK_SER: u64 = 2;

pub struct PollInner<'a> {
    device:		&'a Device,
    fd_rx:		Option<OwnedFd>,
    fd_tx:		Option<OwnedFd>,

    khs:		HashMap<Kh, EpollEvent>,

    kh_in:		Vec<Kh>,
    kh_out:		Vec<Kh>,
    kh_pri:		Vec<Kh>,

    fd_epoll:		OwnedFd,
}

impl <'a> PollInner<'a> {
    pub fn new(dev: &'a Device) -> nix::Result<Self> {

	let pipe = nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC)?;
	let efd = epoll::epoll_create1(epoll::EpollCreateFlags::EPOLL_CLOEXEC)?;

	Ok(Self {
	    device:	dev,
	    fd_rx:	Some(unsafe { OwnedFd::from_raw_fd(pipe.0.into()) }),
	    fd_tx:	Some(unsafe { OwnedFd::from_raw_fd(pipe.1.into()) }),
	    fd_epoll:	unsafe { OwnedFd::from_raw_fd(efd) },

	    kh_in:	Vec::new(),
	    kh_out:	Vec::new(),
	    kh_pri:	Vec::new(),
	})
    }

    pub fn signal(&self, ev: EpollFlags) {
	use std::collections::hash_map::Entry;

	let mut res: HashMap<Kh, u32> = HashMap::new();

	fn add(ev: EpollFlags, res: &mut HashMap<Kh, ProtoEvent>, vec: &Vec<Kh>, e_flag: EpollFlags,
	       mut p_flag: nix::libc::c_short)
	{
	    if ev.contains(EpollFlags::EPOLLERR) {
		p_flag |= nix::libc::POLLERR;
	    } else if ev.contains(EpollFlags::EPOLLHUP) {
		p_flag |= nix::libc::POLLHUP;
	    } else if !ev.contains(e_flag) {
		return;
	    }

	    for kh in vec {
		match res.entry(*kh) {
		    Entry::Occupied(mut e)	=> *(e.get_mut()) |= p_flag as ProtoEvent,
		    Entry::Vacant(v)		=> {
			v.insert(p_flag as ProtoEvent);
		    }
		}
	    }
	}

	add(ev, &mut res, &self.kh_in,  EpollFlags::EPOLLIN,  nix::libc::POLLIN);
	add(ev, &mut res, &self.kh_out, EpollFlags::EPOLLOUT, nix::libc::POLLOUT);
	add(ev, &mut res, &self.kh_pri, EpollFlags::EPOLLPRI, nix::libc::POLLPRI);

	for (kh, ev) in res {
	}
    }

    pub fn get_events(&self) -> Option<EpollFlags> {
	fn chk(vec: &Vec<u64>, flag: EpollFlags) -> EpollFlags {
	    match vec.is_empty() {
		true	=> EpollFlags::empty(),
		false	=> flag,
	    }
	}

	let ev =
	    chk(&self.kh_in,  EpollFlags::EPOLLIN) |
	    chk(&self.kh_pri, EpollFlags::EPOLLPRI) |
	    chk(&self.kh_out, EpollFlags::EPOLLOUT);

	match ev.is_empty() {
	    true	=> None,
	    false	=> Some(ev)
	}
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

    // TODO: move to super::
    fn consume_sync(&self, fd: RawFd) {
	#[allow(invalid_value)]
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
	    #[allow(invalid_value)]
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
