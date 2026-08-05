//! Network layer — plain HTTPS, no browser process.

use anyhow::{Context, Result, bail};
use std::time::Instant;

#[derive(Debug)]
pub struct Fetched {
    pub url: String,
    #[allow(dead_code)]
    pub status: u16,
    #[allow(dead_code)]
    pub content_type: String,
    pub body: String,
    pub fetch_ms: u64,
    pub bytes: usize,
}

pub async fn fetch_url(url: &str) -> Result<Fetched> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "termbrowse/0.1 (+https://github.com/local/termbrowse; document browser)"
        ))
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("build http client")?;

    let start = Instant::now();
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("request failed: {url}"))?;

    let status = response.status().as_u16();
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    if !response.status().is_success() {
        bail!("HTTP {status} for {final_url}");
    }

    let body = response
        .text()
        .await
        .context("read response body as text")?;
    let fetch_ms = start.elapsed().as_millis() as u64;
    let bytes = body.len();

    Ok(Fetched {
        url: final_url,
        status,
        content_type,
        body,
        fetch_ms,
        bytes,
    })
}
