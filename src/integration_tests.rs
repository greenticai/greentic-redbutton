use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::config::Config;
use crate::constants::{DEFAULT_TIMEOUT_MS, EVENT_TYPE_CLICK, SOURCE_NAME};
use crate::device::DeviceBackend;
use crate::device::mock::{MockBackend, MockEvent};
use crate::event::{ButtonKey, DeviceInfo, WebhookEvent};
use crate::webhook::send_webhook;

use pretty_assertions::assert_eq;

/// Start a minimal HTTP server that accepts one POST and returns the body.
fn spawn_webhook_receiver() -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock webhook server");
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{port}/events/webhook");

    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        if let Some(stream) = listener.incoming().next() {
            let mut stream = match stream {
                Ok(stream) => stream,
                Err(_) => return,
            };

            let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
            let mut request_line = String::new();
            reader.read_line(&mut request_line).ok();

            let mut content_length: usize = 0;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).ok();
                let trimmed = header.trim();
                if trimmed.is_empty() {
                    break;
                }
                if let Some(value) = trimmed.strip_prefix("content-length:") {
                    content_length = value.trim().parse().unwrap_or(0);
                } else if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }

            let mut body = vec![0_u8; content_length];
            if content_length > 0 {
                std::io::Read::read_exact(&mut reader, &mut body).ok();
            }

            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(response.as_bytes()).ok();
            stream.flush().ok();

            let _ = sender.send(String::from_utf8_lossy(&body).to_string());
        }
    });

    (url, receiver)
}

/// Start a mock server that always returns an HTTP error status.
fn spawn_failing_webhook_receiver() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind failing mock server");
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{port}/events/webhook");

    thread::spawn(move || {
        if let Some(stream) = listener.incoming().next() {
            let mut stream = match stream {
                Ok(stream) => stream,
                Err(_) => return,
            };

            let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).ok();
                if line.trim().is_empty() {
                    break;
                }
            }

            let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(response.as_bytes()).ok();
            stream.flush().ok();
        }
    });

    url
}

fn test_device_info() -> DeviceInfo {
    DeviceInfo {
        name: Some("MockButton".to_string()),
        vendor_id: 32_904,
        product_id: 21,
        backend: "mock",
    }
}

fn test_config(webhook_url: &str) -> Config {
    Config {
        vendor_id: 32_904,
        product_id: 21,
        key: ButtonKey::Enter,
        webhook_url: url::Url::parse(webhook_url).unwrap(),
        timeout_ms: DEFAULT_TIMEOUT_MS,
        verbose: false,
        suppress: false,
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[test]
fn webhook_payload_has_correct_shape() {
    let (url, receiver) = spawn_webhook_receiver();
    let payload = WebhookEvent {
        source: SOURCE_NAME,
        event_type: EVENT_TYPE_CLICK,
        vendor_id: 32_904,
        product_id: 21,
        key: "enter".to_string(),
        timestamp: chrono::Utc::now(),
        device_name: Some("MockButton".to_string()),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    };

    send_webhook(&url::Url::parse(&url).unwrap(), &payload, 5_000).expect("webhook send");

    let body = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("receive webhook body");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("parse JSON");

    assert_eq!(parsed["source"], "greentic-redbutton");
    assert_eq!(parsed["event_type"], "redbutton.click");
    assert_eq!(parsed["vendor_id"], 32_904);
    assert_eq!(parsed["product_id"], 21);
    assert_eq!(parsed["key"], "enter");
    assert!(parsed["timestamp"].is_string());
    assert_eq!(parsed["device_name"], "MockButton");
    assert_eq!(parsed["os"], std::env::consts::OS);
    assert_eq!(parsed["arch"], std::env::consts::ARCH);
}

#[test]
fn webhook_returns_error_on_server_failure() {
    let url = spawn_failing_webhook_receiver();
    // Give the server thread time to start listening.
    thread::sleep(Duration::from_millis(50));

    let payload = WebhookEvent {
        source: SOURCE_NAME,
        event_type: EVENT_TYPE_CLICK,
        vendor_id: 32_904,
        product_id: 21,
        key: "enter".to_string(),
        timestamp: chrono::Utc::now(),
        device_name: None,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    };

    let result = send_webhook(&url::Url::parse(&url).unwrap(), &payload, 5_000);
    assert!(result.is_err());
    let err_msg = format!("{:#}", result.unwrap_err());
    assert!(
        err_msg.contains("500"),
        "expected HTTP 500 in error, got: {err_msg}"
    );
}

#[test]
fn mock_backend_lists_devices() {
    let (_sender, receiver) = mpsc::channel();
    let backend = MockBackend::new(vec![test_device_info()], receiver);

    let devices = backend.list_devices().expect("list devices");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].vendor_id, 32_904);
    assert_eq!(devices[0].product_id, 21);
    assert_eq!(devices[0].name.as_deref(), Some("MockButton"));
}

#[test]
fn mock_backend_connect_fails_for_unknown_device() {
    use crate::device::DeviceBackend;

    let (_sender, receiver) = mpsc::channel();
    let backend = MockBackend::new(vec![test_device_info()], receiver);

    let matcher = crate::event::DeviceMatcher {
        vendor_id: 9999,
        product_id: 9999,
        key: ButtonKey::Enter,
    };

    let result = backend.connect(&matcher);
    assert!(result.is_err());
}

