//! Session browser — **custom only**: HTTPS fetch → HTML parse → Document.
//! No headless Chrome. No pixel paint. Same model for TUI + agents.

use crate::fetch::fetch_url;
use crate::model::{Block, Document, Span};
use crate::parse::{self, parse_html};
use crate::urlutil::ensure_http_url;
use anyhow::{Context, Result};
use std::time::Instant;

/// How the page was obtained (always structure in the custom engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadSource {
    /// HTTPS + custom HTML → block model.
    Structure,
}

#[derive(Debug, Clone)]
pub struct LoadedPage {
    pub doc: Document,
    pub source: LoadSource,
    pub total_ms: u64,
}

pub struct Session {
    history: Vec<LoadedPage>,
    cursor: usize,
}

impl Session {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            cursor: 0,
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

    pub async fn open(&mut self, url: &str) -> Result<&LoadedPage> {
        let url = ensure_http_url(url)?;
        let page = load_page(&url).await?;
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
        let page = load_page(&url).await?;
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

/// Custom pipeline only: normalize URL → fetch → parse → annotate.
pub async fn load_page(url: &str) -> Result<LoadedPage> {
    let start = Instant::now();
    let mut doc = load_structure(url).await?;
    parse::annotate_if_captcha(&mut doc);
    annotate_if_sparse_js(&mut doc);

    let total_ms = start.elapsed().as_millis() as u64;
    Ok(LoadedPage {
        doc,
        source: LoadSource::Structure,
        total_ms,
    })
}

async fn load_structure(url: &str) -> Result<Document> {
    let url = normalize_search_url(url);
    let fetched = fetch_url(&url).await?;
    let body_l = fetched.body.to_ascii_lowercase();
    let mut doc = parse_html(&fetched.url, &fetched.body, fetched.fetch_ms);
    parse::attach_known_forms(&mut doc);

    let blocked = body_l.contains("captcha")
        || body_l.contains("unusual traffic")
        || body_l.contains("trouble accessing google")
        || body_l.contains("bots use duckduckgo")
        || body_l.contains("our systems have detected")
        || body_l.contains("complete the following challenge")
        || fetched.url.to_ascii_lowercase().contains("/sorry/");
    if blocked {
        doc.title = "CAPTCHA".into();
    }
    Ok(doc)
}

/// Google basic HTML when searching (no browser engine required).
fn normalize_search_url(url: &str) -> String {
    let Ok(mut u) = url::Url::parse(url) else {
        return url.to_string();
    };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    if host.contains("google.") && u.path().contains("search") {
        let mut pairs: Vec<(String, String)> = u
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        pairs.retain(|(k, _)| k == "q" || k == "hl" || k == "gbv" || k == "start" || k == "tbm");
        if !pairs.iter().any(|(k, _)| k == "gbv") {
            pairs.push(("gbv".into(), "1".into()));
        }
        {
            let mut ser = u.query_pairs_mut();
            ser.clear();
            for (k, v) in &pairs {
                ser.append_pair(k, v);
            }
        }
        return u.to_string();
    }
    url.to_string()
}

/// JS-heavy shell with almost no HTML content — explain, don't launch Chrome.
fn annotate_if_sparse_js(doc: &mut Document) {
    if doc.looks_like_captcha() {
        return;
    }
    if !is_thin(doc) {
        return;
    }
    // Keep forms (search home) — empty Google shell with a search form is fine.
    if doc.primary_search().is_some() && doc.is_search_home() {
        return;
    }
    if doc.text_len() >= 40 || doc.links.len() >= 5 {
        return;
    }

    let url = doc.url.clone();
    doc.blocks = vec![
        Block::Heading {
            level: 1,
            text: "Sparse page (custom HTML engine)".into(),
        },
        Block::Spacer,
        Block::Paragraph {
            spans: vec![Span::Text {
                text: "This site ships almost no content in static HTML (JS app shell). \
termbrowse is a custom structure browser — it does not run a browser engine."
                    .into(),
            }],
        },
        Block::Spacer,
        Block::Paragraph {
            spans: vec![Span::Text {
                text: "Works great: docs, blogs, HTML search (DuckDuckGo HTML, Google gbv=1), most marketing pages."
                    .into(),
            }],
        },
        Block::ListItem {
            spans: vec![Span::Text {
                text: "Try: https://html.duckduckgo.com/html/".into(),
            }],
            index: 0,
        },
        Block::ListItem {
            spans: vec![Span::Text {
                text: "Try: https://doc.rust-lang.org/book/".into(),
            }],
            index: 0,
        },
        Block::Spacer,
        Block::Paragraph {
            spans: vec![Span::Text {
                text: format!("URL: {url}"),
            }],
        },
    ];
}

fn is_thin(doc: &Document) -> bool {
    if doc.blocks.is_empty() {
        return true;
    }
    let has_prose = doc.blocks.iter().any(|b| {
        matches!(
            b,
            Block::Paragraph { .. }
                | Block::ListItem { .. }
                | Block::Heading { .. }
                | Block::Pre { .. }
        )
    });
    if has_prose && doc.text_len() >= 40 {
        return false;
    }
    doc.text_len() < 40 && doc.links.len() < 3
}
