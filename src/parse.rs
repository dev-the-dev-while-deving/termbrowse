//! HTML → document model. No JS. Skip chrome (script/style/svg noise).

use crate::model::{Block, Document, Link, Ref, Span, Timing};
use scraper::{Html, Node, Selector};
use std::time::Instant;

const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "svg", "template", "iframe", "object", "embed",
];

pub fn parse_html(url: &str, html: &str, fetch_ms: u64) -> Document {
    let start = Instant::now();
    let dom = Html::parse_document(html);

    let title = dom
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|t| normalize_ws(&t.text().collect::<String>()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| url.to_string());

    let mut blocks = Vec::new();
    let mut links = Vec::new();
    let mut next_ref = 1u32;

    // Prefer <main> / <article>, else body.
    let root = Selector::parse("main, article, body").unwrap();
    let roots: Vec<_> = dom.select(&root).collect();
    let scope = roots
        .iter()
        .find(|e| e.value().name() == "main")
        .or_else(|| roots.iter().find(|e| e.value().name() == "article"))
        .or_else(|| roots.iter().find(|e| e.value().name() == "body"));

    if let Some(scope) = scope {
        walk_element(scope, &mut blocks, &mut links, &mut next_ref, 0);
    }

    // Collapse runs of spacers.
    blocks = collapse_spacers(blocks);

    let parse_ms = start.elapsed().as_millis() as u64;
    Document {
        url: url.to_string(),
        title,
        blocks,
        links,
        timing_ms: Timing {
            fetch_ms,
            parse_ms,
            layout_ms: 0,
        },
    }
}

fn walk_element(
    el: &scraper::ElementRef<'_>,
    blocks: &mut Vec<Block>,
    links: &mut Vec<Link>,
    next_ref: &mut u32,
    list_depth: u8,
) {
    let name = el.value().name();

    if SKIP_TAGS.contains(&name) {
        return;
    }

    // Landmark / non-content chrome often living in body.
    if matches!(name, "nav" | "footer" | "aside") && list_depth == 0 {
        // Still allow a short pass for nav links? Skip for cleaner reader v0.
        return;
    }

    match name {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = name.as_bytes()[1] - b'0';
            let text = normalize_ws(&plain_text(el));
            if !text.is_empty() {
                blocks.push(Block::Heading { level, text });
                blocks.push(Block::Spacer);
            }
        }
        "p" => {
            let spans = collect_spans(el, links, next_ref);
            if !spans_empty(&spans) {
                blocks.push(Block::Paragraph { spans });
                blocks.push(Block::Spacer);
            }
        }
        "li" => {
            let spans = collect_spans(el, links, next_ref);
            if !spans_empty(&spans) {
                blocks.push(Block::ListItem { spans });
            }
        }
        "pre" => {
            let text = el.text().collect::<String>();
            if !text.trim().is_empty() {
                blocks.push(Block::Pre {
                    text: text.trim_end().to_string(),
                });
                blocks.push(Block::Spacer);
            }
        }
        "blockquote" => {
            let spans = collect_spans(el, links, next_ref);
            if !spans_empty(&spans) {
                blocks.push(Block::Quote { spans });
                blocks.push(Block::Spacer);
            }
        }
        "hr" => {
            blocks.push(Block::Hr);
            blocks.push(Block::Spacer);
        }
        "br" => {
            blocks.push(Block::Spacer);
        }
        "a" => {
            // Standalone link block if direct child of flow container handled via spans.
            // When walk hits <a> as a block-level-ish node under body/div, treat as paragraph.
            if is_blockish_parent(el) {
                let spans = collect_spans(el, links, next_ref);
                if !spans_empty(&spans) {
                    blocks.push(Block::Paragraph { spans });
                    blocks.push(Block::Spacer);
                }
            } else {
                for child in el.children() {
                    if let Some(child_el) = scraper::ElementRef::wrap(child) {
                        walk_element(&child_el, blocks, links, next_ref, list_depth);
                    }
                }
            }
        }
        "ul" | "ol" => {
            for child in el.children() {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    walk_element(&child_el, blocks, links, next_ref, list_depth.saturating_add(1));
                }
            }
            blocks.push(Block::Spacer);
        }
        "div" | "section" | "main" | "article" | "body" | "span" | "header" | "figure"
        | "figcaption" | "td" | "th" | "tr" | "table" | "tbody" | "thead" | "dl" | "dt"
        | "dd" | "form" | "label" | "button" | "details" | "summary" | "center" | "font" => {
            for child in el.children() {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    walk_element(&child_el, blocks, links, next_ref, list_depth);
                } else if let Node::Text(t) = child.value() {
                    let text = normalize_ws(t);
                    if !text.is_empty() && is_blockish_parent(el) {
                        // Loose text under div.
                        blocks.push(Block::Paragraph {
                            spans: vec![Span::Text { text }],
                        });
                        blocks.push(Block::Spacer);
                    }
                }
            }
        }
        _ => {
            for child in el.children() {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    walk_element(&child_el, blocks, links, next_ref, list_depth);
                }
            }
        }
    }
}