#[test]
fn run_once_delivers_webhook_on_button_press() {
    let (url, receiver) = spawn_webhook_receiver();
    let config = test_config(&url);

    let (event_sender, event_receiver) = mpsc::channel();
    let backend = MockBackend::new(vec![test_device_info()], event_receiver);

    // Send a Down event after a brief delay so run_once picks it up.
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        event_sender.send(MockEvent::Down).ok();
    });

    let payload = crate::runtime::run_once(&config, &backend).expect("run_once");

    assert_eq!(payload.source, SOURCE_NAME);
    assert_eq!(payload.event_type, EVENT_TYPE_CLICK);
    assert_eq!(payload.vendor_id, 32_904);
    assert_eq!(payload.key, "enter");

    // Verify the webhook was actually received by the mock server.
    let body = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("receive webhook body");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("parse JSON");
    assert_eq!(parsed["event_type"], "redbutton.click");
}

#[test]
fn run_once_ignores_up_events() {
    let (url, receiver) = spawn_webhook_receiver();
    let config = test_config(&url);

    let (event_sender, event_receiver) = mpsc::channel();
    let backend = MockBackend::new(vec![test_device_info()], event_receiver);

    // Send Up first (should be ignored), then Down.
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        event_sender.send(MockEvent::Up).ok();
        thread::sleep(Duration::from_millis(50));
        event_sender.send(MockEvent::Down).ok();
    });

    let payload = crate::runtime::run_once(&config, &backend).expect("run_once");
    assert_eq!(payload.event_type, EVENT_TYPE_CLICK);

    let body = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("receive webhook body");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("parse JSON");
    assert_eq!(parsed["event_type"], "redbutton.click");
}

#[test]
fn doctor_reports_matching_devices() {
    let (event_sender, event_receiver) = mpsc::channel();
    let backend = MockBackend::new(vec![test_device_info()], event_receiver);

    let mut config = test_config("http://127.0.0.1:9999/events/webhook");
    // Use short timeout so doctor press-test finishes quickly.
    config.timeout_ms = 500;

    // Send a press to satisfy the doctor's press detection.
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        event_sender.send(MockEvent::Down).ok();
    });

    let report = crate::doctor::run(&config, &backend).expect("doctor run");

    assert_eq!(report.matching_devices.len(), 1);
    assert!(report.matching_devices[0].contains("MockButton"));
    assert!(report.press_result.is_some());
    assert!(report.config_summary.len() >= 5);
}

#[test]
fn doctor_reports_no_match_when_device_absent() {
    let (_sender, event_receiver) = mpsc::channel::<MockEvent>();
    let backend = MockBackend::new(vec![], event_receiver);

    let mut config = test_config("http://127.0.0.1:9999/events/webhook");
    config.timeout_ms = 300;

    let report = crate::doctor::run(&config, &backend).expect("doctor run");

    assert!(report.matching_devices.is_empty());
    assert!(report.press_result.is_none());
}

#[test]
fn doctor_times_out_when_no_press() {
    let (_sender, event_receiver) = mpsc::channel::<MockEvent>();
    let backend = MockBackend::new(vec![test_device_info()], event_receiver);

    let mut config = test_config("http://127.0.0.1:9999/events/webhook");
    config.timeout_ms = 300;

    let report = crate::doctor::run(&config, &backend).expect("doctor run");

    assert_eq!(report.matching_devices.len(), 1);
    // No press sent, so press_result should be None (timeout).
    assert!(report.press_result.is_none());
}

#[test]
fn i18n_loads_and_translates() {
    let bundle = crate::i18n::I18n::load().expect("load i18n");
    let locale = bundle.select_locale(Some("en".to_string()));
    assert_eq!(locale, "en");

    // The version message should contain the placeholder.
    let version_msg = bundle.tf(
        &locale,
        "cli.runtime.version",
        &[("version", "0.4.1".to_string())],
    );
    assert!(
        version_msg.contains("0.4.1"),
        "expected version in message, got: {version_msg}"
    );
}

#[test]
fn config_resolves_from_defaults() {
    use crate::cli::Cli;
    use clap::Parser;

    // Simulate bare CLI invocation with no flags.
    let cli = Cli::parse_from(["greentic-redbutton", "version"]);
    let config = Config::resolve(&cli).expect("config resolve");

    assert_eq!(config.vendor_id, 32_904);
    assert_eq!(config.product_id, 21);
    assert_eq!(config.key, ButtonKey::Enter);
    assert_eq!(
        config.webhook_url.as_str(),
        "http://127.0.0.1:8080/events/webhook"
    );
    assert_eq!(config.timeout_ms, 5_000);
}

#[test]
fn config_resolves_cli_overrides() {
    use crate::cli::Cli;
    use clap::Parser;

    let cli = Cli::parse_from([
        "greentic-redbutton",
        "--vendor-id",
        "1234",
        "--product-id",
        "5678",
        "--key",
        "space",
        "--webhook-url",
        "http://example.com/hook",
        "--timeout-ms",
        "9999",
        "version",
    ]);
    let config = Config::resolve(&cli).expect("config resolve");

    assert_eq!(config.vendor_id, 1234);
    assert_eq!(config.product_id, 5678);
    assert_eq!(config.key, ButtonKey::Other("space".to_string()));
    assert_eq!(config.webhook_url.as_str(), "http://example.com/hook");
    assert_eq!(config.timeout_ms, 9999);
}
