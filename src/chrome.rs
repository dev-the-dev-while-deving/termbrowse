//! Chrome backend: optional **extract** for thin JS shells, plus legacy screenshot path.
//! Product default uses structure-first; Chrome is not the face of the browser.

use anyhow::{Context, Result, bail};
use headless_chrome::protocol::cdp::Emulation::SetDeviceMetricsOverride;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{Browser, LaunchOptions, Tab};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::model::{Block, Document, Link, Ref, Span, Timing};
use crate::parse::parse_html;

/// Source capture resolution — true 720p.
pub const VIEW_W: u32 = 1280;
pub const VIEW_H: u32 = 720;

pub struct FullBrowser {
    _browser: Browser,
    tab: Arc<Tab>,
}

#[derive(Debug, Clone)]
pub struct PageFrame {
    pub doc: Document,
    /// PNG screenshot of the current viewport (JS-rendered).
    pub png: Vec<u8>,
    pub load_ms: u64,
}

#[derive(Debug, Deserialize)]
struct JsLink {
    href: String,
    text: String,
}

impl FullBrowser {
    pub fn launch() -> Result<Self> {
        let options = LaunchOptions::default_builder()
            .headless(true)
            .window_size(Some((VIEW_W as u32, VIEW_H as u32)))
            .idle_browser_timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| anyhow::anyhow!("launch options: {e}"))?;

        let browser = Browser::new(options).context(
            "failed to launch Chrome — is Google Chrome installed?\n\
             macOS: install from https://www.google.com/chrome/",
        )?;

