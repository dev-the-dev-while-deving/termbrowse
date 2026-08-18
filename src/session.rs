//! History + load. Structure pipeline only.

use crate::fetch::fetch_url;
use crate::model::Document;
use crate::parse::{self, parse_html};
use crate::urlutil::{ensure_http_url, normalize_search_url};
use anyhow::{Context, Result};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct LoadedPage {
    pub doc: Document,
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

    #[allow(dead_code)]
    pub fn pages(&self) -> &[LoadedPage] {
        &self.history
    }

    #[allow(dead_code)]
    pub fn cursor(&self) -> usize {
        self.cursor
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
        self.history[self.cursor] = load_page(&url).await?;
        Ok(self.current().expect("cursor valid"))
    }

    pub async fn follow_href(&mut self, href: &str) -> Result<&LoadedPage> {
        let base = self
            .current()
            .map(|p| p.doc.url.clone())
            .unwrap_or_default();
        let abs = crate::urlutil::resolve_and_unwrap(&base, href)?;
        self.open(&abs).await
    }
}

pub async fn load_page(url: &str) -> Result<LoadedPage> {
    let start = Instant::now();
    let url = normalize_search_url(url);
    let fetched = fetch_url(&url).await?;
    let mut doc = if crate::md::looks_like_markdown_file(
        &fetched.url,
        &fetched.content_type,
        &fetched.body,
    ) {
        parse::parse_markdown(&fetched.url, &fetched.body, fetched.fetch_ms)
    } else {
        parse_html(&fetched.url, &fetched.body, fetched.fetch_ms)
    };
    if fetched.status >= 400 {
        // still parse; captcha / error HTML is useful
    }
    parse::annotate_if_captcha(&mut doc);
    parse::annotate_if_sparse(&mut doc);
    Ok(LoadedPage {
        doc,
        total_ms: start.elapsed().as_millis() as u64,
    })
}
