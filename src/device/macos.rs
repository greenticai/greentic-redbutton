use super::{DeviceBackend, DeviceMatcher, DeviceStream, GenericHidBackend};
use crate::event::DeviceInfo;

#[allow(dead_code)]
pub struct MacOsBackend;

impl DeviceBackend for MacOsBackend {
    fn list_devices(&self) -> anyhow::Result<Vec<DeviceInfo>> {
        GenericHidBackend::new("macos-hid").list_devices()
    }

    fn connect(&self, matcher: &DeviceMatcher) -> anyhow::Result<Box<dyn DeviceStream>> {
        GenericHidBackend::new("macos-hid").connect(matcher)
    }
}
