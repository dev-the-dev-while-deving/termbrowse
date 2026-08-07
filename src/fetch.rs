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
    #[allow(dead_code)]
    pub bytes: usize,
}

pub async fn fetch_url(url: &str) -> Result<Fetched> {
    // Look like a normal browser — custom bot UAs get CAPTCHAs on Google/etc.
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/122.0.0.0 Safari/537.36",
        )
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("build http client")?;

    let start = Instant::now();
    // Prefer HTML over other types when the server negotiates.
    let response = client
        .get(url)
        .header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8",
        )
        .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
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

#[derive(Debug, Clone)]
pub struct FetchedImage {
    #[allow(dead_code)]
    pub url: String,
    pub bytes: Vec<u8>,
    #[allow(dead_code)]
    pub content_type: String,
}

static IMAGE_FETCH_SEMAPHORE: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::OnceLock::new();

fn get_image_semaphore() -> std::sync::Arc<tokio::sync::Semaphore> {
    IMAGE_FETCH_SEMAPHORE
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(4)))
        .clone()
}

pub async fn fetch_image(url: &str) -> Result<FetchedImage> {
    if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
        bail!("invalid image URL: {url}");
    }

    let sem = get_image_semaphore();
    let _permit = sem
        .acquire()
        .await
        .context("acquire image fetch concurrency permit")?;

    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/122.0.0.0 Safari/537.36",
        )
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("build image http client")?;

    let response = client
        .get(url)
        .header(
            reqwest::header::ACCEPT,
            "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
        )
        .send()
        .await
        .with_context(|| format!("image fetch failed: {url}"))?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        bail!("HTTP {status} for image {url}");
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = response.bytes().await.context("read image response bytes")?;

    if bytes.len() > 10 * 1024 * 1024 {
        bail!("image size {} exceeds 10MB limit", bytes.len());
    }

    Ok(FetchedImage {
        url: url.to_string(),
        bytes: bytes.to_vec(),
        content_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_image_valid() {
        let res = fetch_image("https://kalalawyer.com/static/images/AdvMohanLalKala.webp").await;
        assert!(res.is_ok(), "Image fetch should succeed");
        let img = res.unwrap();
        assert!(!img.bytes.is_empty());
        assert!(img.bytes.len() < 10 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_fetch_image_invalid_url() {
        let res = fetch_image("not_a_valid_url").await;
        assert!(res.is_err());
    }
}

