mod read;
mod poll;

use std::mem::MaybeUninit;
use std::os::fd::{OwnedFd, FromRawFd, AsRawFd};
use std::path::Path;
use std::net::TcpStream;
use std::thread::scope;

use nix::fcntl::OFlag;

use crate::proto::ioctl::Arg;
use crate::proto::{self, Sequence};

pub struct Device {
    fd:		OwnedFd,
    conn:	TcpStream,
}

impl Device {
    pub fn open<P: AsRef<Path>>(p: P, seq: Sequence, flags: OFlag, conn: TcpStream) -> crate::Result<Self> {
	use nix::sys::stat::Mode;

	let p = p.as_ref();

	let fd = nix::fcntl::open(
	    p,
	    OFlag::O_CLOEXEC | OFlag::O_NONBLOCK | OFlag::O_NOCTTY | flags,
	    Mode::empty());

	let fd = match fd {
	    Ok(fd)	=> unsafe { OwnedFd::from_raw_fd(fd) },
	    Err(e)	=> {
		error!("failed to open {p:?}: {e:?}");
		proto::Response::send_err(&conn, seq, e)?;
		return Err(e.into());
	    }
	};

	seq.send_ok(&conn)?;

	Ok(Self {
	    fd:		fd,
	    conn:	conn,
	})
    }

    pub fn run(self) -> crate::Result<()> {
	debug!("running device");

	let read = read::Read::new(&self)?;
	let poll = poll::Poll::new(&self)?;

	scope(|s| {
	    std::thread::Builder::new()
		.name("read".to_string())
		.spawn_scoped(s, || read.run())?;

	    std::thread::Builder::new()
		.name("poll".to_string())
		.spawn_scoped(s, || poll.run())?;

	    self.main(&read, &poll)
	})
    }

    fn main(&self, read: &read::Read, poll: &poll::Poll) -> crate::Result<()> {
	debug!("running device");

	let mut buf: [MaybeUninit<u8>; proto::MAX_MSG_SIZE] = [MaybeUninit::uninit(); proto::MAX_MSG_SIZE];

	loop {
	    let op = proto::Request::recv(&self.conn, &mut buf)?;

	    debug!("got {op:?}");

	    match op {
		proto::Request::Open(seq, _) => {
		    warn!("can not open an already opened device");
		    seq.send_err(&self.conn, nix::Error::EINVAL)?;
		}

		proto::Request::Release(seq) => {
		    seq.send_ok(&self.conn)?;
		    break Ok(());
		}

		proto::Request::Write(seq, wrinfo, data)	=> {
		    self.write(seq, wrinfo, data)?;
		}

		proto::Request::Read(seq, rdinfo)	=> {
		    self.read(read, seq, rdinfo)?;
		}

		proto::Request::Ioctl(seq, ioinfo, arg)	=> {
		    self.ioctl(seq, ioinfo.cmd.into(), arg)?;
		},

		proto::Request::Poll(seq, parm)		=> {
		    self.poll(poll, seq, parm.kh.into(), parm.flags.into(), parm.events.into())?;
		}

		proto::Request::Interrupt(seq) => {
		    read.do_intr(Some(seq));
		}
	    }
	}
    }

    fn read(&self, read: &read::Read, seq: Sequence, rdinfo: proto::request::Read)
	    -> crate::Result<()>
    {
	trace!("read#{seq:?}@{rdinfo:?}");

	let req = (seq, rdinfo.size.as_native() as usize);

	match rdinfo.fh_flags.is_nonblock() {
	    true	=> read.read_nonblock(req),
	    false	=> read.push_request(req),
	}

	Ok(())
    }

    fn write(&self, seq: Sequence, wrinfo: proto::request::Write, data: &[u8]) -> crate::Result<()> {
	trace!("write({seq:?}, {wrinfo:?}, #{})", data.len());

	// TODO: use only write() and required that 'offset' is zero?  write()
	// and pwrite() have different semantics regarding file position after
	// the call
	let l = match wrinfo.offset.into() {
	    0		=> nix::unistd::write(self.fd.as_raw_fd(), data),
	    offs	=> nix::sys::uio::pwrite(self.fd.as_raw_fd(), data, offs as nix::libc::off_t),
	};

	match l {
	    Ok(l)	=> proto::Response::send_write(&self.conn, seq, l as u32),
	    Err(e)	=> proto::Response::send_err(&self.conn, seq, e),
	}?;

	Ok(())
    }

    fn ioctl(&self, seq: Sequence, cmd: u32, arg: Arg) -> crate::Result<()> {
	trace!("ioctl({seq:?}, {cmd:x}, {arg:?})");

	let (cmd, arg, buf) = arg.encode(cmd)?;

	let rc = unsafe {
	    nix::libc::ioctl(self.fd.as_raw_fd(), cmd as u64, arg)
	};

	if rc < 0 {
	    warn!("ioctl ({cmd:x}, {arg:?}) failed: {rc}");
	    proto::Response::send_err(&self.conn, seq, nix::Error::last())
	} else {
	    let res_arg = Arg::decode(cmd, arg, &buf, proto::ioctl::Source::Device)?;

	    proto::Response::send_ioctl(&self.conn, seq, rc as u64, res_arg)
	}?;

	Ok(())
    }

    fn poll(&self, poll: &poll::Poll, seq: Sequence, kh: u64, flags: u32, events: u32) -> crate::Result<()> {
	trace!("poll({seq:?}, {kh}, {flags:x}, {events:?})");

	match flags & proto::request::Poll::FLAG_SCHEDULE_NOTIFY {
	    0	=> poll.poll_once((seq, events)),
	    _	=> poll.poll((seq, kh, events)),
	}

	Ok(())
    }
}
