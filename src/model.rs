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
    /// Wall-clock fetch + parse timing (ms).
    pub timing_ms: Timing,
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
