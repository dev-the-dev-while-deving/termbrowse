//! Core document model — the browser's truth is a tree + agent refs, not pixels.

use serde::Serialize;
use url::Url;

/// Stable handle agents (and the TUI) use to point at interactive things.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Ref(pub u32);

impl std::fmt::Display for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "e{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub url: String,
    pub title: String,
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
    /// Search / GET forms discovered on the page (Google-style search box).
    #[serde(default)]
    pub forms: Vec<SearchForm>,
    /// Wall-clock fetch + parse timing (ms).
    pub timing_ms: Timing,
}

/// A typeable search form (GET). Primary query field + hidden params.
#[derive(Debug, Clone, Serialize)]
pub struct SearchForm {
    /// Absolute or relative form action URL.
    pub action: String,
    /// Method (only `get` is submitted for now).
    pub method: String,
    /// Name of the query parameter (e.g. `q`).
    pub query_param: String,
    pub placeholder: String,
    /// Extra hidden fields to include on submit.
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph {
        spans: Vec<Span>,
    },
    ListItem {
        spans: Vec<Span>,
    },
    Pre {
        text: String,
    },
    Quote {
        spans: Vec<Span>,
    },
    Hr,
    /// Blank vertical gap (after blocks).
    Spacer,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Span {
    Text { text: String },
    Link { r#ref: Ref, text: String },
}

impl Document {
    pub fn resolve_link(&self, r: Ref) -> Option<&Link> {
        self.links.iter().find(|l| l.r#ref == r)
    }

    pub fn resolve_href(&self, href: &str) -> Option<Url> {
        Url::parse(&self.url)
            .ok()
            .and_then(|base| base.join(href).ok())
            .or_else(|| Url::parse(href).ok())
    }

    /// Primary search form, if any.
    pub fn primary_search(&self) -> Option<&SearchForm> {
        self.forms.first()
    }

    /// Homepage-style search: centered box UI (Google / DDG / empty search roots).
    pub fn is_search_home(&self) -> bool {
        if self.primary_search().is_none() {
            return false;
        }
        if let Ok(u) = Url::parse(&self.url) {
            let path = u.path();
            let path_ok = path.is_empty() || path == "/";
            // No results query yet
            let no_q = u.query_pairs().all(|(k, _)| k != "q" && k != "search_query");
            if path_ok && no_q {
                let host = u.host_str().unwrap_or("");
                if host.contains("google.")
                    || host.contains("duckduckgo.")
                    || host.contains("bing.")
                    || host.contains("youtube.")
                {
                    return true;
                }
            }
        }
        // Generic: form + sparse body
        self.text_len() < 400 && self.links.len() < 40
    }

    /// Build absolute results URL for a typed query (GET only).
    pub fn search_url(&self, query: &str) -> Option<String> {
        let form = self.primary_search()?;
        if !form.method.eq_ignore_ascii_case("get") {
            return None;
        }
        let base = self.resolve_href(&form.action)?;
        let mut pairs: Vec<(String, String)> = form.hidden.clone();
        pairs.push((form.query_param.clone(), query.to_string()));
        // url crate query pairs
        let mut url = base;
        {
            let mut ser = url.query_pairs_mut();
            ser.clear();
            for (k, v) in &pairs {
                ser.append_pair(k, v);
            }
        }
        Some(url.to_string())
    }

    /// Rough content weight for thin-page detection / UI.
    pub fn text_len(&self) -> usize {
        self.blocks
            .iter()
            .map(|b| match b {
                Block::Heading { text, .. } | Block::Pre { text } => text.chars().count(),
                Block::Paragraph { spans }
                | Block::ListItem { spans }
                | Block::Quote { spans } => spans
                    .iter()
                    .map(|s| match s {
                        Span::Text { text } | Span::Link { text, .. } => text.chars().count(),
                    })
                    .sum(),
                Block::Hr | Block::Spacer => 0,
            })
            .sum()
    }
}
