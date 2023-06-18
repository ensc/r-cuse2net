use std::{os::fd::{AsFd, AsRawFd, RawFd}, mem::MaybeUninit, time::Duration, io::IoSlice};

use nix::sys::socket::{MsgFlags, SockaddrStorage};

use super::{AsReprBytesMut, TIMEOUT_READ};

fn wait_read(fd: RawFd, d: Duration) -> std::io::Result<Duration>
{
    use nix::sys::select;

    let mut fds = select::FdSet::new();
    let mut timeout = nix::sys::time::TimeVal::new(d.as_secs() as nix::sys::time::time_t,
						   (d.as_micros() % 1_000_000) as nix::sys::time::suseconds_t);

    fds.insert(fd);

    select::select(fd + 1, Some(&mut fds), None, None, Some(&mut timeout))?;

    match fds.contains(fd) {
	true	=> Ok(Duration::from_secs(timeout.tv_sec() as u64) +
		      Duration::from_micros(timeout.tv_usec() as u64)),
	false	=> Err(nix::Error::ETIMEDOUT.into()),
    }
}

fn recv_timeout(fd: RawFd, buf: &mut [u8], d: Duration) -> std::io::Result<usize>
{
    use nix::sys::socket;
    use nix::Error as NixError;

    assert_ne!(buf.len(), 0);

    let l = match socket::recv(fd, buf, MsgFlags::MSG_DONTWAIT) {
	Err(e) if e == NixError::EAGAIN	=> {
	    wait_read(fd, d)?;
	    0
	},
	Ok(0)		=> return Err(NixError::EPIPE.into()),
	Ok(l)		=> l,
	Err(e)		=> return Err(e.into()),
    };

    Ok(l)
}

fn recv_exact_timeout_internal(fd: RawFd, buf: &mut [MaybeUninit<u8>],
			       to_initial: Option<Duration>,
			       to_cont: Option<Duration>) -> std::io::Result<&[u8]>
{
    use nix::sys::socket;
    use nix::Error as NixError;

    let buf: &mut [u8] = unsafe {
	core::mem::transmute(buf)
    };
    let mut len = buf.len();
    let mut pos = 0;

    if to_initial.is_none() && to_cont.is_none() {
	socket::recv(fd, buf, MsgFlags::MSG_WAITALL)?;

	return Ok(buf);
    }

    if len > 0 {
	match to_initial {
	    Some(d)	=> {
		let l = recv_timeout(fd, &mut buf[pos..], d)?;

		assert!(l <= len);

		pos += l;
		len -= l;
	    },

	    None	=> {
		let l = socket::recv(fd, &mut buf[pos..], MsgFlags::empty())?;

		if l == 0 {
		    // eof
		    return Err(NixError::EPIPE.into());
		}

		assert!(l <= len);

		pos += l;
		len -= l;
	    },
	}
    }

    match to_cont {
	None	=> {
	    socket::recv(fd, &mut buf[pos..], MsgFlags::MSG_WAITALL)?;
	}

	Some(d)	=> {
	    while len > 0 {
		let l = recv_timeout(fd, &mut buf[pos..], d)?;

		assert!(l <= len);

		pos += l;
		len -= l;
	    }
	}
    }

    Ok(buf)
}

pub fn recv_exact_timeout<'a, R, B>(fd: R, buf: &'a mut MaybeUninit<B>,
				    to_initial: Option<Duration>, to_cont: Option<Duration>)
				    -> std::io::Result<&'a B>
where
    R: AsFd,
    B: super::AsReprBytesMut,
{
    let fd = fd.as_fd().as_raw_fd();

    let buf_bytes = (buf as &mut dyn super::AsReprBytesMut).as_repr_bytes_mut();
    let buf_bytes = unsafe {
	core::mem::transmute(buf_bytes)
    };

    let res = recv_exact_timeout_internal(fd, buf_bytes, to_initial, to_cont)?;

    (buf as &mut dyn super::AsReprBytesMut).update_repr(res);

    Ok(unsafe { buf.assume_init_ref() })
}

pub fn recv_to<R, B>(fd: R, mut buf: MaybeUninit<B>) -> std::io::Result<B>
where
    R: AsFd,
    B: AsReprBytesMut + Sized,
{
    recv_exact_timeout(fd, &mut buf, Some(TIMEOUT_READ), Some(TIMEOUT_READ))?;

    Ok(unsafe { buf.assume_init() })
}

pub fn send_vectored<W: AsFd + std::io::Write>(w: W, b: &[IoSlice]) -> std::io::Result<usize>
{
    use nix::sys::socket;

    let fd = w.as_fd().as_raw_fd();

    let len = socket::sendmsg(fd, b, &[], MsgFlags::MSG_NOSIGNAL,
			      Option::<SockaddrStorage>::None.as_ref())?;

    Ok(len)
}

pub fn send_vectored_all<W: AsFd + std::io::Write>(mut w: W, b: &[IoSlice]) -> std::io::Result<()>
{
    let mut len = b.iter().fold(0, |acc, b| acc + b.len());

    while len > 0 {
	match send_vectored(&mut w, b) {
	    Ok(l) if l == len		=> {
		len -= l;
	    },

	    Ok(_)			=>
		unimplemented!("incomplete vectored send not implemented"),
	    Err(e)			=> return Err(e),
	}
    }

    Ok(())
}

pub fn send_all<W: AsFd + std::io::Write>(w: W, b: &[u8]) -> std::io::Result<()>
{
    use nix::sys::socket;

    let fd = w.as_fd().as_raw_fd();

    let mut len = b.len();
    let mut pos = 0;

    while len > 0 {
	let l = socket::send(fd, &b[pos..], MsgFlags::MSG_NOSIGNAL)?;

	assert_ne!(l, 0);
	assert!(l <= len);

	len -= l;
	pos += l;
    }

    Ok(())
}
