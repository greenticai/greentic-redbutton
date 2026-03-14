use super::{DeviceBackend, DeviceMatcher, DeviceStream, GenericHidBackend};
use crate::event::DeviceInfo;

#[allow(dead_code)]
pub struct LinuxBackend;

impl DeviceBackend for LinuxBackend {
    fn list_devices(&self) -> anyhow::Result<Vec<DeviceInfo>> {
        GenericHidBackend::new("linux-hid").list_devices()
    }

    fn connect(&self, matcher: &DeviceMatcher) -> anyhow::Result<Box<dyn DeviceStream>> {
        GenericHidBackend::new("linux-hid").connect(matcher)
    }
}
