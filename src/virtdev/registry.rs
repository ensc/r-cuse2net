use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use ensc_cuse_ffi::{AsBytes, OpInInfo, OpenParams};
use ensc_cuse_ffi::ffi as cuse_ffi;

use parking_lot::RwLock;

use crate::error::Error;
use crate::CuseFileDevice;

use super::{ DeviceState, DeviceOpen, Device };

pub struct DeviceRegistryInner {
    dev_hdl:	AtomicU64,
    devices:	HashMap<cuse_ffi::fh_t, DeviceState>,
    cuse:	Arc<CuseFileDevice>,
}

impl DeviceRegistryInner {
    pub fn get_cuse(&self) -> &CuseFileDevice {
	self.cuse.as_ref()
    }
}

#[derive(Clone)]
pub struct DeviceRegistry(Arc<RwLock<DeviceRegistryInner>>);

impl std::ops::Deref for DeviceRegistry {
    type Target = RwLock<DeviceRegistryInner>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

struct ManagedHdl<'a> {
    registry:	&'a DeviceRegistry,
    hdl:	Option<cuse_ffi::fh_t>,
}

impl ManagedHdl<'_> {
    pub fn commit(mut self, state: DeviceState) {
	let hdl = self.hdl.take().unwrap();

	self.registry.write()
	    .devices
	    .insert(hdl, state);
    }
}

impl std::ops::Drop for ManagedHdl<'_> {
    fn drop(&mut self) {
        if let Some(hdl) = self.hdl {
	    self.registry.write()
		.devices
		.remove(&hdl);
	}
    }
}

impl DeviceRegistry {
    fn new_managed_hdl(&self, hdl: cuse_ffi::fh_t) -> ManagedHdl {
	ManagedHdl {
	    registry:	self,
	    hdl:	Some(hdl),
	}
    }

    pub fn new(cuse: Arc<CuseFileDevice>) -> Self {
	Self(Arc::new(RwLock::new(DeviceRegistryInner {
	    dev_hdl:	AtomicU64::new(1),
	    devices:	HashMap::new(),
	    cuse:	cuse,
	})))
    }

    pub fn interrupt(&self, info: OpInInfo, unique: cuse_ffi::unique_t) {
	let this = self.0.write();

	for dev in this.devices.values() {
	    match dev {
		DeviceState::Opening(_)		=> {}, // noop
		DeviceState::Running(dev)	=> dev.try_interrupt(info.clone(), unique),
	    }
	}
    }

    pub fn for_fh<F: FnOnce(&Device)>(&self, fh: cuse_ffi::fh_t, func: F) {
	match self.read().devices.get(&fh) {
	    None	=>
		warn!("no such device {fh:?}"),

	    Some(DeviceState::Opening(_))	=>
		warn!("device {fh:?} not ready yet"),

	    Some(DeviceState::Running(d))	=>
		func(d),
	}
    }

    pub fn release(&self, fh: cuse_ffi::fh_t, info: OpInInfo) {
	let dev = {
	    let mut reg = self.write();

	    reg.devices.remove(&fh)
	};

	match dev {
	    Some(DeviceState::Running(dev))	=> dev.release(info),
	    _					=> warn!("no such device with fh {fh:?}"),
	}
    }

    pub fn create(&self, addr: SocketAddr, op_info: OpInInfo, params: OpenParams) -> Result<(), Error>
    {
	let registry = self.clone();

	// lock the registry so that thread sees a 'DeviceOpen' in the hash map
	let mut reg = self.write();

	let dev_hdl = reg.dev_hdl.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
	let dev_hdl = cuse_ffi::fh_t::from_ffi(dev_hdl);

	assert!(!reg.devices.contains_key(&dev_hdl));

	std::thread::Builder::new()
	    .name("open".to_string())
	    .spawn(move || -> Result<(), Error> {
		let cuse = registry.read().cuse.clone();
		let mngd_hdl = registry.new_managed_hdl(dev_hdl);

		let args = super::device::OpenArgs {
		    addr:		addr,
		    cuse:		cuse.clone(),
		    flags:		params.flags,
		};

		match Device::open(args) {
		    Ok(dev)		=> {
			let hdr = cuse_ffi::fuse_open_out {
			    fh:		dev_hdl,
			    open_flags:	params.open_flags,
			    _padding:	Default::default(),
			};

			op_info.send_response(&cuse, &[
			    hdr.as_bytes()
			])?;

			mngd_hdl.commit(dev.into());

			Ok(())
		    },

		    Err(e)		=> {
			error!("failed to open device: {e:?}");

			drop(mngd_hdl);

			let _ = op_info.send_error(&cuse, nix::Error::EIO);

			Err(e)
		    }
		}
	    })?;

	reg.devices.insert(dev_hdl, DeviceOpen {}.into());

	drop(reg);

	Ok(())
    }
}
