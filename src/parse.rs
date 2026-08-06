//! HTML → document model. No JS. Skip chrome (script/style/svg noise).

use crate::model::{Block, Document, Link, Ref, SearchForm, Span, Timing};
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

    let mut forms = extract_search_forms(&dom, url);
    // Known search engines: always expose a search box even if HTML hid the form.
    if forms.is_empty() {
        if let Some(f) = known_engine_form(url) {
            forms.push(f);
        }
    }

    let parse_ms = start.elapsed().as_millis() as u64;
    Document {
        url: url.to_string(),
        title,
        blocks,
        links,
        forms,
        timing_ms: Timing {
            fetch_ms,
            parse_ms,
            layout_ms: 0,
        },
    }
}

/// Pull GET search-like forms: input[type=search|text] named q/query/search…
fn extract_search_forms(dom: &Html, page_url: &str) -> Vec<SearchForm> {
    let Ok(form_sel) = Selector::parse("form") else {
        return vec![];
    };
    let mut out = Vec::new();

    for form in dom.select(&form_sel) {
        let method = form
            .value()
            .attr("method")
            .unwrap_or("get")
            .to_ascii_lowercase();
        if method != "get" {
            continue;
        }
        let action = form.value().attr("action").unwrap_or("").to_string();
        let action = if action.is_empty() {
            page_url.to_string()
        } else {
            action
        };

        let mut query_param: Option<String> = None;
        let mut placeholder = String::from("Search…");
        let mut hidden: Vec<(String, String)> = Vec::new();

        // Inputs inside form
        for input in form.select(&Selector::parse("input").unwrap()) {
            let name = input.value().attr("name").unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let itype = input
                .value()
                .attr("type")
                .unwrap_or("text")
                .to_ascii_lowercase();
            let value = input.value().attr("value").unwrap_or("").to_string();
            let ph = input.value().attr("placeholder").unwrap_or("");

            match itype.as_str() {
                "hidden" => hidden.push((name, value)),
                "search" | "text" => {
                    let is_query = matches!(
                        name.as_str(),
                        "q" | "query" | "search" | "search_query" | "p" | "text" | "wd" | "keyword"
                    ) || itype == "search"
                        || query_param.is_none();
                    if is_query && query_param.is_none() {
                        query_param = Some(name);
                        if !ph.is_empty() {
                            placeholder = ph.to_string();
                        } else if !value.is_empty() {
                            // keep empty for typing
                        }
                    } else if !matches!(itype.as_str(), "submit" | "button" | "image") {
                        // non-query text fields → hidden-ish defaults
                        if !value.is_empty() {
                            hidden.push((name, value));
                        }
                    }
                }
                "submit" | "button" | "image" | "checkbox" | "radio" => {}
                _ => {
                    if !value.is_empty() {
                        hidden.push((name, value));
                    }
                }
            }
        }

        // textarea named q etc.
        if query_param.is_none() {
            for ta in form.select(&Selector::parse("textarea").unwrap()) {
                let name = ta.value().attr("name").unwrap_or("");
                if matches!(name, "q" | "query" | "search") {
                    query_param = Some(name.to_string());
                    break;
                }
            }
        }

        let Some(query_param) = query_param else {
            continue;
        };

        // Prefer forms that look like site search
        let score = score_search_form(&action, &query_param, &placeholder);
        out.push((
            score,
            SearchForm {
                action,
                method: "get".into(),
                query_param,
                placeholder,
                hidden,
            },
        ));
    }

    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|(_, f)| f).take(3).collect()
}

fn score_search_form(action: &str, query_param: &str, placeholder: &str) -> i32 {
    let mut s = 0;
    let a = action.to_ascii_lowercase();
    let p = placeholder.to_ascii_lowercase();
    if query_param == "q" {
        s += 5;
    }
    if a.contains("search") {
        s += 4;
    }
    if p.contains("search") || p.contains("google") {
        s += 3;
    }
    if matches!(query_param, "query" | "search" | "search_query") {
        s += 3;
    }
    s
}

/// Ensure search engines always get a typeable form.
pub fn attach_known_forms(doc: &mut Document) {
    if doc.forms.is_empty() {
        if let Some(f) = known_engine_form(&doc.url) {
            doc.forms.push(f);
        }
    }
}

/// Hard-coded engines when markup is minimal / JS-only.
fn known_engine_form(page_url: &str) -> Option<SearchForm> {
    let u = page_url.to_ascii_lowercase();
    let host = url::Url::parse(page_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_default();

    if host.contains("google.") {
        return Some(SearchForm {
            action: "https://www.google.com/search".into(),
            method: "get".into(),
            query_param: "q".into(),
            placeholder: "Search Google…".into(),
            hidden: vec![],
        });
    }
    if host.contains("duckduckgo.") || u.contains("duckduckgo.com") {
        return Some(SearchForm {
            action: "https://duckduckgo.com/".into(),
            method: "get".into(),
            query_param: "q".into(),
            placeholder: "Search DuckDuckGo…".into(),
            hidden: vec![],
        });
    }
    if host.contains("bing.") {
        return Some(SearchForm {
            action: "https://www.bing.com/search".into(),
            method: "get".into(),
            query_param: "q".into(),
            placeholder: "Search Bing…".into(),
            hidden: vec![],
        });
    }
    if host.contains("youtube.") {
        return Some(SearchForm {
            action: "https://www.youtube.com/results".into(),
            method: "get".into(),
            query_param: "search_query".into(),
            placeholder: "Search YouTube…".into(),
            hidden: vec![],
        });
    }
    None
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

    #[test]
    fn google_gets_search_form() {
        let doc = parse_html("https://www.google.com/", "<html><body></body></html>", 1);
        assert!(!doc.forms.is_empty());
        let url = doc.search_url("rust lang").unwrap();
        assert!(url.contains("google.com/search"));
        assert!(url.contains("q=rust"));
    }

    #[test]
    fn parses_form_with_q() {
        let html = r#"
        <html><body>
          <form action="/search" method="get">
            <input type="text" name="q" placeholder="Search site">
            <input type="hidden" name="hl" value="en">
          </form>
        </body></html>
        "#;
        let doc = parse_html("https://example.com/", html, 1);
        assert_eq!(doc.forms[0].query_param, "q");
        assert_eq!(doc.forms[0].placeholder, "Search site");
        let u = doc.search_url("hello").unwrap();
        assert!(u.contains("q=hello"));
        assert!(u.contains("hl=en"));
    }
}
