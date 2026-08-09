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
/// Attach search forms. **Google/Bing are rewired to DuckDuckGo HTML** —
/// those engines CAPTCHA terminal clients; we don't pretend otherwise.
pub fn attach_known_forms(doc: &mut Document) {
    let host = url::Url::parse(&doc.url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_default();

    // Captcha-prone search hosts: always use our safe DDG form.
    if host.contains("google.") || host.contains("bing.") {
        doc.forms = vec![safe_search_form()];
        return;
    }
    if host.contains("duckduckgo.") {
        doc.forms = vec![safe_search_form()];
        return;
    }
    if doc.forms.is_empty() {
        if let Some(f) = known_engine_form(&doc.url) {
            doc.forms.push(f);
        }
    }
}

/// Default terminal-safe search (DuckDuckGo HTML lite).
pub fn safe_search_form() -> SearchForm {
    SearchForm {
        action: "https://html.duckduckgo.com/html/".into(),
        method: "get".into(),
        query_param: "q".into(),
        placeholder: "Search the web (DuckDuckGo)…".into(),
        hidden: vec![],
    }
}

/// Hard-coded engines when markup is minimal / JS-only.
fn known_engine_form(page_url: &str) -> Option<SearchForm> {
    let u = page_url.to_ascii_lowercase();
    let host = url::Url::parse(page_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_default();

    // Google / Bing → DDG (CAPTCHA avoidance)
    if host.contains("google.") || host.contains("bing.") {
        return Some(safe_search_form());
    }
    if host.contains("duckduckgo.") || u.contains("duckduckgo.com") {
        return Some(safe_search_form());
    }
    if host.contains("youtube.") {
        // YouTube results also bot-wall often; keep form but warn via placeholder.
        return Some(SearchForm {
            action: "https://html.duckduckgo.com/html/".into(),
            method: "get".into(),
            query_param: "q".into(),
            placeholder: "Search via DuckDuckGo (YouTube blocks terminals)…".into(),
            hidden: vec![],
        });
    }
    None
}

/// Replace CAPTCHA / sorry pages with a clear terminal-native explanation.
pub fn annotate_if_captcha(doc: &mut Document) {
    if !doc.looks_like_captcha() {
        let u = doc.url.to_ascii_lowercase();
        if !u.contains("/sorry/") && !u.contains("captcha") {
            return;
        }
    }
    let original = doc.url.clone();
    doc.title = "Blocked by CAPTCHA".into();
    doc.blocks = vec![
        Block::Heading {
            level: 1,
            text: "Blocked by CAPTCHA".into(),
        },
        Block::Spacer,
        Block::Paragraph {
            spans: vec![Span::Text {
                text: "This is not a termbrowse bug. Google, Bing, and sometimes DuckDuckGo \
treat terminal clients as bots and demand a human CAPTCHA. We cannot solve CAPTCHAs here."
                    .into(),
            }],
        },
        Block::Spacer,
        Block::Paragraph {
            spans: vec![Span::Strong {
                text: "What to do:".into(),
            }],
        },
        Block::ListItem {
            spans: vec![Span::Text {
                text: "Type a query in the search box below — searches go through DuckDuckGo HTML."
                    .into(),
            }],
            index: 0,
        },
        Block::ListItem {
            spans: vec![Span::Text {
                text: "Or press H for Start Page and open docs / HN / Wikipedia (no CAPTCHA)."
                    .into(),
            }],
            index: 0,
        },
        Block::ListItem {
            spans: vec![Span::Text {
                text: "Avoid google.com in the terminal — they will keep blocking you."
                    .into(),
            }],
            index: 0,
        },
        Block::Spacer,
        Block::Paragraph {
            spans: vec![Span::Text {
                text: format!("Blocked URL: {original}"),
            }],
        },
    ];
    doc.forms = vec![safe_search_form()];
    doc.links.clear();
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
        return;
    }

    // Bordered containers: fieldset, figure, or style/class hints of a box.
    if matches!(name, "fieldset" | "figure") || looks_bordered(el) {
        let title = if name == "fieldset" {
            el.select(&Selector::parse("legend").unwrap())
                .next()
                .map(|l| normalize_ws(&plain_text(&l)))
                .filter(|s| !s.is_empty())
        } else if name == "figure" {
            el.select(&Selector::parse("figcaption").unwrap())
                .next()
                .map(|l| normalize_ws(&plain_text(&l)))
                .filter(|s| !s.is_empty())
        } else {
            None
        };
        let mut inner_blocks = Vec::new();
        for child in el.children() {
            if let Some(child_el) = scraper::ElementRef::wrap(child) {
                let cn = child_el.value().name();
                if cn == "legend" || cn == "figcaption" {
                    continue;
                }
                walk_element(&child_el, &mut inner_blocks, links, next_ref, list_depth);
            }
        }
        let lines = blocks_to_frame_lines(&inner_blocks);
        if !lines.is_empty() || title.is_some() {
            blocks.push(Block::Frame { title, lines });
            blocks.push(Block::Spacer);
        }
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
                blocks.push(Block::ListItem { spans, index: 0 });
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
        "img" => {
            let alt = el
                .value()
                .attr("alt")
                .or_else(|| el.value().attr("title"))
                .unwrap_or("")
                .trim()
                .to_string();
            blocks.push(Block::Image { alt });
            blocks.push(Block::Spacer);
        }
        "table" => {
            if let Some(t) = parse_table(el) {
                blocks.push(t);
                blocks.push(Block::Spacer);
            }
        }
        "a" => {
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
        "ul" => {
            for child in el.children() {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    walk_element(&child_el, blocks, links, next_ref, list_depth.saturating_add(1));
                }
            }
            blocks.push(Block::Spacer);
        }
        "ol" => {
            let mut i = 1u32;
            for child in el.children() {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    if child_el.value().name() == "li" {
                        let spans = collect_spans(&child_el, links, next_ref);
                        if !spans_empty(&spans) {
                            blocks.push(Block::ListItem { spans, index: i });
                            i += 1;
                        }
                    } else {
                        walk_element(
                            &child_el,
                            blocks,
                            links,
                            next_ref,
                            list_depth.saturating_add(1),
                        );
                    }
                }
            }
            blocks.push(Block::Spacer);
        }
        "figcaption" => {
            let text = normalize_ws(&plain_text(el));
            if !text.is_empty() {
                blocks.push(Block::Caption { text });
            }
        }
        "div" | "section" | "main" | "article" | "body" | "span" | "header" | "td" | "th"
        | "tr" | "tbody" | "thead" | "dl" | "dt" | "dd" | "form" | "label" | "button"
        | "details" | "summary" | "center" | "font" => {
            for child in el.children() {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    walk_element(&child_el, blocks, links, next_ref, list_depth);
                } else if let Node::Text(t) = child.value() {
                    let text = normalize_ws(t);
                    if !text.is_empty() && is_blockish_parent(el) {
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

/// Browser would draw a box (minimal CSS/class heuristic — not full CSS).
fn looks_bordered(el: &scraper::ElementRef<'_>) -> bool {
    let style = el.value().attr("style").unwrap_or("").to_ascii_lowercase();
    if style.contains("border") && !style.contains("border:none") && !style.contains("border: none")
    {
        return true;
    }
    let class = el.value().attr("class").unwrap_or("").to_ascii_lowercase();
    class.split_whitespace().any(|c| {
        matches!(
            c,
            "border"
                | "bordered"
                | "card"
                | "box"
                | "panel"
                | "well"
                | "callout"
                | "alert"
                | "message"
                | "modal"
                | "dialog"
        ) || c.contains("card")
            || c.contains("panel")
            || c.ends_with("-box")
    })
}

fn parse_table(el: &scraper::ElementRef<'_>) -> Option<Block> {
    let mut headers = Vec::new();
    let mut rows = Vec::new();

    for th in el.select(&Selector::parse("thead th, tr th").unwrap()) {
        let t = normalize_ws(&plain_text(&th));
        if !t.is_empty() {
            headers.push(t);
        }
    }
    // de-dupe if we double-counted
    if headers.len() > 12 {
        headers.truncate(12);
    }

    for tr in el.select(&Selector::parse("tr").unwrap()) {
        let cells: Vec<String> = tr
            .select(&Selector::parse("td").unwrap())
            .map(|td| normalize_ws(&plain_text(&td)))
            .filter(|s| !s.is_empty())
            .collect();
        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    if headers.is_empty() && rows.is_empty() {
        return None;
    }
    // If no headers, promote first row
    if headers.is_empty() && !rows.is_empty() {
        headers = rows.remove(0);
    }
    Some(Block::Table { headers, rows })
}

fn blocks_to_frame_lines(blocks: &[Block]) -> Vec<Vec<Span>> {
    let mut lines = Vec::new();
    for b in blocks {
        match b {
            Block::Heading { text, .. } => lines.push(vec![Span::Strong {
                text: text.clone(),
            }]),
            Block::Paragraph { spans } | Block::ListItem { spans, .. } | Block::Quote { spans } => {
                if !spans_empty(spans) {
                    lines.push(spans.clone());
                }
            }
            Block::Pre { text } => {
                for line in text.lines() {
                    lines.push(vec![Span::Code {
                        text: line.to_string(),
                    }]);
                }
            }
            Block::Image { alt } => lines.push(vec![Span::Em {
                text: if alt.is_empty() {
                    "[image]".into()
                } else {
                    format!("[img: {alt}]")
                },
            }]),
            Block::Caption { text } => lines.push(vec![Span::Em {
                text: text.clone(),
            }]),
            Block::Hr => lines.push(vec![Span::Text {
                text: "───".into(),
            }]),
            Block::Spacer => {}
            Block::Table { .. } | Block::Frame { .. } => {
                // nested tables/frames: flatten to summary
                lines.push(vec![Span::Em {
                    text: "[…]".into(),
                }]);
            }
        }
    }
    lines
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

    if matches!(name, "strong" | "b") {
        let t = normalize_ws(&plain_text(el));
        if !t.is_empty() {
            out.push(Span::Strong { text: t });
        }
        return;
    }
    if matches!(name, "em" | "i") {
        let t = normalize_ws(&plain_text(el));
        if !t.is_empty() {
            out.push(Span::Em { text: t });
        }
        return;
    }
    if name == "code" && el.parent().map(|p| {
        matches!(p.value(), Node::Element(e) if e.name() == "pre")
    }) != Some(true)
    {
        let t = el.text().collect::<String>();
        if !t.is_empty() {
            out.push(Span::Code {
                text: t.trim().to_string(),
            });
        }
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
        Span::Text { text }
        | Span::Strong { text }
        | Span::Em { text }
        | Span::Code { text }
        | Span::Link { text, .. } => text.split_whitespace().next().is_none(),
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
            Span::Strong { text } => {
                let t = normalize_ws(&text);
                if t.is_empty() {
                    None
                } else {
                    Some(Span::Strong { text: t })
                }
            }
            Span::Em { text } => {
                let t = normalize_ws(&text);
                if t.is_empty() {
                    None
                } else {
                    Some(Span::Em { text: t })
                }
            }
            Span::Code { text } => {
                let t = text.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(Span::Code { text: t })
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
    fn google_rewrites_to_ddg_search() {
        let mut doc = parse_html("https://www.google.com/", "<html><body></body></html>", 1);
        attach_known_forms(&mut doc);
        assert!(!doc.forms.is_empty());
        let url = doc.search_url("rust lang").unwrap();
        // Terminal clients use DDG HTML — Google CAPTCHAs bots.
        assert!(
            url.contains("duckduckgo.com"),
            "expected DDG rewrite, got {url}"
        );
        assert!(url.contains("q=rust"));
    }

    #[test]
    fn role_borders_table_pre() {
        let html = r#"
        <html><body><main>
          <h1>Doc</h1>
          <p>Hello <strong>world</strong> and <code>x</code>.</p>
          <pre>fn main() {}</pre>
          <table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>
          <fieldset><legend>Box</legend><p>Inside</p></fieldset>
          <img alt="logo" src="x.png">
        </main></body></html>
        "#;
        let doc = parse_html("https://example.com/", html, 1);
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Pre { .. })));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Table { .. })));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Frame { .. })));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Image { .. })));
        let has_strong = doc.blocks.iter().any(|b| match b {
            Block::Paragraph { spans } => spans.iter().any(|s| matches!(s, Span::Strong { .. })),
            _ => false,
        });
        assert!(has_strong);
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