        let tab = browser.new_tab().context("open tab")?;
        // Prefer a normal desktop UA so sites serve full layouts / thumbnails.
        tab.set_user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/122.0.0.0 Safari/537.36",
            None,
            None,
        )
        .ok();

        // Lock viewport to 720p (not just window chrome size).
        tab.call_method(SetDeviceMetricsOverride {
            width: VIEW_W,
            height: VIEW_H,
            device_scale_factor: 1.0,
            mobile: false,
            scale: None,
            screen_width: Some(VIEW_W),
            screen_height: Some(VIEW_H),
            position_x: None,
            position_y: None,
            dont_set_visible_size: None,
            screen_orientation: None,
            viewport: None,
            display_feature: None,
            device_posture: None,
        })
        .context("set 720p device metrics")?;

        Ok(Self {
            _browser: browser,
            tab,
        })
    }

    pub fn open(&self, url: &str) -> Result<PageFrame> {
        let start = Instant::now();
        self.tab
            .navigate_to(url)
            .with_context(|| format!("navigate to {url}"))?;
        self.tab
            .wait_until_navigated()
            .context("wait for navigation")?;

        // SPAs (YouTube, etc.) keep working after "navigated" — give paint time.
        self.wait_for_content()?;

        let mut frame = self.capture()?;
        frame.load_ms = start.elapsed().as_millis() as u64;
        frame.doc.timing_ms.fetch_ms = frame.load_ms;
        Ok(frame)
    }

    pub fn reload(&self) -> Result<PageFrame> {
        let start = Instant::now();
        self.tab.reload(false, None).context("reload")?;
        self.tab.wait_until_navigated().ok();
        self.wait_for_content()?;
        let mut frame = self.capture()?;
        frame.load_ms = start.elapsed().as_millis() as u64;
        frame.doc.timing_ms.fetch_ms = frame.load_ms;
        Ok(frame)
    }

    /// Scroll the real page, then re-capture so the terminal preview updates.
    pub fn scroll(&self, dy: i32) -> Result<PageFrame> {
        let js = format!("window.scrollBy(0, {dy}); true");
        self.tab.evaluate(&js, false).context("scroll")?;
        std::thread::sleep(Duration::from_millis(120));
        self.capture()
    }

    pub fn click_selector(&self, selector: &str) -> Result<PageFrame> {
        self.tab
            .wait_for_element(selector)
            .with_context(|| format!("find {selector}"))?
            .click()
            .context("click")?;
        std::thread::sleep(Duration::from_millis(400));
        self.tab.wait_until_navigated().ok();
        self.wait_for_content()?;
        self.capture()
    }

    pub fn open_href(&self, href: &str) -> Result<PageFrame> {
        self.open(href)
    }

    /// Navigate and extract a structured Document (no screenshot).
    pub fn extract_document(&self, url: &str) -> Result<Document> {
        let start = Instant::now();
        self.tab
            .navigate_to(url)
            .with_context(|| format!("navigate {url}"))?;
        self.tab.wait_until_navigated().ok();
        std::thread::sleep(Duration::from_millis(1200));
        let _ = self.wait_for_content();

        let final_url = self.tab.get_url();
        let fetch_ms = start.elapsed().as_millis() as u64;

        let html = self
            .tab
            .evaluate("document.documentElement.outerHTML", false)
            .ok()
            .and_then(|r| r.value)
            .and_then(|v| v.as_str().map(|s| s.to_string()));

        if let Some(html) = html {
            let doc = parse_html(&final_url, &html, fetch_ms);
            if doc.text_len() >= 280 || doc.links.len() >= 5 {
                return Ok(doc);
            }
        }

        self.document_from_dom(&final_url, fetch_ms)
    }

    fn document_from_dom(&self, url: &str, fetch_ms: u64) -> Result<Document> {
        let title = self.tab.get_title().unwrap_or_else(|_| url.to_string());

        let text = self
            .tab
            .evaluate(
                r#"(function(){
                    const b = document.body;
                    return b ? (b.innerText || '').trim() : '';
                })()"#,
                false,
            )
            .ok()
            .and_then(|r| r.value)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        let links = self.extract_links().unwrap_or_default();
        let mut doc_links = Vec::new();
        for (i, l) in links.into_iter().take(120).enumerate() {
            doc_links.push(Link {
                r#ref: Ref((i + 1) as u32),
                href: l.href,
                text: if l.text.is_empty() {
                    format!("link-{}", i + 1)
                } else {
                    l.text.chars().take(100).collect()
                },
            });
        }

        let mut blocks = Vec::new();
        if !title.is_empty() {
            blocks.push(Block::Heading {
                level: 1,
                text: title.clone(),
            });
            blocks.push(Block::Spacer);
        }
        for para in text.split("\n\n") {
            let p = para
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if p.is_empty() {
                continue;
            }
            blocks.push(Block::Paragraph {
                spans: vec![Span::Text { text: p }],
            });
            blocks.push(Block::Spacer);
        }
        if !doc_links.is_empty() {
            blocks.push(Block::Heading {
                level: 2,
                text: "Links".into(),
            });
            blocks.push(Block::Spacer);
            for l in &doc_links {
                blocks.push(Block::ListItem {
                    spans: vec![Span::Link {
                        r#ref: l.r#ref,
                        text: l.text.clone(),
                    }],
                });
            }
        }

        Ok(Document {
            url: url.to_string(),
            title,
            blocks,
            links: doc_links,
            timing_ms: Timing {
                fetch_ms,
                parse_ms: 0,
                layout_ms: 0,
            },
        })
    }

    fn wait_for_content(&self) -> Result<()> {
        // Wait until body has meaningful content OR timeout.
        // Prefer real media (thumbnails) when present — e.g. YouTube search/trending.
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            let ready = self
                .tab
                .evaluate(
                    r#"(function(){
                        const b = document.body;
                        if (!b) return false;
                        const imgs = document.images ? document.images.length : 0;
                        // Count images that actually loaded with size (thumbnails)
                        let painted = 0;
                        if (document.images) {
                          for (const im of document.images) {
                            if (im.naturalWidth > 32 && im.naturalHeight > 32) painted++;
                          }
                        }
                        const text = (b.innerText || '').trim().length;
                        // YouTube search: ytd-video-renderer / rich items
                        const ytCards = document.querySelectorAll(
                          'ytd-video-renderer, ytd-rich-item-renderer, ytd-grid-video-renderer, a#thumbnail'
                        ).length;
                        return painted >= 4 || ytCards >= 3 || (imgs >= 8 && text > 200) || text > 800;
                    })()"#,
                    false,
                )
                .ok()
                .and_then(|r| r.value)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if ready || Instant::now() >= deadline {
                // Extra beat for images to paint.
                std::thread::sleep(Duration::from_millis(800));
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }

    fn capture(&self) -> Result<PageFrame> {
        let png = self
            .tab
            .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)
            .context("screenshot")?;

        let title = self
            .tab
            .get_title()
            .unwrap_or_else(|_| "untitled".into());
        let url = self.tab.get_url();

        let links = self.extract_links().unwrap_or_default();
        let mut doc_links = Vec::new();
        for (i, l) in links.into_iter().take(200).enumerate() {
            doc_links.push(Link {
                r#ref: Ref((i + 1) as u32),
                href: l.href,
                text: if l.text.is_empty() {
                    format!("link-{}", i + 1)
                } else {
                    l.text.chars().take(100).collect()
                },
            });
        }

        // Minimal block list so lite snapshot tools still work if reused.
        let doc = Document {
            url,
            title,
            blocks: vec![],
            links: doc_links,
            timing_ms: Timing::default(),
        };

        Ok(PageFrame {
            doc,
            png,
            load_ms: 0,
        })
    }

    fn extract_links(&self) -> Result<Vec<JsLink>> {
        let result = self
            .tab
            .evaluate(
                r#"(function(){
                    const out = [];
                    const seen = new Set();
                    const nodes = document.querySelectorAll('a[href]');
                    for (const a of nodes) {
                        let href = a.href || '';
                        if (!href || href.startsWith('javascript:')) continue;
                        if (seen.has(href)) continue;
                        seen.add(href);
                        let text = (a.innerText || a.getAttribute('aria-label') || a.title || '').trim();
                        text = text.replace(/\s+/g, ' ').slice(0, 120);
                        if (!text) {
                            const img = a.querySelector('img');
                            if (img) text = img.alt || 'image';
                        }
                        if (!text) text = href;
                        out.push({ href, text });
                        if (out.length >= 150) break;
                    }
                    return JSON.stringify(out);
                })()"#,
                false,
            )
            .context("evaluate links")?;

        let value = result
            .value
            .context("no value from link extraction")?;
        let s = value
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| serde_json::to_string(&value).ok())
            .context("links not string")?;

        // headless_chrome sometimes returns the JSON string already, sometimes nested.
        let parsed: Vec<JsLink> = serde_json::from_str(&s)
            .or_else(|_| {
                // value might be a JSON-encoded string inside JSON
                let inner: String = serde_json::from_str(&s)?;
                serde_json::from_str(&inner)
            })
            .with_context(|| format!("parse links json: {}", &s[..s.len().min(200)]))?;

        Ok(parsed)
    }
}

/// Decode PNG and return RGB8 buffer + dimensions.
pub fn decode_png(png: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(png).context("decode png")?.to_rgb8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}

pub fn ensure_http_url(input: &str) -> Result<String> {
    let t = input.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        return Ok(t.to_string());
    }
    if t.contains('.') && !t.contains(' ') {
        return Ok(format!("https://{t}"));
    }
    bail!("not a URL: {input}");
}
