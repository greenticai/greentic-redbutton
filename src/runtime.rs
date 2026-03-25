use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::config::Config;
use crate::constants::{EVENT_TYPE_CLICK, RECONNECT_DELAY_MS, SOURCE_NAME};
use crate::device::{DeviceBackend, DeviceStream};
use crate::event::{ButtonEventKind, WebhookEvent};
use crate::suppress::{
    InputSuppressor, activate_input_suppressor, ensure_startup_permissions,
    log_startup_permission_status,
};
use crate::webhook::send_webhook;

fn maybe_ensure_permissions(config: &Config) -> Result<()> {
    if config.suppress {
        ensure_startup_permissions(&config.matcher())?;
    }
    Ok(())
}

fn maybe_activate_suppressor(config: &Config) -> Result<Box<dyn InputSuppressor>> {
    if config.suppress {
        activate_input_suppressor(&config.matcher())
    } else {
        Ok(Box::new(NoopSuppressor))
    }
}

struct NoopSuppressor;

impl InputSuppressor for NoopSuppressor {
    fn notify_button_press(&self) {}
}

pub fn run_listener(config: &Config, backend: &dyn DeviceBackend) -> Result<()> {
    maybe_ensure_permissions(config)?;
    let suppressor = maybe_activate_suppressor(config)?;
    let mut webhook_sender = spawn_webhook_worker(config);
    let mut stream = backend
        .connect(&config.matcher())
        .context("failed to connect to matching device")?;

    loop {
        let event = stream.next_event()?;
        if event.kind != ButtonEventKind::Down {
            continue;
        }

        suppressor.notify_button_press();
        println!(
            "Button press detected from {} at {}",
            stream
                .device_info()
                .name
                .as_deref()
                .unwrap_or("unknown device"),
            event.timestamp.to_rfc3339()
        );
        let payload = build_payload(config, stream.as_ref(), event.timestamp);
        if webhook_sender.send(payload.clone()).is_err() {
            eprintln!("webhook worker stopped; restarting worker");
            webhook_sender = spawn_webhook_worker(config);
            if webhook_sender.send(payload).is_err() {
                eprintln!("webhook worker restart failed; dropping event");
            }
        }
    }
}

pub fn run_once(config: &Config, backend: &dyn DeviceBackend) -> Result<WebhookEvent> {
    if config.suppress {
        log_startup_permission_status(&config.matcher());
    }
    maybe_ensure_permissions(config)?;
    let suppressor = maybe_activate_suppressor(config)?;
    let mut stream = backend
        .connect(&config.matcher())
        .context("failed to connect to matching device")?;

    loop {
        let event = stream.next_event()?;
        if event.kind == ButtonEventKind::Down {
            suppressor.notify_button_press();
            println!(
                "Button press detected from {} at {}",
                stream
                    .device_info()
                    .name
                    .as_deref()
                    .unwrap_or("unknown device"),
                event.timestamp.to_rfc3339()
            );
            let payload = build_payload(config, stream.as_ref(), event.timestamp);
            send_webhook(&config.webhook_url, &payload, config.timeout_ms)?;
            return Ok(payload);
        }
    }
}

pub fn wait_for_press(
    config: &Config,
    backend: &dyn DeviceBackend,
) -> Result<Option<(String, chrono::DateTime<chrono::Utc>)>> {
    maybe_ensure_permissions(config)?;
    let suppressor = maybe_activate_suppressor(config)?;
    let start = Instant::now();
    let timeout = Duration::from_millis(config.timeout_ms);

    while start.elapsed() < timeout {
        match backend.connect(&config.matcher()) {
            Ok(mut stream) => {
                let device_name = stream
                    .device_info()
                    .name
                    .clone()
                    .unwrap_or_else(|| "unknown device".to_string());
                let (sender, receiver) = mpsc::channel();
                thread::spawn(move || {
                    let result = loop {
                        match stream.next_event() {
                            Ok(event) if event.kind == ButtonEventKind::Down => {
                                break Ok((device_name, event.timestamp));
                            }
                            Ok(_) => continue,
                            Err(error) => break Err(error),
                        }
                    };
                    let _ = sender.send(result);
                });

                let remaining = timeout.saturating_sub(start.elapsed());
                match receiver.recv_timeout(remaining) {
                    Ok(result) => {
                        if result.is_ok() {
                            suppressor.notify_button_press();
                        }
                        return result.map(Some);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => return Ok(None),
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        thread::sleep(Duration::from_millis(RECONNECT_DELAY_MS));
                    }
                }
            }
            Err(_) => thread::sleep(Duration::from_millis(RECONNECT_DELAY_MS)),
        }
    }

    Ok(None)
}

pub fn reconnecting_listener(config: &Config, backend: &dyn DeviceBackend) -> Result<()> {
    if config.suppress {
        log_startup_permission_status(&config.matcher());
    }
    loop {
        match run_listener(config, backend) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if config.verbose {
                    eprintln!("listener disconnected: {error:#}");
                }
                thread::sleep(Duration::from_millis(RECONNECT_DELAY_MS));
            }
        }
    }
}

fn build_payload(
    config: &Config,
    stream: &dyn DeviceStream,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> WebhookEvent {
    WebhookEvent {
        source: SOURCE_NAME,
        event_type: EVENT_TYPE_CLICK,
        vendor_id: config.vendor_id,
        product_id: config.product_id,
        key: config.key.as_config_value().to_string(),
        timestamp,
        device_name: stream.device_info().name.clone(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

fn spawn_webhook_worker(config: &Config) -> mpsc::Sender<WebhookEvent> {
    let (sender, receiver) = mpsc::channel();
    let webhook_url = config.webhook_url.clone();
    let timeout_ms = config.timeout_ms;
    let verbose = config.verbose;
    let vendor_id = config.vendor_id;
    let product_id = config.product_id;

    thread::spawn(move || {
        while let Ok(payload) = receiver.recv() {
            match send_webhook(&webhook_url, &payload, timeout_ms) {
                Ok(()) => {
                    if verbose {
                        eprintln!(
                            "sent {} for {:04x}:{:04x}",
                            EVENT_TYPE_CLICK, vendor_id, product_id
                        );
                    }
                }
                Err(error) => {
                    eprintln!("webhook delivery failed: {error:#}");
                }
            }
        }
    });

    sender
}
