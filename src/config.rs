use std::env;

use anyhow::{Context, Result, bail};
use url::Url;

use crate::cli::Cli;
use crate::constants::{
    DEFAULT_KEY, DEFAULT_PRODUCT_ID, DEFAULT_TIMEOUT_MS, DEFAULT_VENDOR_ID, DEFAULT_WEBHOOK_URL,
};
use crate::event::{ButtonKey, DeviceMatcher};

#[derive(Debug, Clone)]
pub struct Config {
    pub vendor_id: u16,
    pub product_id: u16,
    pub key: ButtonKey,
    pub webhook_url: Url,
    pub timeout_ms: u64,
    pub verbose: bool,
}

impl Config {
    pub fn resolve(cli: &Cli) -> Result<Self> {
        let vendor_id = resolve_u16(
            cli.vendor_id,
            "GREENTIC_REDBUTTON_VENDOR_ID",
            DEFAULT_VENDOR_ID,
        )?;
        let product_id = resolve_u16(
            cli.product_id,
            "GREENTIC_REDBUTTON_PRODUCT_ID",
            DEFAULT_PRODUCT_ID,
        )?;
        let key = cli
            .key
            .clone()
            .or_else(|| env::var("GREENTIC_REDBUTTON_KEY").ok())
            .unwrap_or_else(|| DEFAULT_KEY.to_string());
        let webhook_url_raw = cli
            .webhook_url
            .clone()
            .or_else(|| env::var("GREENTIC_REDBUTTON_WEBHOOK_URL").ok())
            .unwrap_or_else(|| DEFAULT_WEBHOOK_URL.to_string());
        let timeout_ms = resolve_u64(
            cli.timeout_ms,
            "GREENTIC_REDBUTTON_TIMEOUT_MS",
            DEFAULT_TIMEOUT_MS,
        )?;

        if timeout_ms == 0 {
            bail!("timeout must be greater than zero milliseconds");
        }

        Ok(Self {
            vendor_id,
            product_id,
            key: ButtonKey::parse(&key),
            webhook_url: Url::parse(&webhook_url_raw)
                .with_context(|| format!("invalid webhook URL: {webhook_url_raw}"))?,
            timeout_ms,
            verbose: cli.verbose,
        })
    }

    pub fn matcher(&self) -> DeviceMatcher {
        DeviceMatcher {
            vendor_id: self.vendor_id,
            product_id: self.product_id,
            key: self.key.clone(),
        }
    }
}

fn resolve_u16(cli_value: Option<u16>, env_key: &str, default: u16) -> Result<u16> {
    if let Some(value) = cli_value {
        return Ok(value);
    }
    if let Ok(raw) = env::var(env_key) {
        return raw
            .parse::<u16>()
            .with_context(|| format!("invalid value for {env_key}: {raw}"));
    }
    Ok(default)
}

fn resolve_u64(cli_value: Option<u64>, env_key: &str, default: u64) -> Result<u64> {
    if let Some(value) = cli_value {
        return Ok(value);
    }
    if let Ok(raw) = env::var(env_key) {
        return raw
            .parse::<u64>()
            .with_context(|| format!("invalid value for {env_key}: {raw}"));
    }
    Ok(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command};

    fn base_cli() -> Cli {
        Cli {
            locale: None,
            vendor_id: None,
            product_id: None,
            key: None,
            webhook_url: None,
            timeout_ms: None,
            verbose: false,
            command: Some(Command::Version),
        }
    }

    #[test]
    fn cli_values_override_defaults() {
        let mut cli = base_cli();
        cli.vendor_id = Some(1);
        cli.product_id = Some(2);
        cli.key = Some("space".to_string());
        cli.webhook_url = Some("http://localhost:9090/hook".to_string());
        cli.timeout_ms = Some(1234);

        let config = Config::resolve(&cli).expect("config");
        assert_eq!(config.vendor_id, 1);
        assert_eq!(config.product_id, 2);
        assert_eq!(config.key, ButtonKey::Other("space".to_string()));
        assert_eq!(config.webhook_url.as_str(), "http://localhost:9090/hook");
        assert_eq!(config.timeout_ms, 1234);
    }
}
