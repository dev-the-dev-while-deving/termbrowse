//! Structured page model. Roles, not pixels.

use crate::serp::Serp;
use ratatui::style::Color;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Ref(pub u32);

impl std::fmt::Display for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "e{}", self.0)
    }
}

/// Stolen site colors. Canvas / body text stay product-owned.
#[derive(Debug, Clone, Default)]
pub struct SiteIdentity {
    pub link: Option<Color>,
    pub heading: Option<Color>,
    pub accent: Option<Color>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub url: String,
    pub title: String,
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
    #[serde(default)]
    pub nav: Vec<NavItem>,
    #[serde(default)]
    pub forms: Vec<SearchForm>,
    #[serde(default)]
    pub site_search: Option<SearchForm>,
    pub timing_ms: Timing,
    #[serde(skip)]
    pub identity: SiteIdentity,
    #[serde(skip)]
    pub serp: Serp,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchForm {
    pub action: String,
    pub method: String,
    pub query_param: String,
    pub placeholder: String,
    #[serde(default)]
    pub hidden: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Timing {
    pub fetch_ms: u64,
    pub parse_ms: u64,
    pub layout_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Link {
    pub r#ref: Ref,
    pub href: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NavItem {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    Heading {
        level: u8,
        text: String,
        #[serde(default)]
        id: Option<String>,
    },
    Paragraph { spans: Vec<Span> },
    ListItem { spans: Vec<Span>, index: u32 },
    Pre { text: String },
    Quote { spans: Vec<Span> },
    Hr,
    Spacer,
    Image { alt: String },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Frame {
        title: Option<String>,
        lines: Vec<Vec<Span>>,
    },
    Caption { text: String },
    /// Unicode preview of TeX / MathML (not a pixel formula).
    Math { display: bool, preview: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Span {
    Text { text: String },
    Strong { text: String },
    Em { text: String },
    Code { text: String },
    Link { r#ref: Ref, text: String },
}

impl Document {
    pub fn resolve_link(&self, r: Ref) -> Option<&Link> {
        self.links.iter().find(|l| l.r#ref == r)
    }

    pub fn ddg_search_url(query: &str) -> String {
        let mut u = Url::parse("https://html.duckduckgo.com/html/").expect("ddg url");
        u.query_pairs_mut().append_pair("q", query.trim());
        u.to_string()
    }

    #[allow(dead_code)]
    pub fn search_url(&self, query: &str) -> Option<String> {
        let q = query.trim();
        if q.is_empty() {
            None
        } else {
            Some(Self::ddg_search_url(q))
        }
    }

    pub fn primary_search(&self) -> Option<&SearchForm> {
        self.forms.first()
    }

    pub fn site_search_url(&self, query: &str) -> Option<String> {
        let form = self.site_search.as_ref()?;
        let q = query.trim();
        if q.is_empty() {
            return None;
        }
        let mut u = Url::parse(&form.action).ok()?;
        {
            let mut pairs = u.query_pairs_mut();
            pairs.append_pair(&form.query_param, q);
            for (k, v) in &form.hidden {
                pairs.append_pair(k, v);
            }
        }
        Some(u.to_string())
    }

    pub fn is_search_home(&self) -> bool {
        if self.looks_like_captcha()
            || self.title.contains("CAPTCHA")
            || self.title.contains("Blocked")
        {
            return true;
        }
        if let Ok(u) = Url::parse(&self.url) {
            let host = u.host_str().unwrap_or("").to_ascii_lowercase();
            if host.contains("duckduckgo.") {
                let no_q = u.query_pairs().all(|(k, _)| k != "q");
                let path = u.path();
                let path_ok = path.is_empty() || path == "/" || path.starts_with("/html");
                return path_ok && no_q;
            }
        }
        self.primary_search().is_some() && self.text_len() < 400 && self.links.len() < 40
    }

    pub fn wants_centered_search(&self) -> bool {
        (self.is_search_home() || self.looks_like_captcha()) && !self.is_serp()
    }

    pub fn is_serp(&self) -> bool {
        !self.serp.hits.is_empty()
    }

    pub fn looks_like_captcha(&self) -> bool {
        let blob = format!(
            "{} {}",
            self.title.to_ascii_lowercase(),
            self.blocks
                .iter()
                .take(20)
                .map(block_plain)
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
        );
        blob.contains("captcha")
            || blob.contains("unusual traffic")
            || blob.contains("not a robot")
            || blob.contains("recaptcha")
            || blob.contains("trouble accessing google")
            || blob.contains("bots use duckduckgo")
            || blob.contains("complete the following challenge")
            || blob.contains("our systems have detected")
            || (blob.contains("enable javascript")
                && self.url.to_ascii_lowercase().contains("google."))
            || self.url.to_ascii_lowercase().contains("/sorry/")
    }

    pub fn text_len(&self) -> usize {
        self.blocks.iter().map(|b| block_plain(b).chars().count()).sum()
    }
}

pub fn block_plain(b: &Block) -> String {
    match b {
        Block::Heading { text, .. }
        | Block::Pre { text }
        | Block::Caption { text }
        | Block::Image { alt: text }
        | Block::Math { preview: text, .. } => text.clone(),
        Block::Paragraph { spans } | Block::ListItem { spans, .. } | Block::Quote { spans } => {
            spans_plain(spans)
        }
        Block::Frame { title, lines } => {
            let mut s = title.clone().unwrap_or_default();
            for line in lines {
                s.push(' ');
                s.push_str(&spans_plain(line));
            }
            s
        }
        Block::Table { headers, rows } => {
            let mut s = headers.join(" ");
            for r in rows {
                s.push(' ');
                s.push_str(&r.join(" "));
            }
            s
        }
        Block::Hr | Block::Spacer => String::new(),
    }
}

pub fn spans_plain(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|s| match s {
            Span::Text { text }
            | Span::Strong { text }
            | Span::Em { text }
            | Span::Code { text }
            | Span::Link { text, .. } => text.as_str(),
        })
        .collect()
}
