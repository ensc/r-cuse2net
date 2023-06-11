use std::thread::JoinHandle;

use crate::Error;

pub struct DeviceOpen {
    pub(super) hdl:	JoinHandle<Result<(), Error>>,
}

impl DeviceOpen {
}
