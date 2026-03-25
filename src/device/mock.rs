use std::sync::{Arc, Mutex, mpsc};

use anyhow::{Result, anyhow};
use chrono::Utc;

use super::{DeviceBackend, DeviceStream};
use crate::event::{ButtonEvent, ButtonEventKind, DeviceInfo, DeviceMatcher};

/// Simulated device events sent through a channel.
pub enum MockEvent {
    Down,
    Up,
    #[allow(dead_code)]
    Disconnect,
}

/// A mock device backend that produces events from a shared channel.
pub struct MockBackend {
    devices: Vec<DeviceInfo>,
    event_receiver: Arc<Mutex<mpsc::Receiver<MockEvent>>>,
}

impl MockBackend {
    pub fn new(devices: Vec<DeviceInfo>, event_receiver: mpsc::Receiver<MockEvent>) -> Self {
        Self {
            devices,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
        }
    }
}

impl DeviceBackend for MockBackend {
    fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self.devices.clone())
    }

    fn connect(&self, matcher: &DeviceMatcher) -> Result<Box<dyn DeviceStream>> {
        let matching = self
            .devices
            .iter()
            .find(|device| {
                device.vendor_id == matcher.vendor_id && device.product_id == matcher.product_id
            })
            .ok_or_else(|| {
                anyhow!(
                    "mock: no device matching {:04x}:{:04x}",
                    matcher.vendor_id,
                    matcher.product_id
                )
            })?;

        Ok(Box::new(MockStream {
            device_info: matching.clone(),
            receiver: Arc::clone(&self.event_receiver),
        }))
    }
}

struct MockStream {
    device_info: DeviceInfo,
    receiver: Arc<Mutex<mpsc::Receiver<MockEvent>>>,
}

impl DeviceStream for MockStream {
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    fn next_event(&mut self) -> Result<ButtonEvent> {
        let rx = self
            .receiver
            .lock()
            .map_err(|e| anyhow!("mock: lock poisoned: {e}"))?;
        match rx.recv() {
            Ok(MockEvent::Down) => Ok(ButtonEvent {
                kind: ButtonEventKind::Down,
                timestamp: Utc::now(),
            }),
            Ok(MockEvent::Up) => Ok(ButtonEvent {
                kind: ButtonEventKind::Up,
                timestamp: Utc::now(),
            }),
            Ok(MockEvent::Disconnect) => Err(anyhow!("mock: device disconnected")),
            Err(_) => Err(anyhow!("mock: event channel closed")),
        }
    }
}
