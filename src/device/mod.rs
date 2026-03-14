use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use hidapi::{DeviceInfo as HidDeviceInfo, HidApi, HidDevice};
use std::time::{Duration, Instant};

use crate::constants::READ_TIMEOUT_MS;
use crate::event::{ButtonEvent, ButtonEventKind, DeviceInfo, DeviceMatcher};

const PRESS_DEBOUNCE_MS: u64 = 120;

pub trait DeviceBackend {
    fn list_devices(&self) -> Result<Vec<DeviceInfo>>;
    fn connect(&self, matcher: &DeviceMatcher) -> Result<Box<dyn DeviceStream>>;
}

pub trait DeviceStream: Send {
    fn device_info(&self) -> &DeviceInfo;
    fn next_event(&mut self) -> Result<ButtonEvent>;
}

pub fn default_backend() -> Box<dyn DeviceBackend> {
    #[cfg(target_os = "linux")]
    {
        return Box::new(linux::LinuxBackend);
    }
    #[cfg(target_os = "macos")]
    {
        return Box::new(macos::MacOsBackend);
    }
    #[cfg(target_os = "windows")]
    {
        return Box::new(windows::WindowsBackend);
    }
    #[allow(unreachable_code)]
    Box::new(GenericHidBackend::new("hid"))
}

pub struct GenericHidBackend {
    backend_name: &'static str,
}

impl GenericHidBackend {
    pub const fn new(backend_name: &'static str) -> Self {
        Self { backend_name }
    }
}

impl DeviceBackend for GenericHidBackend {
    fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
        let api = create_hid_api()?;
        Ok(api
            .device_list()
            .map(|info| map_device_info(info, self.backend_name))
            .collect())
    }

    fn connect(&self, matcher: &DeviceMatcher) -> Result<Box<dyn DeviceStream>> {
        let api = create_hid_api()?;
        let info = api
            .device_list()
            .find(|info| {
                info.vendor_id() == matcher.vendor_id && info.product_id() == matcher.product_id
            })
            .ok_or_else(|| {
                anyhow!(
                    "device {:04x}:{:04x} not found",
                    matcher.vendor_id,
                    matcher.product_id
                )
            })?;

        let usage_id = matcher.key.usage_id().ok_or_else(|| {
            anyhow!(
                "unsupported key mapping `{}`; use a known key name or HID usage code like 0x28",
                matcher.key.as_config_value()
            )
        })?;

        let path = info.path().to_owned();
        let device = api
            .open_path(&path)
            .with_context(|| format!("failed to open HID path for {}", describe(info)))?;
        let device_info = map_device_info(info, self.backend_name);

        Ok(Box::new(HidButtonStream {
            device,
            usage_id,
            last_report: Vec::new(),
            last_pressed: false,
            last_down_at: None,
            device_info,
        }))
    }
}

struct HidButtonStream {
    device: HidDevice,
    usage_id: u8,
    last_report: Vec<u8>,
    last_pressed: bool,
    last_down_at: Option<Instant>,
    device_info: DeviceInfo,
}

impl DeviceStream for HidButtonStream {
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    fn next_event(&mut self) -> Result<ButtonEvent> {
        let mut report = [0_u8; 64];

        loop {
            let read = self
                .device
                .read_timeout(&mut report, READ_TIMEOUT_MS)
                .context("failed to read HID report")?;
            if read == 0 {
                continue;
            }

            let current_report = report[..read].to_vec();
            let pressed = report_indicates_press(&current_report, self.usage_id);
            let should_emit_down = should_emit_down(pressed, self.last_pressed, self.last_down_at);
            let report_changed = current_report != self.last_report;

            if !report_changed && !should_emit_down {
                continue;
            }

            self.last_report = current_report.clone();
            self.last_pressed = pressed;
            if should_emit_down {
                self.last_down_at = Some(Instant::now());
            }
            return Ok(ButtonEvent {
                kind: if should_emit_down {
                    ButtonEventKind::Down
                } else {
                    ButtonEventKind::Up
                },
                timestamp: Utc::now(),
            });
        }
    }
}

fn map_device_info(info: &HidDeviceInfo, backend_name: &'static str) -> DeviceInfo {
    DeviceInfo {
        name: info.product_string().map(str::to_string),
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        backend: backend_name,
    }
}

fn describe(info: &HidDeviceInfo) -> String {
    format!(
        "{:04x}:{:04x} {}",
        info.vendor_id(),
        info.product_id(),
        info.product_string().unwrap_or("unknown device")
    )
}

fn report_contains_usage(report: &[u8], usage_id: u8) -> bool {
    keyboard_slots(report).contains(&usage_id)
}

fn report_indicates_press(report: &[u8], usage_id: u8) -> bool {
    report_contains_usage(report, usage_id) || report_payload(report).iter().any(|byte| *byte != 0)
}

fn keyboard_slots(report: &[u8]) -> Vec<u8> {
    if report.len() >= 9 {
        let with_report_id = &report[3..report.len().min(9)];
        if with_report_id.iter().any(|slot| *slot != 0) {
            return with_report_id.to_vec();
        }
    }
    if report.len() >= 8 {
        return report[2..report.len().min(8)].to_vec();
    }
    report.to_vec()
}

fn report_payload(report: &[u8]) -> &[u8] {
    if report.len() > 3 {
        &report[3..]
    } else {
        report
    }
}

fn should_emit_down(pressed: bool, last_pressed: bool, last_down_at: Option<Instant>) -> bool {
    pressed
        && (!last_pressed
            || last_down_at
                .is_none_or(|at| at.elapsed() >= Duration::from_millis(PRESS_DEBOUNCE_MS)))
}

fn create_hid_api() -> Result<HidApi> {
    let api = HidApi::new().context("failed to initialize HID API")?;
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(true);
    Ok(api)
}

pub mod linux;
pub mod macos;
pub mod windows;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_keyboard_usage_from_boot_report() {
        let report = [0_u8, 0, 0x28, 0, 0, 0, 0, 0];
        assert!(report_contains_usage(&report, 0x28));
        assert!(!report_contains_usage(&report, 0x2c));
    }

    #[test]
    fn detects_keyboard_usage_from_report_with_id() {
        let report = [1_u8, 0, 0, 0x28, 0, 0, 0, 0, 0];
        assert!(report_contains_usage(&report, 0x28));
    }

    #[test]
    fn detects_press_from_vendor_report_activity() {
        let report = [0x66_u8, 0xcc, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00];
        assert!(report_indicates_press(&report, 0x28));
    }

    #[test]
    fn detects_press_from_vendor_report_beyond_keyboard_slots() {
        let report = [0x66_u8, 0xcc, 0x03, 0, 0, 0, 0, 0, 0, 0x01];
        assert!(report_indicates_press(&report, 0x28));
    }

    #[test]
    fn repeated_active_reports_can_still_count_as_presses_after_debounce() {
        assert!(should_emit_down(
            true,
            true,
            Some(Instant::now() - Duration::from_millis(PRESS_DEBOUNCE_MS + 1))
        ));
    }

    #[test]
    fn repeated_active_reports_are_filtered_inside_debounce_window() {
        assert!(!should_emit_down(
            true,
            true,
            Some(Instant::now() - Duration::from_millis(PRESS_DEBOUNCE_MS - 1))
        ));
    }
}
