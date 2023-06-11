use std::fs::File;
use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;

use crate::Error;

use super::CONNECT_TIMEOUT;

struct DeviceInner {
    cuse:	Arc<File>,
    rx_hdl:	Option<JoinHandle<()>>,
    tx_hdl:	Option<JoinHandle<()>>,
    fuse_hdl:	u64,
    stream:	TcpStream,
    flags:	u32,
}

impl DeviceInner {
    fn rx_thread(self: Arc<Self>) {
    }

    fn tx_thread(self: Arc<Self>) {
    }
}

pub struct Device(Arc<DeviceInner>);

pub(super) struct OpenArgs {
    pub addr:		SocketAddr,
    pub cuse:		Arc<File>,
    pub fuse_hdl:	u64,
    pub flags:		u32,
}

impl Device {
    pub(super) fn open(args: OpenArgs) -> Result<Self, Error> {
	let conn = TcpStream::connect_timeout(&args.addr, CONNECT_TIMEOUT)?;

	let inner = Arc::new(DeviceInner {
	    cuse:	args.cuse,
	    fuse_hdl:	args.fuse_hdl,
	    flags:	args.flags,
	    stream:	conn,

	    rx_hdl:	None,
	    tx_hdl:	None,
	});

	let inner = Arc::new(RwLock::new(inner));

	// hold the write until to end; this makes sure that 'Arc::get_mut()'
	// below sees only one instance.  Threads will do a read lock and
	// start after completing the initialization.
	let mut dev = inner.write().unwrap();

	let inner_rx = inner.clone();
	let inner_tx = inner.clone();

	let rx_hdl = std::thread::spawn(move || {
	    DeviceInner::rx_thread(inner_rx.read().unwrap().clone())
	});

	let tx_hdl = std::thread::spawn(move || {
	    DeviceInner::tx_thread(inner_tx.read().unwrap().clone())
	});

	{
	    let dev = Arc::get_mut(&mut dev).unwrap();

	    dev.rx_hdl = Some(rx_hdl);
	    dev.tx_hdl = Some(tx_hdl);
	}

	Ok(Self(dev.clone()))
    }

}
