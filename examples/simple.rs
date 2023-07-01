//

use std::{os::{unix::prelude::OpenOptionsExt, fd::{RawFd, AsRawFd}}, io::{Write, Read}, time::Duration};

use nix::poll::{PollFd, PollFlags};


const RX_TIMEOUT: Duration = Duration::from_secs(5);

fn set_termios_raw(fd: RawFd) -> r_ser2net::Result<()> {
    use nix::sys::termios;

    let mut ios = termios::tcgetattr(fd)?;

    termios::cfmakeraw(&mut ios);
    termios::cfsetspeed(&mut ios, termios::BaudRate::B2400)?;

    termios::tcsetattr(fd, termios::SetArg::TCSANOW, &ios)?;

    Ok(())
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
		let mut pfd = [
		    PollFd::new(fd, PollFlags::POLLIN)
		];
		nix::poll::poll(&mut pfd, RX_TIMEOUT.as_millis() as i32)?;
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
	.open(&dev_cuse)?;

    let mut f_ser = std::fs::File::options()
	.read(true)
	.write(true)
	.custom_flags(nix::libc::O_NONBLOCK)
	.open(&dev_ser)?;

    print!("setting termios...");
    set_termios_raw(f_ser.as_raw_fd())?;
    set_termios_raw(f_cuse.as_raw_fd())?;
    println!(" ok");


    {
	print!("write(cuse -> ser");
	f_cuse.write_all(b"test")?;
	let tmp = read_all(&f_ser, 4)?;
	assert_eq!(&tmp, b"test");
    }

    {
	print!("write(ser -> test");
	f_ser.write_all(b"test")?;
	let tmp = read_all(&f_cuse, 4)?;
	assert_eq!(&tmp, b"test");
    }


    todo!()
}