#[path = "ioctl_serial.rs"]
mod serial;

use super::{ Error, Result };

pub use serial::*;
