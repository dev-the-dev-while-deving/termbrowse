//! Session browser: structure-first load, optional Chrome escalate, history stack.
//!
//! Same Document model for human TUI and agent snapshot.

use crate::chrome::{self, FullBrowser};
use crate::fetch::fetch_url;
use crate::model::Document;
use crate::parse::parse_html;
use anyhow::{Context, Result};
use std::time::Instant;

/// How the page was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadSource {
    /// Plain HTTPS + HTML parse (fast path).
    Structure,
    /// Headless Chrome extract → same structure model (when HTML is a JS shell).
    Escalated,
}

#[derive(Debug, Clone)]
pub struct LoadedPage {
    pub doc: Document,
    pub source: LoadSource,
    pub total_ms: u64,
}

pub struct Session {
    /// Chronological stack; `cursor` points at current.
    history: Vec<LoadedPage>,
    cursor: usize,
    /// Allow Chrome escalate when structure is thin.
    pub escalate: bool,
    /// Reserved for sticky Chrome (v1 uses one-shot extract).
    #[allow(dead_code)]
    browser: Option<FullBrowser>,
}

impl Session {
    pub fn new(escalate: bool) -> Self {
        Self {
            history: Vec::new(),
            cursor: 0,
            escalate,
            browser: None,
        }
    }

    pub fn current(&self) -> Option<&LoadedPage> {
        self.history.get(self.cursor)
    }

    pub fn can_back(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_forward(&self) -> bool {
        self.cursor + 1 < self.history.len()
    }

    pub fn back(&mut self) -> Option<&LoadedPage> {
        if self.can_back() {
            self.cursor -= 1;
            self.current()
        } else {
            None
        }
    }

    pub fn forward(&mut self) -> Option<&LoadedPage> {
        if self.can_forward() {
            self.cursor += 1;
            self.current()
        } else {
            None
        }
    }

    /// Open URL, push history (drops any forward entries).
    pub async fn open(&mut self, url: &str) -> Result<&LoadedPage> {
        let url = chrome::ensure_http_url(url)?;
        let page = load_page(&url, self.escalate, &mut self.browser).await?;
        if self.cursor + 1 < self.history.len() {
            self.history.truncate(self.cursor + 1);
        }
        self.history.push(page);
        self.cursor = self.history.len() - 1;
        Ok(self.current().expect("just pushed"))
    }

    pub async fn reload(&mut self) -> Result<&LoadedPage> {
        let url = self
            .current()
            .map(|p| p.doc.url.clone())
            .context("nothing to reload")?;
        // Replace current entry rather than push.
        let page = load_page(&url, self.escalate, &mut self.browser).await?;
        self.history[self.cursor] = page;
        Ok(self.current().expect("cursor valid"))
    }

    pub async fn follow_href(&mut self, href: &str) -> Result<&LoadedPage> {
        let base = self
            .current()
            .map(|p| p.doc.url.clone())
            .unwrap_or_default();
        let abs = url::Url::parse(&base)
            .ok()
            .and_then(|b| b.join(href).ok())
            .or_else(|| url::Url::parse(href).ok())
            .context("bad href")?
            .to_string();
        self.open(&abs).await
    }
}

/// Load one page: structure first, escalate if thin.
pub async fn load_page(
    url: &str,
    escalate: bool,
    browser: &mut Option<FullBrowser>,
) -> Result<LoadedPage> {
    let start = Instant::now();
    let structure = load_structure(url).await?;

    let needs_escalate = is_thin(&structure) || is_sparse_serp(&structure);
    if !needs_escalate || !escalate {
        let total_ms = start.elapsed().as_millis() as u64;
        return Ok(LoadedPage {
            doc: structure,
            source: LoadSource::Structure,
            total_ms,
        });
    }

    // Escalate: Chrome renders JS, we extract structure — we do not paint pixels.
    let url_owned = url.to_string();
    let escalated = tokio::task::spawn_blocking(move || {
        // Local browser for one-shot if caller didn't keep one — but we need mut browser.
        // Handled below with browser option.
        extract_via_chrome_standalone(&url_owned)
    })
    .await
    .context("escalate task")??;

    // Prefer reusing browser if we add that later; standalone is fine for v1.
    let _ = browser; // reserved for sticky Chrome session

    let total_ms = start.elapsed().as_millis() as u64;
    let mut doc = escalated;
    doc.timing_ms.fetch_ms = total_ms;
    Ok(LoadedPage {
        doc,
        source: LoadSource::Escalated,
        total_ms,
    })
}

async fn load_structure(url: &str) -> Result<Document> {
    let fetched = fetch_url(url).await?;
    let mut doc = parse_html(&fetched.url, &fetched.body, fetched.fetch_ms);
    crate::parse::attach_known_forms(&mut doc);
    Ok(doc)
}

/// Thin page ⇒ likely JS shell or empty main (not merely a short article).
pub fn is_thin(doc: &Document) -> bool {
    if doc.blocks.is_empty() {
        return true;
    }
    let has_prose = doc.blocks.iter().any(|b| {
        matches!(
            b,
            crate::model::Block::Paragraph { .. }
                | crate::model::Block::ListItem { .. }
                | crate::model::Block::Heading { .. }
                | crate::model::Block::Pre { .. }
        )
    });
    // Short but real documents (example.com) are fine — don't escalate.
    if has_prose && doc.text_len() >= 40 {
        return false;
    }
    doc.text_len() < 40 && doc.links.len() < 3
}

/// Search engine results with almost no links in static HTML → need Chrome extract.
fn is_sparse_serp(doc: &Document) -> bool {
    let u = doc.url.to_ascii_lowercase();
    let looks_like_serp = u.contains("google.") && u.contains("/search")
        || u.contains("bing.com/search")
        || (u.contains("duckduckgo.") && u.contains("q="))
        || (u.contains("youtube.") && u.contains("search_query"));
    looks_like_serp && doc.links.len() < 12
}

fn extract_via_chrome_standalone(url: &str) -> Result<Document> {
    let browser = FullBrowser::launch()?;
    browser.extract_document(url)
}
