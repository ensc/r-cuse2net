//

use std::os::fd::{OwnedFd, FromRawFd};
use std::path::Path;
use std::net::TcpStream;

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
	loop {
	    let op = proto::Request::recv(&self.conn)?;

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
	    }
	}
    }
}
