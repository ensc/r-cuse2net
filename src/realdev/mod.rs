#![allow(unused_variables)]
//

use std::mem::MaybeUninit;
use std::os::fd::{OwnedFd, FromRawFd, AsRawFd};
use std::path::Path;
use std::net::TcpStream;

use crate::proto::ioctl::TermIOs;
use crate::proto::{self, Sequence};

pub struct Device {
    fd:		OwnedFd,
    conn:	TcpStream,
    flags:	u32,
}

impl Device {
    pub fn open<P: AsRef<Path>>(p: P, seq: Sequence, flags: u32, conn: TcpStream) -> crate::Result<Self> {
	use nix::fcntl::OFlag;
	use nix::sys::stat::Mode;

	let p = p.as_ref();

	let fd = nix::fcntl::open(
	    p,
	    OFlag::O_CLOEXEC | OFlag::O_NONBLOCK | OFlag::O_NOCTTY | OFlag::O_RDWR,
	    Mode::empty());

	let fd = match fd {
	    Ok(fd)	=> unsafe { OwnedFd::from_raw_fd(fd) },
	    Err(e)	=> {
		error!("failed to open {p:?}: {e:?}");
		proto::Response::send_err(&conn, seq, e as i32)?;
		return Err(e.into());
	    }
	};

	seq.send_ok(&conn)?;

	Ok(Self {
	    fd:		fd,
	    conn:	conn,
	    flags:	flags,
	})
    }

    pub fn run(self) -> crate::Result<()> {
	debug!("running device");

	let mut buf: [MaybeUninit<u8>; proto::MAX_MSG_SIZE] = [MaybeUninit::uninit(); proto::MAX_MSG_SIZE];

	loop {
	    let op = proto::Request::recv(&self.conn, &mut buf)?;

	    debug!("got {op:?}");

	    match op {
		proto::Request::Open(_, seq) => {
		    warn!("can not open an already opened device");
		    seq.send_err(&self.conn, nix::libc::EINVAL)?;
		}

		proto::Request::Release(seq) => {
		    seq.send_ok(&self.conn)?;
		    break Ok(());
		}

		proto::Request::Write(seq, wrinfo, data)	=> {
		    self.write(seq, wrinfo.offset.into(), data)?;
		}

		proto::Request::Ioctl(seq, ioinfo, data)	=> {
		    self.ioctl(seq, ioinfo.cmd.into(), ioinfo.arg.into(), data)?;
		},

		proto::Request::IoctlTermiosGet(seq)		=> {
		    self.ioctl_termios_get(seq)?;
		}

		proto::Request::IoctlTermiosSet(seq, cmd, ios)	=> todo!(),
	    }
	}
    }

    fn write(&self, seq: Sequence, offset: u64, data: &[u8]) -> crate::Result<()> {
	trace!("write#{seq:?}@{offset}: {data:?}");

	// TODO: use only write() and required that 'offset' is zero?  write()
	// and pwrite() have different semantics regarding file position after
	// the call
	let l = match offset {
	    0		=> nix::unistd::write(self.fd.as_raw_fd(), data),
	    offs	=> nix::sys::uio::pwrite(self.fd.as_raw_fd(), data, offs as nix::libc::off_t),
	};

	match l {
	    Ok(l)	=> proto::Response::send_write(&self.conn, seq, l as u32),
	    Err(e)	=> proto::Response::send_err(&self.conn, seq, e as i32),
	}?;

	Ok(())
    }

    fn ioctl(&self, seq: Sequence, cmd: u32, arg: u64, data: &[u8]) -> crate::Result<()> {
	todo!()
    }

    fn ioctl_termios_get(&self, seq: Sequence) -> crate::Result<()> {
	let mut ios: MaybeUninit<nix::libc::termios2> = MaybeUninit::uninit();
	let rc = unsafe {
	    nix::libc::ioctl(self.fd.as_raw_fd(), nix::libc::TCGETS2, ios.as_mut_ptr())
	};

	if rc < 0 {
	    return Err(nix::Error::from_i32(rc).into());
	}

	let ios = unsafe {
	    ios.assume_init();
	};

	proto::Response::send_ioctl_termios(&self.conn, seq, TermIOs::try_from_os2(&ios))?;

	Ok(())
    }
}
