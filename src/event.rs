use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceMatcher {
    pub vendor_id: u16,
    pub product_id: u16,
    pub key: ButtonKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonKey {
    Enter,
    Other(String),
}

impl ButtonKey {
    pub fn parse(raw: &str) -> Self {
        let normalized = raw.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "enter" | "return" => Self::Enter,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn as_config_value(&self) -> &str {
        match self {
            Self::Enter => "enter",
            Self::Other(value) => value.as_str(),
        }
    }

    pub fn usage_id(&self) -> Option<u8> {
        match self {
            Self::Enter => Some(0x28),
            Self::Other(value) => keyboard_usage_for_name(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub name: Option<String>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub backend: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEventKind {
    Down,
    Up,
}

#[derive(Debug, Clone)]
pub struct ButtonEvent {
    pub kind: ButtonEventKind,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebhookEvent {
    pub source: &'static str,
    pub event_type: &'static str,
    pub vendor_id: u16,
    pub product_id: u16,
    pub key: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    pub os: &'static str,
    pub arch: &'static str,
}

fn keyboard_usage_for_name(name: &str) -> Option<u8> {
    match name {
        "enter" | "return" => Some(0x28),
        "escape" | "esc" => Some(0x29),
        "space" => Some(0x2c),
        "tab" => Some(0x2b),
        "backspace" => Some(0x2a),
        "up" => Some(0x52),
        "down" => Some(0x51),
        "left" => Some(0x50),
        "right" => Some(0x4f),
        "f1" => Some(0x3a),
        "f2" => Some(0x3b),
        "f3" => Some(0x3c),
        "f4" => Some(0x3d),
        "f5" => Some(0x3e),
        "f6" => Some(0x3f),
        "f7" => Some(0x40),
        "f8" => Some(0x41),
        "f9" => Some(0x42),
        "f10" => Some(0x43),
        "f11" => Some(0x44),
        "f12" => Some(0x45),
        _ => {
            let bytes = name.as_bytes();
            if bytes.len() == 1 {
                let ch = bytes[0];
                match ch {
                    b'a'..=b'z' => Some(0x04 + (ch - b'a')),
                    b'1'..=b'9' => Some(0x1e + (ch - b'1')),
                    b'0' => Some(0x27),
                    _ => None,
                }
            } else {
                name.strip_prefix("0x")
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_keys() {
        assert_eq!(ButtonKey::parse("Enter"), ButtonKey::Enter);
        assert_eq!(ButtonKey::parse("A"), ButtonKey::Other("a".to_string()));
    }

    #[test]
    fn maps_keyboard_usage_codes() {
        assert_eq!(ButtonKey::Enter.usage_id(), Some(0x28));
        assert_eq!(ButtonKey::Other("a".to_string()).usage_id(), Some(0x04));
        assert_eq!(ButtonKey::Other("0x28".to_string()).usage_id(), Some(0x28));
        assert_eq!(ButtonKey::Other("unknown".to_string()).usage_id(), None);
    }
}
