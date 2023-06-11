use super::{ DeviceOpen, Device };

pub enum DeviceState {
    Opening(DeviceOpen),
    Running(Device),
}

impl From<DeviceOpen> for DeviceState {
    fn from(value: DeviceOpen) -> Self {
        Self::Opening(value)
    }
}

impl From<Device> for DeviceState {
    fn from(value: Device) -> Self {
        Self::Running(value)
    }
}
