use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::time::Duration;

use nix::sys::socket::{MsgFlags, SockaddrStorage};

use super::{AsReprBytesMut, TIMEOUT_READ};

fn wait_read(fd: BorrowedFd, d: Duration) -> std::io::Result<Duration>
{
    use nix::sys::select;

    let (d_s, d_ms) = (d.as_secs() as nix::sys::time::time_t,
		       (d.as_micros() % 1_000_000) as nix::sys::time::suseconds_t);

    let mut fds = select::FdSet::new();
    let mut timeout = nix::sys::time::TimeVal::new(d_s, d_ms);

    fds.insert(&fd);

    select::select(None, Some(&mut fds), None, None, Some(&mut timeout))?;

    match fds.contains(&fd) {
	true	=> Ok(Duration::from_secs(timeout.tv_sec() as u64) +
		      Duration::from_micros(timeout.tv_usec() as u64)),
	false	=> Err(nix::Error::ETIMEDOUT.into()),
    }
}

fn recv_timeout(fd: BorrowedFd, buf: &mut [u8], d: Duration) -> std::io::Result<usize>
{
    use nix::sys::socket;
    use nix::Error as NixError;

    assert_ne!(buf.len(), 0);

    let l = match socket::recv(fd.as_raw_fd(), buf, MsgFlags::MSG_DONTWAIT) {
	Err(NixError::EAGAIN)		=> {
	    wait_read(fd, d)?;
	    0
	},
	Ok(0)		=> return Err(NixError::EPIPE.into()),
	Ok(l)		=> l,
	Err(e)		=> return Err(e.into()),
    };

    Ok(l)
}

fn recv_exact_timeout_internal<'a>(fd: BorrowedFd, buf: &'a mut [MaybeUninit<u8>],
				   to_initial: Option<Duration>,
				   to_cont: Option<Duration>) -> std::io::Result<&'a [u8]>
{
    use nix::sys::socket;
    use nix::Error as NixError;

    let buf: &mut [u8] = unsafe {
	core::mem::transmute(buf)
    };
    let mut len = buf.len();
    let mut pos = 0;

    if to_initial.is_none() && to_cont.is_none() {
	socket::recv(fd.as_fd().as_raw_fd(), buf, MsgFlags::MSG_WAITALL)?;

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
		let l = socket::recv(fd.as_fd().as_raw_fd(), &mut buf[pos..], MsgFlags::empty())?;

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
	    socket::recv(fd.as_raw_fd(), &mut buf[pos..], MsgFlags::MSG_WAITALL)?;
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
				    len_avail: &mut Option<usize>,
				    to_initial: Option<Duration>, to_cont: Option<Duration>)
				    -> std::io::Result<&'a B>
where
    R: AsFd,
    B: super::AsReprBytesMut,
{
    let buf_bytes = (buf as &mut dyn super::AsReprBytesMut).as_repr_bytes_mut();
    let buf_bytes: &mut [MaybeUninit<u8>] = unsafe {
	core::mem::transmute(buf_bytes)
    };
    let buf_len = buf_bytes.len();

    let new_avail = match len_avail {
	Some(l)	if *l < buf_len	=> {
	    warn!("not enough space for rx; {l} < {}", buf_bytes.len());
	    return Err(nix::Error::EPROTO.into())
	}

	Some(l)			=> Some(*l - buf_len),
	None			=> None,
    };

    let res = recv_exact_timeout_internal(fd.as_fd(), buf_bytes, to_initial, to_cont)?;

    *len_avail = new_avail;

    (buf as &mut dyn super::AsReprBytesMut).update_repr(res);

    Ok(unsafe { buf.assume_init_ref() })
}

pub fn recv_to<R, B>(fd: R, mut buf: MaybeUninit<B>, len_avail: &mut Option<usize>) -> std::io::Result<B>
where
    R: AsFd,
    B: AsReprBytesMut + Sized,
{
    recv_exact_timeout(fd, &mut buf, len_avail, Some(TIMEOUT_READ), Some(TIMEOUT_READ))?;

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
