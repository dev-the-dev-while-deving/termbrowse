//! Document model: browser-like *roles*, not pixels.
//! Classification (what is it?) is separate from presentation (how we draw it).

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
    #[serde(default)]
    pub forms: Vec<SearchForm>,
    pub timing_ms: Timing,
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

/// Structural units — how a browser would *treat* the node, not its CSS pixels.
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
        /// Ordered list number; 0 = bullet.
        #[serde(default)]
        index: u32,
    },
    /// Code / pre — always drawn with a box border (monospace region).
    Pre {
        text: String,
    },
    /// Blockquote — left bar (browser quote treatment).
    Quote {
        spans: Vec<Span>,
    },
    /// Horizontal rule.
    Hr,
    Spacer,
    /// Image -> metadata and URL for rendering.
    Image {
        src: String,
        alt: String,
        width: Option<u32>,
        height: Option<u32>,
    },
    /// Table — minimal grid with borders.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// Bordered container (fieldset, figure, card-like boxes, explicit border CSS).
    /// Children already flattened into `inner` lines of spans.
    Frame {
        title: Option<String>,
        lines: Vec<Vec<Span>>,
    },
    /// Inline caption under a figure/table.
    Caption {
        text: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Span {
    Text { text: String },
    /// Browser <strong>/<b>
    Strong { text: String },
    /// Browser <em>/<i>
    Em { text: String },
    /// Browser <code>
    Code { text: String },
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

    pub fn primary_search(&self) -> Option<&SearchForm> {
        self.forms.first()
    }

    pub fn is_search_home(&self) -> bool {
        if self.primary_search().is_none() {
            return false;
        }
        if self.looks_like_captcha()
            || self.title.contains("CAPTCHA")
            || self.title.contains("Blocked")
        {
            return true;
        }
        if let Ok(u) = Url::parse(&self.url) {
            let path = u.path();
            let path_ok = path.is_empty() || path == "/" || path.starts_with("/html");
            let no_q = u
                .query_pairs()
                .all(|(k, _)| k != "q" && k != "search_query" && k != "p");
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
        self.text_len() < 400 && self.links.len() < 40
    }

    pub fn wants_centered_search(&self) -> bool {
        self.primary_search().is_some()
            && (self.is_search_home() || self.looks_like_captcha())
    }

    pub fn search_url(&self, query: &str) -> Option<String> {
        let form = self.primary_search()?;
        if !form.method.eq_ignore_ascii_case("get") {
            return None;
        }
        let base = self.resolve_href(&form.action)?;
        let mut pairs: Vec<(String, String)> = form.hidden.clone();
        pairs.push((form.query_param.clone(), query.to_string()));

        let host = base.host_str().unwrap_or("").to_ascii_lowercase();
        if host.contains("google.") {
            pairs.retain(|(k, _)| {
                let k = k.as_str();
                k == "q" || k == "hl" || k == "gbv" || k == "ie"
            });
            if !pairs.iter().any(|(k, _)| k == "gbv") {
                pairs.push(("gbv".into(), "1".into()));
            }
            if !pairs.iter().any(|(k, _)| k == "hl") {
                pairs.push(("hl".into(), "en".into()));
            }
        }

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

    pub fn looks_like_captcha(&self) -> bool {
        let blob = format!(
            "{} {}",
            self.title.to_ascii_lowercase(),
            self.blocks
                .iter()
                .take(20)
                .map(|b| block_plain(b))
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

fn block_plain(b: &Block) -> String {
    match b {
        Block::Heading { text, .. }
        | Block::Pre { text }
        | Block::Caption { text }
        | Block::Image { alt: text, .. } => text.clone(),
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

fn spans_plain(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|s| match s {
            Span::Text { text }
            | Span::Strong { text }
            | Span::Em { text }
            | Span::Code { text }
            | Span::Link { text, .. } => text.as_str(),
        })
        .collect::<Vec<_>>()
        .join("")
}
