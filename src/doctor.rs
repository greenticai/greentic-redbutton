use anyhow::Result;

use crate::config::Config;
use crate::device::DeviceBackend;
use crate::runtime::wait_for_press;

pub struct DoctorReport {
    pub config_summary: Vec<String>,
    pub matching_devices: Vec<String>,
    pub press_result: Option<String>,
}

pub fn run(config: &Config, backend: &dyn DeviceBackend) -> Result<DoctorReport> {
    let devices = backend.list_devices()?;
    let matching_devices = devices
        .iter()
        .filter(|device| {
            device.vendor_id == config.vendor_id && device.product_id == config.product_id
        })
        .map(|device| {
            format!(
                "{:04x}:{:04x} {} via {}",
                device.vendor_id,
                device.product_id,
                device.name.as_deref().unwrap_or("unknown device"),
                device.backend
            )
        })
        .collect::<Vec<_>>();

    let press_result = wait_for_press(config, backend)?
        .map(|(name, timestamp)| format!("press detected from {name} at {timestamp}"));

    Ok(DoctorReport {
        config_summary: vec![
            format!("vendor_id={}", config.vendor_id),
            format!("product_id={}", config.product_id),
            format!("key={}", config.key.as_config_value()),
            format!("webhook_url={}", config.webhook_url),
            format!("timeout_ms={}", config.timeout_ms),
        ],
        matching_devices,
        press_result,
    })
}
