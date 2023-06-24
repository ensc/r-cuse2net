impl super::ioctl {
    pub const TCGETS: Self = Self(0x5401);
    pub const TCSETS: Self = Self(0x5402);
    pub const TCSETSW: Self = Self(0x5403);
    pub const TCSETSF: Self = Self(0x5404);
    pub const TCGETA: Self = Self(0x5405);
    pub const TCSETA: Self = Self(0x5406);
    pub const TCSETAW: Self = Self(0x5407);
    pub const TCSETAF: Self = Self(0x5408);
    pub const TCSBRK: Self = Self(0x5409);
    pub const TCXONC: Self = Self(0x540A);
    pub const TCFLSH: Self = Self(0x540B);
    pub const TIOCEXCL: Self = Self(0x540C);
    pub const TIOCNXCL: Self = Self(0x540D);
    pub const TIOCSCTTY: Self = Self(0x540E);
    pub const TIOCGPGRP: Self = Self(0x540F);
    pub const TIOCSPGRP: Self = Self(0x5410);
    pub const TIOCOUTQ: Self = Self(0x5411);
    pub const TIOCSTI: Self = Self(0x5412);
    pub const TIOCGWINSZ: Self = Self(0x5413);
    pub const TIOCSWINSZ: Self = Self(0x5414);
    pub const TIOCMGET: Self = Self(0x5415);
    pub const TIOCMBIS: Self = Self(0x5416);
    pub const TIOCMBIC: Self = Self(0x5417);
    pub const TIOCMSET: Self = Self(0x5418);
    pub const TIOCGSOFTCAR: Self = Self(0x5419);
    pub const TIOCSSOFTCAR: Self = Self(0x541A);
    pub const FIONREAD: Self = Self(0x541B);
    pub const TIOCINQ: Self = Self::FIONREAD;
    pub const TIOCLINUX: Self = Self(0x541C);
    pub const TIOCCONS: Self = Self(0x541D);
    pub const TIOCGSERIAL: Self = Self(0x541E);
    pub const TIOCSSERIAL: Self = Self(0x541F);
    pub const TIOCPKT: Self = Self(0x5420);
    pub const FIONBIO: Self = Self(0x5421);
    pub const TIOCNOTTY: Self = Self(0x5422);
    pub const TIOCSETD: Self = Self(0x5423);
    pub const TIOCGETD: Self = Self(0x5424);
    pub const TCSBRKP: Self = Self(0x5425);
    pub const TIOCSBRK: Self = Self(0x5427);
    pub const TIOCCBRK: Self = Self(0x5428);
    pub const TIOCGSID: Self = Self(0x5429);
    pub const TIOCGLCKTRMIOS: Self = Self(0x5456);
    pub const TIOCSLCKTRMIOS: Self = Self(0x5457);
    pub const TIOCSERGSTRUCT: Self = Self(0x5458);
    pub const TIOCSERGETLSR: Self = Self(0x5459);
    pub const TIOCSERGETMULTI: Self = Self(0x545A);
    pub const TIOCSERSETMULTI: Self = Self(0x545B);
    pub const TIOCMIWAIT: Self = Self(0x545C);
    pub const TIOCGICOUNT: Self = Self(0x545D);

    pub const TCGETS2:  Self = Self::IOR::<termios2>(b'T', 0x2a);
    pub const TCSETS2:  Self = Self::IOW::<termios2>(b'T', 0x2b);
    pub const TCSETSW2: Self = Self::IOW::<termios2>(b'T', 0x2c);
    pub const TCSETSF2: Self = Self::IOW::<termios2>(b'T', 0x2d);
    pub const TIOCGEXCL: Self = Self::IOR::<nix::libc::c_int>(b'T', 0x40);

}

pub type tcflag_t = nix::libc::c_uint;
pub type cc_t = nix::libc::c_uchar;
pub type speed_t = nix::libc::c_uint;

#[repr(transparent)]
pub struct c_iflag(pub tcflag_t);

impl From<tcflag_t> for c_iflag {
    fn from(value: tcflag_t) -> Self {
        Self(value)
    }
}

#[repr(transparent)]
pub struct c_oflag(pub tcflag_t);

impl From<tcflag_t> for c_oflag {
    fn from(value: tcflag_t) -> Self {
        Self(value)
    }
}

#[repr(transparent)]
pub struct c_cflag(pub tcflag_t);

impl From<tcflag_t> for c_cflag {
    fn from(value: tcflag_t) -> Self {
        Self(value)
    }
}

#[repr(transparent)]
pub struct c_lflag(pub tcflag_t);

impl From<tcflag_t> for c_lflag {
    fn from(value: tcflag_t) -> Self {
        Self(value)
    }
}

pub const NCCS: usize = 19;

#[repr(C)]
pub struct termios {
    pub c_iflag:	c_iflag,
    pub c_oflag:	c_oflag,
    pub c_cflag:	c_cflag,
    pub c_lflag:	c_lflag,
    pub c_line:		cc_t,
    pub c_cc:		[cc_t;NCCS],
}

#[repr(C)]
pub struct termio {
    pub c_iflag:	c_iflag,
    pub c_oflag:	c_oflag,
    pub c_cflag:	c_cflag,
    pub c_lflag:	c_lflag,
    pub c_line:		cc_t,
    pub c_cc:		[cc_t;NCCS],
}

#[repr(C)]
pub struct termios2 {
    pub c_iflag:	c_iflag,
    pub c_oflag:	c_oflag,
    pub c_cflag:	c_cflag,
    pub c_lflag:	c_lflag,

    pub c_line:		cc_t,
    pub c_cc:		[cc_t;NCCS],

    pub c_ispeed:	speed_t,
    pub c_ospeed:	speed_t,
}

#[repr(C)]
pub struct winsize {
    pub ws_row:		nix::libc::c_ushort,
    pub ws_col:		nix::libc::c_ushort,
    pub ws_xpixel:	nix::libc::c_ushort,
    pub ws_ypixel:	nix::libc::c_ushort,
}
