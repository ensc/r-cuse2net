declare_ioctls!("termios", map_termios, {
    pub TCGETS		=> BAD(0x5401),
    pub TCSETS		=> BAD(0x5402),
    pub TCSETSW		=> BAD(0x5403),
    pub TCSETSF		=> BAD(0x5404),
    pub TCGETA		=> BAD(0x5405),
    pub TCSETA		=> BAD(0x5406),
    pub TCSETAW		=> BAD(0x5407),
    pub TCSETAF		=> BAD(0x5408),
    pub TCSBRK		=> BAD(0x5409),
    pub TCXONC		=> BAD(0x540A),
    pub TCFLSH		=> BAD(0x540B),
    pub TIOCEXCL	=> BAD(0x540C),
    pub TIOCNXCL	=> BAD(0x540D),
    pub TIOCSCTTY	=> BAD(0x540E),
    pub TIOCGPGRP	=> BAD(0x540F),
    pub TIOCSPGRP	=> BAD(0x5410),
    pub TIOCOUTQ	=> BAD(0x5411),
    pub TIOCSTI		=> BAD(0x5412),
    pub TIOCGWINSZ	=> BAD(0x5413),
    pub TIOCSWINSZ	=> BAD(0x5414),
    pub TIOCMGET	=> BAD(0x5415),
    pub TIOCMBIS	=> BAD(0x5416),
    pub TIOCMBIC	=> BAD(0x5417),
    pub TIOCMSET	=> BAD(0x5418),
    pub TIOCGSOFTCAR	=> BAD(0x5419),
    pub TIOCSSOFTCAR	=> BAD(0x541A),
    pub TIOCINQ		=> BAD(0x541B),
    pub FIONREAD	=> ALIAS(Self::TIOCINQ),
    pub TIOCLINUX	=> BAD(0x541C),
    pub TIOCCONS	=> BAD(0x541D),
    pub TIOCGSERIAL	=> BAD(0x541E),
    pub TIOCSSERIAL	=> BAD(0x541F),
    pub TIOCPKT		=> BAD(0x5420),
    pub FIONBIO		=> BAD(0x5421),
    pub TIOCNOTTY	=> BAD(0x5422),
    pub TIOCSETD	=> BAD(0x5423),
    pub TIOCGETD	=> BAD(0x5424),
    pub TCSBRKP		=> BAD(0x5425),
    pub TIOCSBRK	=> BAD(0x5427),
    pub TIOCCBRK	=> BAD(0x5428),
    pub TIOCGSID	=> BAD(0x5429),
    pub TIOCGLCKTRMIOS	=> BAD(0x5456),
    pub TIOCSLCKTRMIOS	=> BAD(0x5457),
    pub TIOCSERGSTRUCT	=> BAD(0x5458),
    pub TIOCSERGETLSR	=> BAD(0x5459),
    pub TIOCSERGETMULTI	=> BAD(0x545A),
    pub TIOCSERSETMULTI	=> BAD(0x545B),
    pub TIOCMIWAIT	=> BAD(0x545C),
    pub TIOCGICOUNT	=> BAD(0x545D),

    pub TCGETS2		=> IOR(b'T', 0x2a, termios2),
    pub TCSETS2		=> IOW(b'T', 0x2b, termios2),
    pub TCSETSW2	=> IOW(b'T', 0x2c, termios2),
    pub TCSETSF2	=> IOW(b'T', 0x2d, termios2),
    pub TIOCGEXCL	=> IOR(b'T', 0x40, nix::libc::c_int),
});

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
