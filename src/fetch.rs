//! HTTPS only. One shared client. No browser process.

use anyhow::{Context, Result};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const MAX_BODY: usize = 2 * 1024 * 1024;
const TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub struct Fetched {
    pub url: String,
    pub status: u16,
    pub content_type: String,
    pub body: String,
    pub fetch_ms: u64,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/122.0.0.0 Safari/537.36",
            )
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .pool_max_idle_per_host(4)
            .build()
            .expect("http client")
    })
}

pub async fn fetch_url(url: &str) -> Result<Fetched> {
    let start = Instant::now();
    let response = send(url).await?;
    let status = response.status().as_u16();
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let mut body = response.text().await.context("read response body")?;
    if body.len() > MAX_BODY {
        body.truncate(MAX_BODY);
        while !body.is_char_boundary(body.len()) {
            body.pop();
        }
    }
    Ok(Fetched {
        url: final_url,
        status,
        content_type,
        body,
        fetch_ms: start.elapsed().as_millis() as u64,
    })
}

async fn send(url: &str) -> Result<reqwest::Response> {
    match send_once(url).await {
        Ok(r) => Ok(r),
        Err(e) if retryable(&e) => send_once(url)
            .await
            .with_context(|| format!("retry failed: {url}")),
        Err(e) => Err(e).with_context(|| format!("request failed: {url}")),
    }
}

async fn send_once(url: &str) -> Result<reqwest::Response, reqwest::Error> {
    client()
        .get(url)
        .header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8",
        )
        .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .send()
        .await
}

fn retryable(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || e.is_request()
}
