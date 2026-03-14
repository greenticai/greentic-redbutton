use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use std::time::Duration;
use url::Url;

use crate::event::WebhookEvent;

pub fn send_webhook(url: &Url, payload: &WebhookEvent, timeout_ms: u64) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .post(url.clone())
        .header(CONTENT_TYPE, "application/json")
        .json(payload)
        .send()
        .with_context(|| format!("failed to POST webhook to {url}"))?;

    if !response.status().is_success() {
        bail!("webhook returned HTTP {}", response.status());
    }

    Ok(())
}