fn is_blockish_parent(el: &scraper::ElementRef<'_>) -> bool {
    el.parent()
        .and_then(|p| match p.value() {
            Node::Element(e) => Some(e.name()),
            _ => None,
        })
        .map(|n| {
            matches!(
                n,
                "body" | "main" | "article" | "section" | "div" | "td" | "li" | "dd" | "blockquote"
            )
        })
        .unwrap_or(true)
}

fn collect_spans(
    el: &scraper::ElementRef<'_>,
    links: &mut Vec<Link>,
    next_ref: &mut u32,
) -> Vec<Span> {
    let mut spans = Vec::new();
    collect_spans_into(el, links, next_ref, &mut spans);
    merge_text_spans(spans)
}

fn collect_spans_into(
    el: &scraper::ElementRef<'_>,
    links: &mut Vec<Link>,
    next_ref: &mut u32,
    out: &mut Vec<Span>,
) {
    let name = el.value().name();
    if SKIP_TAGS.contains(&name) {
        return;
    }

    if name == "a" {
        let href = el.value().attr("href").unwrap_or("").trim().to_string();
        let text = normalize_ws(&plain_text(el));
        if text.is_empty() && href.is_empty() {
            return;
        }
        let label = if text.is_empty() {
            href.clone()
        } else {
            text
        };
        if href.starts_with('#') || href.starts_with("javascript:") {
            out.push(Span::Text { text: label });
            return;
        }
        let r = Ref(*next_ref);
        *next_ref += 1;
        links.push(Link {
            r#ref: r,
            href,
            text: label.clone(),
        });
        out.push(Span::Link {
            r#ref: r,
            text: label,
        });
        return;
    }

    if name == "br" {
        out.push(Span::Text {
            text: " ".to_string(),
        });
        return;
    }

    for child in el.children() {
        match child.value() {
            Node::Text(t) => {
                let raw = t.to_string();
                // Preserve single spaces; collapse later via normalize on merge.
                if !raw.is_empty() {
                    out.push(Span::Text { text: raw });
                }
            }
            Node::Element(_) => {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    collect_spans_into(&child_el, links, next_ref, out);
                }
            }
            _ => {}
        }
    }
}

fn plain_text(el: &scraper::ElementRef<'_>) -> String {
    el.text().collect::<String>()
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn spans_empty(spans: &[Span]) -> bool {
    spans.iter().all(|s| match s {
        Span::Text { text } => text.split_whitespace().next().is_none(),
        Span::Link { text, .. } => text.is_empty(),
    })
}

fn merge_text_spans(spans: Vec<Span>) -> Vec<Span> {
    let mut out = Vec::new();
    for span in spans {
        match span {
            Span::Text { text } => {
                if let Some(Span::Text { text: prev }) = out.last_mut() {
                    prev.push_str(&text);
                } else {
                    out.push(Span::Text { text });
                }
            }
            other => out.push(other),
        }
    }
    // Normalize whitespace inside text spans but keep structure.
    out.into_iter()
        .filter_map(|s| match s {
            Span::Text { text } => {
                let t = normalize_ws(&text);
                if t.is_empty() {
                    None
                } else {
                    Some(Span::Text { text: t })
                }
            }
            Span::Link { r#ref, text } => {
                let t = normalize_ws(&text);
                if t.is_empty() {
                    None
                } else {
                    Some(Span::Link { r#ref, text: t })
                }
            }
        })
        .collect()
}

fn collapse_spacers(blocks: Vec<Block>) -> Vec<Block> {
    let mut out = Vec::new();
    let mut last_spacer = true; // don't start with spacer
    for b in blocks {
        match b {
            Block::Spacer => {
                if !last_spacer {
                    out.push(Block::Spacer);
                    last_spacer = true;
                }
            }
            other => {
                out.push(other);
                last_spacer = false;
            }
        }
    }
    while matches!(out.last(), Some(Block::Spacer)) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_article() {
        let html = r#"
        <html><head><title>Hello</title></head>
        <body>
          <main>
            <h1>Title</h1>
            <p>Read <a href="https://example.com/more">more</a> here.</p>
            <ul><li>One</li><li><a href="/two">Two</a></li></ul>
          </main>
        </body></html>
        "#;
        let doc = parse_html("https://example.com/", html, 1);
        assert_eq!(doc.title, "Hello");
        assert!(doc.links.len() >= 2);
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Heading { level: 1, .. })));
    }
}
