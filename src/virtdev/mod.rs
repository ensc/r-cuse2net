//

use std::time::Duration;

mod registry;
mod registry_element;

pub mod device;
mod device_open;

pub mod ioctl;

pub use registry::DeviceRegistry;
use registry_element::DeviceState;
use device::Device;
use device_open::DeviceOpen;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
