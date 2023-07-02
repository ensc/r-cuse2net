use std::io::Write;
use std::os::fd::{RawFd, AsRawFd};
use std::os::unix::prelude::OpenOptionsExt;
use std::time::Duration;

use nix::poll::{PollFd, PollFlags};
use nix::sys::termios::{self, FlushArg};

const RX_TIMEOUT: Duration = Duration::from_secs(5);

fn set_termios_raw(fd: RawFd) -> r_ser2net::Result<()> {
    let mut ios = termios::tcgetattr(fd)?;

    termios::cfmakeraw(&mut ios);
    termios::cfsetspeed(&mut ios, termios::BaudRate::B2400)?;

    termios::tcsetattr(fd, termios::SetArg::TCSANOW, &ios)?;

    Ok(())
}

fn poll_to<F: AsRawFd>(fd: &F, flags: PollFlags, to_ms: i32) -> nix::Result<bool> {
    let fd = fd.as_raw_fd();
    let mut pfd = [
	PollFd::new(fd, PollFlags::POLLIN)
    ];

    nix::poll::poll(&mut pfd, to_ms)?;

    Ok(pfd[0].revents().unwrap().intersects(flags))
}

fn poll<F: AsRawFd>(fd: &F, flags: PollFlags) -> nix::Result<bool> {
    poll_to(fd, flags, RX_TIMEOUT.as_millis() as i32)
}

fn read_all<F: AsRawFd>(fd: &F, mut cnt: usize) -> nix::Result<Vec<u8>> {
    let fd = fd.as_raw_fd();
    let mut res = Vec::<u8>::with_capacity(cnt);

    let buf = unsafe {
	core::slice::from_raw_parts_mut(res.as_mut_ptr() as * mut u8, cnt)
    };

    let mut pos = 0;

    while cnt > 0 {
	match nix::unistd::read(fd, &mut buf[pos..]) {
	    Ok(0)	=> return Err(nix::Error::ENXIO),
	    Ok(l)	=> {
		assert!(l <= cnt);
		pos += l;
		cnt -= l;
	    }
	    Err(e) if e == nix::Error::EAGAIN	=> {
		poll(&fd, PollFlags::POLLIN)?;
	    }

	    Err(e)	=> return Err(e)
	}
    }

    unsafe {
	res.set_len(pos)
    }

    Ok(res)
}


fn main() -> r_ser2net::Result<()> {
    let mut args = std::env::args();
    let _ = args.next();
    let dev_cuse = args.next().expect("missing cuse device");
    let dev_ser  = args.next().expect("missing serial device");

    let mut f_cuse = std::fs::File::options()
	.read(true)
	.write(true)
	.custom_flags(nix::libc::O_NONBLOCK)
	.open(dev_cuse)?;

    let mut f_ser = std::fs::File::options()
	.read(true)
	.write(true)
	.custom_flags(nix::libc::O_NONBLOCK)
	.open(dev_ser)?;

    print!("setting termios...");
    set_termios_raw(f_ser.as_raw_fd())?;
    set_termios_raw(f_cuse.as_raw_fd())?;
    println!(" ok");


    {
	print!("write(cuse -> ser");
	f_cuse.write_all(b"test")?;
	let tmp = read_all(&f_ser, 4)?;
	assert_eq!(&tmp, b"test");
	println!(" ok");
    }

    {
	print!("write(ser -> test");
	f_ser.write_all(b"test")?;
	let tmp = read_all(&f_cuse, 4)?;
	assert_eq!(&tmp, b"test");
	println!(" ok");
    }

    {
	print!("write(ser -> test (poll)");
	f_ser.write_all(b"TEST")?;
	assert_eq!(poll(&f_cuse, PollFlags::POLLIN), Ok(true));
	let tmp = read_all(&f_cuse, 4)?;
	assert_eq!(&tmp, b"TEST");
	assert_eq!(poll_to(&f_cuse, PollFlags::POLLIN, 100), Ok(false));
	println!(" ok");
    }

    {
	termios::tcsendbreak(f_cuse.as_raw_fd(), 1000)?;
	let _ = read_all(&f_ser, 1)?;
    }

    {
	termios::tcflush(f_cuse.as_raw_fd(), FlushArg::TCIFLUSH)?;
	termios::tcflush(f_cuse.as_raw_fd(), FlushArg::TCOFLUSH)?;
    }

    {
	let mut tmp: nix::libc::c_int = 0;

	let rc = unsafe {
	    nix::libc::ioctl(f_cuse.as_raw_fd(), nix::libc::TIOCINQ, &mut tmp as * mut _)
	};
	assert_eq!(rc, 0);

	let rc = unsafe {
	    nix::libc::ioctl(f_cuse.as_raw_fd(), nix::libc::TIOCMGET, &mut tmp as * mut _)
	};
	assert_eq!(rc, 0);
    }

    Ok(())
}
