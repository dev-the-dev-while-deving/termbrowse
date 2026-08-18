//! HTML → Document + SiteIdentity. No JS. Style tags scanned for color only.

use crate::color::steal;
use crate::math::{looks_like_tex, tex_to_unicode};
use crate::md;
use crate::model::{
    Block, Document, Link, NavItem, Ref, SearchForm, SiteIdentity, Span, Timing,
};
use crate::serp;
use crate::urlutil::unwrap_redirect;
use scraper::{Html, Node, Selector};
use url::Url;
use std::time::Instant;

const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "svg", "template", "iframe", "object", "embed", "canvas",
];
const MAX_BLOCKS: usize = 4000;
const MAX_LINKS: usize = 500;
const MAX_TABLE_ROWS: usize = 20;
const MAX_TABLE_COLS: usize = 8;
const MAX_STYLE_SCAN: usize = 80 * 1024;

pub fn parse_html(url: &str, html: &str, fetch_ms: u64) -> Document {
    let start = Instant::now();
    let dom = Html::parse_document(html);

    let title = select_one(&dom, "title")
        .map(|t| normalize_ws(&t.text().collect::<String>()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| url.to_string());

    let identity = extract_identity(&dom);

    let mut blocks = Vec::new();
    let mut links = Vec::new();
    let mut next_ref = 1u32;

    if let Some(scope) = pick_scope(&dom) {
        walk_element(&scope, &mut blocks, &mut links, &mut next_ref, 0);
    }

    let blocks = collapse_spacers(preview_blocks(blocks));
    for link in &mut links {
        link.href = unwrap_redirect(&link.href);
    }

    let serp = if serp::is_ddg_results_url(url) {
        serp::extract(&dom, url)
    } else {
        crate::serp::Serp::default()
    };
    let nav = extract_nav(&dom, url);
    let site_search = extract_site_search(&dom, url);

    Document {
        url: url.to_string(),
        title,
        blocks,
        links,
        nav,
        forms: vec![ddg_form()],
        site_search,
        timing_ms: Timing {
            fetch_ms,
            parse_ms: start.elapsed().as_millis() as u64,
            layout_ms: 0,
        },
        identity,
        serp,
    }
}

pub fn ddg_form() -> SearchForm {
    SearchForm {
        action: "https://html.duckduckgo.com/html/".into(),
        method: "get".into(),
        query_param: "q".into(),
        placeholder: "Search DuckDuckGo…".into(),
        hidden: vec![],
    }
}

fn extract_site_search(dom: &Html, page_url: &str) -> Option<SearchForm> {
    let form_sel = Selector::parse("form").ok()?;
    let mut best: Option<(i32, SearchForm)> = None;
    for form in dom.select(&form_sel) {
        let method = form
            .value()
            .attr("method")
            .unwrap_or("get")
            .to_ascii_lowercase();
        if method != "get" && !method.is_empty() {
            continue;
        }
        let action_raw = form.value().attr("action").unwrap_or("").trim();
        let action = if action_raw.is_empty() {
            page_url.to_string()
        } else {
            crate::urlutil::resolve_and_unwrap(page_url, action_raw).unwrap_or_else(|_| {
                if action_raw.starts_with('/') {
                    Url::parse(page_url)
                        .ok()
                        .and_then(|b| b.join(action_raw).ok())
                        .map(|u| u.to_string())
                        .unwrap_or_else(|| action_raw.to_string())
                } else {
                    action_raw.to_string()
                }
            })
        };
        let Ok(action_url) = Url::parse(&action) else {
            continue;
        };
        // Keep DDG as the product engine — skip DDG forms here.
        if action_url
            .host_str()
            .unwrap_or("")
            .contains("duckduckgo.")
        {
            continue;
        }

        let mut query_param: Option<String> = None;
        let mut placeholder = String::from("Search this site");
        let mut hidden = Vec::new();
        let mut score = 0;

        let id = form.value().attr("id").unwrap_or("").to_ascii_lowercase();
        let class = form.value().attr("class").unwrap_or("").to_ascii_lowercase();
        if id.contains("search") || class.contains("search") {
            score += 12;
        }

        if let Ok(input_sel) = Selector::parse("input, textarea") {
            for input in form.select(&input_sel) {
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
                    "search" | "text" | "textarea" => {
                        let is_q = matches!(
                            name.as_str(),
                            "search" | "q" | "query" | "search_query" | "searchfield" | "wd"
                        ) || itype == "search";
                        if is_q && query_param.is_none() {
                            query_param = Some(name.clone());
                            if !ph.is_empty() {
                                placeholder = ph.to_string();
                            }
                            if itype == "search" {
                                score += 10;
                            }
                            if name == "search" {
                                score += 8;
                            }
                            if ph.to_ascii_lowercase().contains("search") {
                                score += 4;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let Some(query_param) = query_param else {
            continue;
        };
        if action.to_ascii_lowercase().contains("search") {
            score += 4;
        }
        let cand = SearchForm {
            action: action_url.to_string(),
            method: "get".into(),
            query_param,
            placeholder,
            hidden,
        };
        if best.as_ref().map(|(s, _)| *s).unwrap_or(-1) < score {
            best = Some((score, cand));
        }
    }
    best.map(|(_, f)| f)
}

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
        heading(1, "Blocked by CAPTCHA"),
        Block::Spacer,
        para(
            "This is not a termbrowse bug. Search engines treat terminal clients as bots. \
We do not solve CAPTCHAs.",
        ),
        Block::Spacer,
        Block::ListItem {
            spans: vec![Span::Text {
                text: "Type a query below — searches go through DuckDuckGo HTML.".into(),
            }],
            index: 0,
        },
        Block::ListItem {
            spans: vec![Span::Text {
                text: "Or press H for Start Page and open docs / HN / Wikipedia.".into(),
            }],
            index: 0,
        },
        Block::Spacer,
        para(&format!("Blocked URL: {original}")),
    ];
    doc.forms = vec![ddg_form()];
    doc.links.clear();
}

pub fn annotate_if_sparse(doc: &mut Document) {
    if doc.looks_like_captcha() || doc.is_serp() {
        return;
    }
    if doc.is_search_home() {
        return;
    }
    if doc.text_len() >= 40 || doc.links.len() >= 5 {
        return;
    }
    let url = doc.url.clone();
    doc.blocks = vec![
        heading(1, "Sparse page"),
        Block::Spacer,
        para(
            "This site ships almost no content in static HTML (a JS app shell). \
termbrowse does not run a browser engine.",
        ),
        Block::Spacer,
        Block::ListItem {
            spans: vec![Span::Text {
                text: "Try DuckDuckGo HTML, the Rust book, MDN, Wikipedia.".into(),
            }],
            index: 0,
        },
        Block::Spacer,
        para(&format!("URL: {url}")),
    ];
}

fn heading(level: u8, text: &str) -> Block {
    Block::Heading {
        level,
        text: text.into(),
        id: None,
    }
}

fn para(text: &str) -> Block {
    Block::Paragraph {
        spans: vec![Span::Text {
            text: text.into(),
        }],
    }
}

fn select_one<'a>(dom: &'a Html, sel: &str) -> Option<scraper::ElementRef<'a>> {
    Selector::parse(sel).ok().and_then(|s| dom.select(&s).next())
}

fn extract_identity(dom: &Html) -> SiteIdentity {
    let mut id = SiteIdentity::default();

    if let Some(el) = select_one(dom, "meta[name='theme-color'], meta[name='msapplication-TileColor']")
    {
        if let Some(c) = el.value().attr("content").and_then(steal) {
            id.accent = Some(c);
        }
    }

    if let Ok(sel) = Selector::parse("style") {
        let mut css = String::new();
        for el in dom.select(&sel) {
            css.push_str(&el.text().collect::<String>());
            if css.len() > MAX_STYLE_SCAN {
                css.truncate(MAX_STYLE_SCAN);
                break;
            }
        }
        steal_from_css(&css, &mut id);
    }

    if id.link.is_none() {
        if let Some(a) = select_one(dom, "a[style]") {
            if let Some(c) = style_prop(a.value().attr("style").unwrap_or(""), "color").and_then(steal)
            {
                id.link = Some(c);
            }
        }
    }
    if id.heading.is_none() {
        if let Some(h) = select_one(dom, "h1[style], h2[style]") {
            if let Some(c) = style_prop(h.value().attr("style").unwrap_or(""), "color").and_then(steal)
            {
                id.heading = Some(c);
            }
        }
    }
    id
}

fn steal_from_css(css: &str, id: &mut SiteIdentity) {
    let css = strip_css_comments(css);
    for (key, slot) in [
        ("--accent", Slot::Accent),
        ("--primary", Slot::Accent),
        ("--brand", Slot::Accent),
        ("--theme-color", Slot::Accent),
        ("--link", Slot::Link),
        ("--link-color", Slot::Link),
        ("--heading", Slot::Heading),
        ("--title-color", Slot::Heading),
    ] {
        if slot.get(id).is_some() {
            continue;
        }
        if let Some(val) = css_decl_value(&css, key) {
            if let Some(c) = steal(&val) {
                slot.set(id, c);
            }
        }
    }
    if id.link.is_none() {
        if let Some(val) = first_rule_color(&css, &["a", "a:link"]) {
            id.link = steal(&val);
        }
    }
    if id.heading.is_none() {
        if let Some(val) = first_rule_color(&css, &["h1", "h2", ".title"]) {
            id.heading = steal(&val);
        }
    }
}

#[derive(Clone, Copy)]
enum Slot {
    Link,
    Heading,
    Accent,
}

impl Slot {
    fn get(self, id: &SiteIdentity) -> Option<ratatui::style::Color> {
        match self {
            Slot::Link => id.link,
            Slot::Heading => id.heading,
            Slot::Accent => id.accent,
        }
    }
    fn set(self, id: &mut SiteIdentity, c: ratatui::style::Color) {
        match self {
            Slot::Link => id.link = Some(c),
            Slot::Heading => id.heading = Some(c),
            Slot::Accent => id.accent = Some(c),
        }
    }
}

fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len().min(MAX_STYLE_SCAN));
    let bytes = css.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = i.saturating_add(2);
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn css_decl_value(css: &str, name: &str) -> Option<String> {
    let lower = css.to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(&name) {
        let abs = from + pos + name.len();
        let rest = css[abs..].trim_start();
        if let Some(rest) = rest.strip_prefix(':') {
            let val = rest.split([';', '}']).next()?.trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
        from = abs + 1;
        if from >= lower.len() {
            break;
        }
    }
    None
}

fn first_rule_color(css: &str, selectors: &[&str]) -> Option<String> {
    let lower = css.to_ascii_lowercase();
    for sel in selectors {
        let needle = format!("{sel} {{");
        let needle2 = format!("{sel}{{");
        let idx = lower.find(&needle).or_else(|| lower.find(&needle2));
        if let Some(idx) = idx {
            if let Some(body) = css[idx..].split('{').nth(1) {
                if let Some(val) = css_decl_value(body, "color") {
                    return Some(val);
                }
            }
        }
    }
    None
}

fn style_prop<'a>(style: &'a str, prop: &str) -> Option<&'a str> {
    let prop = prop.to_ascii_lowercase();
    for part in style.split(';') {
        let part = part.trim();
        if part.to_ascii_lowercase().starts_with(&prop) {
            if let Some((_, v)) = part.split_once(':') {
                return Some(v.trim());
            }
        }
    }
    None
}

fn extract_nav(dom: &Html, page_url: &str) -> Vec<NavItem> {
    let Ok(sel) = Selector::parse("nav a[href], header a[href], [role=navigation] a[href]") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for a in dom.select(&sel) {
        if out.len() >= 10 {
            break;
        }
        let href = a.value().attr("href").unwrap_or("").trim();
        if href.is_empty() || href.starts_with("javascript:") || href.starts_with("mailto:") {
            continue;
        }
        let title = normalize_ws(&plain_text(&a));
        if title.is_empty() || title.chars().count() > 24 {
            continue;
        }
        let url = crate::urlutil::resolve_and_unwrap(page_url, href).unwrap_or_default();
        if url.is_empty() || !seen.insert(url.clone()) {
            continue;
        }
        out.push(NavItem { title, url });
    }
    out
}

fn math_from_element(el: &scraper::ElementRef<'_>) -> Option<Block> {
    let tex = math_tex_from(el)?;
    Some(Block::Math {
        display: el.value().name() == "math"
            && el.value().attr("display").map(|d| d == "block").unwrap_or(false),
        preview: tex_to_unicode(&tex),
    })
}

fn math_tex_from(el: &scraper::ElementRef<'_>) -> Option<String> {
    if let Some(enc) = el.value().attr("encoding") {
        if enc.contains("tex") || enc.contains("TeX") {
            let t = normalize_ws(&plain_text(el));
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    if let Ok(sel) = Selector::parse("annotation[encoding], annotation") {
        for a in el.select(&sel) {
            let enc = a.value().attr("encoding").unwrap_or("");
            if enc.contains("tex") || enc.contains("TeX") || enc.is_empty() {
                let t = normalize_ws(&plain_text(&a));
                if looks_like_tex(&t) || !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }
    if let Some(alt) = el.value().attr("alt") {
        if looks_like_tex(alt) {
            return Some(alt.to_string());
        }
    }
    if let Ok(sel) = Selector::parse("img[alt]") {
        for img in el.select(&sel) {
            if let Some(alt) = img.value().attr("alt") {
                if looks_like_tex(alt) {
                    return Some(alt.to_string());
                }
            }
        }
    }
    let t = normalize_ws(&plain_text(el));
    if looks_like_tex(&t) {
        return Some(t);
    }
    None
}

fn preview_blocks(blocks: Vec<Block>) -> Vec<Block> {
    let mut out = Vec::new();
    for b in blocks {
        match b {
            Block::Heading { level, text, id } => out.push(Block::Heading {
                level,
                text: md::clean_heading(&text),
                id,
            }),
            Block::Paragraph { spans } => {
                let spans = md::rewrite_spans(spans);
                let plain = crate::model::spans_plain(&spans);
                if let Some((level, title)) = md_heading_plain(&plain) {
                    out.push(Block::Heading {
                        level,
                        text: title,
                        id: None,
                    });
                } else {
                    out.push(Block::Paragraph { spans });
                }
            }
            Block::ListItem { spans, index } => out.push(Block::ListItem {
                spans: md::rewrite_spans(spans),
                index,
            }),
            Block::Quote { spans } => out.push(Block::Quote {
                spans: md::rewrite_spans(spans),
            }),
            Block::Image { alt } if looks_like_tex(&alt) => out.push(Block::Math {
                display: false,
                preview: tex_to_unicode(&alt),
            }),
            other => out.push(other),
        }
    }
    out
}

fn md_heading_plain(plain: &str) -> Option<(u8, String)> {
    let t = plain.trim();
    let n = t.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&n) {
        let rest = t[n..].trim();
        if !rest.is_empty() {
            return Some((n as u8, md::clean_heading(rest)));
        }
    }
    None
}

pub fn parse_markdown(url: &str, body: &str, fetch_ms: u64) -> Document {
    let start = Instant::now();
    let title = body
        .lines()
        .find_map(|l| {
            let t = l.trim().trim_start_matches('#').trim();
            if l.trim_start().starts_with('#') && !t.is_empty() {
                Some(t.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| url.to_string());
    Document {
        url: url.to_string(),
        title,
        blocks: collapse_spacers(md::parse_document(body)),
        links: Vec::new(),
        nav: Vec::new(),
        forms: vec![ddg_form()],
        site_search: None,
        timing_ms: Timing {
            fetch_ms,
            parse_ms: start.elapsed().as_millis() as u64,
            layout_ms: 0,
        },
        identity: SiteIdentity::default(),
        serp: crate::serp::Serp::default(),
    }
}

/// Prefer a real article root. An empty `<main>` must not hide the rest of `<body>`.
fn pick_scope<'a>(dom: &'a Html) -> Option<scraper::ElementRef<'a>> {
    const PREFERRED: &[&str] = &[
        "#mw-content-text",
        ".markdown-body",
        "article",
        "main",
        "[role=main]",
        "#content",
        "body",
    ];
    let mut fallback = None;
    for sel in PREFERRED {
        if let Some(el) = select_one(dom, sel) {
            let n = el.text().collect::<String>().chars().count();
            if n >= 80 || *sel == "body" {
                return Some(el);
            }
            if fallback.is_none() {
                fallback = Some(el);
            }
        }
    }
    fallback
}

fn hidden(el: &scraper::ElementRef<'_>) -> bool {
    if el.value().attr("hidden").is_some() {
        return true;
    }
    let style = el.value().attr("style").unwrap_or("").to_ascii_lowercase();
    style.contains("display:none")
        || style.contains("display: none")
        || style.contains("visibility:hidden")
        || style.contains("visibility: hidden")
}

fn walk_element(
    el: &scraper::ElementRef<'_>,
    blocks: &mut Vec<Block>,
    links: &mut Vec<Link>,
    next_ref: &mut u32,
    list_depth: u8,
) {
    if blocks.len() >= MAX_BLOCKS {
        return;
    }
    let name = el.value().name();
    if name == "script" {
        let typ = el.value().attr("type").unwrap_or("").to_ascii_lowercase();
        if typ.contains("math") {
            let tex = el.text().collect::<String>();
            if !tex.trim().is_empty() {
                let display = typ.contains("display");
                blocks.push(Block::Math {
                    display,
                    preview: tex_to_unicode(&tex),
                });
                blocks.push(Block::Spacer);
            }
        }
        return;
    }
    if SKIP_TAGS.contains(&name) || hidden(el) {
        return;
    }
    if name == "math" || name == "annotation" {
        if let Some(b) = math_from_element(el) {
            blocks.push(b);
            blocks.push(Block::Spacer);
        }
        return;
    }
    if matches!(name, "nav" | "footer" | "aside") && list_depth == 0 {
        return;
    }

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
        let mut inner = Vec::new();
        for child in el.children() {
            if let Some(child_el) = scraper::ElementRef::wrap(child) {
                let cn = child_el.value().name();
                if cn == "legend" || cn == "figcaption" {
                    continue;
                }
                walk_element(&child_el, &mut inner, links, next_ref, list_depth);
            }
        }
        let lines = blocks_to_frame_lines(&inner);
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
                let id = el
                    .value()
                    .attr("id")
                    .or_else(|| el.value().attr("name"))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                blocks.push(Block::Heading {
                    level,
                    text: md::clean_heading(&text),
                    id,
                });
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
        "br" => blocks.push(Block::Spacer),
        "img" => {
            let alt = el
                .value()
                .attr("alt")
                .or_else(|| el.value().attr("title"))
                .unwrap_or("")
                .trim()
                .to_string();
            if looks_like_tex(&alt) {
                blocks.push(Block::Math {
                    display: el
                        .value()
                        .attr("class")
                        .unwrap_or("")
                        .contains("display"),
                    preview: tex_to_unicode(&alt),
                });
            } else {
                blocks.push(Block::Image { alt });
            }
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
                descend(el, blocks, links, next_ref, list_depth);
            }
        }
        "ul" => {
            descend(el, blocks, links, next_ref, list_depth.saturating_add(1));
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
        _ => {
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
    }
}

fn descend(
    el: &scraper::ElementRef<'_>,
    blocks: &mut Vec<Block>,
    links: &mut Vec<Link>,
    next_ref: &mut u32,
    list_depth: u8,
) {
    for child in el.children() {
        if let Some(child_el) = scraper::ElementRef::wrap(child) {
            walk_element(&child_el, blocks, links, next_ref, list_depth);
        }
    }
}

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
    if let Ok(sel) = Selector::parse("thead th, tr th") {
        for th in el.select(&sel) {
            let t = normalize_ws(&plain_text(&th));
            if !t.is_empty() {
                headers.push(t);
            }
            if headers.len() >= MAX_TABLE_COLS {
                break;
            }
        }
    }
    if let Ok(sel) = Selector::parse("tr") {
        for tr in el.select(&sel) {
            if rows.len() >= MAX_TABLE_ROWS {
                break;
            }
            let cells: Vec<String> = tr
                .select(&Selector::parse("td").unwrap())
                .map(|td| normalize_ws(&plain_text(&td)))
                .take(MAX_TABLE_COLS)
                .collect();
            if cells.iter().any(|s| !s.is_empty()) {
                rows.push(cells);
            }
        }
    }
    if headers.is_empty() && rows.is_empty() {
        return None;
    }
    if headers.is_empty() && !rows.is_empty() {
        headers = rows.remove(0);
    }
    headers.truncate(MAX_TABLE_COLS);
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
                lines.push(vec![Span::Em {
                    text: "[…]".into(),
                }]);
            }
            Block::Math { preview, .. } => lines.push(vec![Span::Em {
                text: preview.clone(),
            }]),
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
    if SKIP_TAGS.contains(&name) || hidden(el) {
        return;
    }
    if name == "math"
        || name == "annotation"
        || (name == "span"
            && el
                .value()
                .attr("class")
                .unwrap_or("")
                .contains("mwe-math"))
        || (name == "span" && el.value().attr("class").unwrap_or("").contains("katex"))
    {
        if let Some(tex) = math_tex_from(el) {
            let preview = tex_to_unicode(&tex);
            if !preview.is_empty() {
                out.push(Span::Em { text: preview });
            }
            return;
        }
    }
    if name == "img" {
        let alt = el.value().attr("alt").unwrap_or("").trim();
        if looks_like_tex(alt) {
            out.push(Span::Em {
                text: tex_to_unicode(alt),
            });
            return;
        }
    }
    if name == "a" {
        let href = el.value().attr("href").unwrap_or("").trim().to_string();
        let text = normalize_ws(&plain_text(el));
        if text.is_empty() && href.is_empty() {
            return;
        }
        let label = if text.is_empty() { href.clone() } else { text };
        if href.starts_with('#')
            || href.starts_with("javascript:")
            || href.starts_with("mailto:")
            || links.len() >= MAX_LINKS
        {
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
    if name == "code"
        && el.parent().map(|p| {
            matches!(p.value(), Node::Element(e) if e.name() == "pre")
        }) != Some(true)
    {
        let t = el.text().collect::<String>();
        if !t.trim().is_empty() {
            out.push(Span::Code {
                text: t.trim().to_string(),
            });
        }
        return;
    }
    if name == "br" {
        out.push(Span::Text { text: " ".into() });
        return;
    }
    for child in el.children() {
        match child.value() {
            Node::Text(t) => {
                if !t.is_empty() {
                    out.push(Span::Text { text: t.to_string() });
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
    let out: Vec<Span> = out
        .into_iter()
        .filter_map(|s| match s {
            Span::Text { text } => nonempty(text).map(|text| Span::Text { text }),
            Span::Strong { text } => nonempty(text).map(|text| Span::Strong { text }),
            Span::Em { text } => nonempty(text).map(|text| Span::Em { text }),
            Span::Code { text } => {
                let t = text.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(Span::Code { text: t })
                }
            }
            Span::Link { r#ref, text } => nonempty(text).map(|text| Span::Link { r#ref, text }),
        })
        .collect();
    ensure_span_gaps(out)
}

fn span_text(span: &Span) -> &str {
    match span {
        Span::Text { text }
        | Span::Strong { text }
        | Span::Em { text }
        | Span::Code { text }
        | Span::Link { text, .. } => text,
    }
}

fn prepend_space(span: &mut Span) {
    match span {
        Span::Text { text }
        | Span::Strong { text }
        | Span::Em { text }
        | Span::Code { text }
        | Span::Link { text, .. } => text.insert(0, ' '),
    }
}

fn needs_space(prev: &Span, next: &Span) -> bool {
    let a = span_text(prev).chars().last();
    let b = span_text(next).chars().next();
    match (a, b) {
        (Some(a), Some(b)) => {
            !a.is_whitespace()
                && !b.is_whitespace()
                && !matches!(a, '(' | '[' | '{' | '/' | '"' | '\'')
                && !matches!(b, '.' | ',' | ';' | ':' | ')' | ']' | '}' | '!' | '?' | '\'' | '"')
        }
        _ => false,
    }
}

fn ensure_span_gaps(mut spans: Vec<Span>) -> Vec<Span> {
    for i in (1..spans.len()).rev() {
        let need = needs_space(&spans[i - 1], &spans[i]);
        if need {
            prepend_space(&mut spans[i]);
        }
    }
    spans
}

fn nonempty(text: String) -> Option<String> {
    let t = normalize_ws(&text);
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

fn collapse_spacers(blocks: Vec<Block>) -> Vec<Block> {
    let mut out = Vec::new();
    let mut last_spacer = true;
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
        let html = r##"
        <html><head><title>Hello</title>
        <meta name="theme-color" content="#3366cc">
        <style>a { color: #ff8800; } h1 { color: #88ccff; }</style>
        </head>
        <body>
          <main>
            <h1>Title</h1>
            <p>Read <a href="https://example.com/more">more</a> here.</p>
            <ul><li>One</li><li><a href="/two">Two</a></li></ul>
          </main>
        </body></html>
        "##;
        let doc = parse_html("https://example.com/", html, 1);
        assert_eq!(doc.title, "Hello");
        assert!(doc.links.len() >= 2);
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Heading { level: 1, .. })));
        assert!(doc.identity.accent.is_some());
        assert!(doc.identity.link.is_some());
        assert!(doc.identity.heading.is_some());
    }

    #[test]
    fn only_ddg_search() {
        let doc = parse_html("https://example.com/", "<html><body></body></html>", 1);
        assert_eq!(doc.forms.len(), 1);
        assert!(doc.forms[0].action.contains("duckduckgo.com"));
        let url = doc.search_url("rust lang").unwrap();
        assert!(url.contains("duckduckgo.com"));
        assert!(url.contains("q=rust"));
    }

    #[test]
    fn unwraps_ddg_links() {
        let html = r#"<html><body><main>
          <a href="https://duckduckgo.com/l/?uddg=https%3A%2F%2Fdoc.rust-lang.org%2Fbook%2F">Rust Book</a>
        </main></body></html>"#;
        let doc = parse_html("https://html.duckduckgo.com/html/?q=rust", html, 1);
        assert!(doc.links.iter().any(|l| l.href.contains("doc.rust-lang.org")));
    }

    #[test]
    fn roles_and_skips_hidden() {
        let html = r#"<html><body><main>
          <h1>Doc</h1>
          <p>Hello <strong>world</strong> and <code>x</code>.</p>
          <pre>fn main() {}</pre>
          <table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>
          <fieldset><legend>Box</legend><p>Inside</p></fieldset>
          <p style="display:none">secret</p>
          <img alt="logo" src="x.png">
        </main></body></html>"#;
        let doc = parse_html("https://example.com/", html, 1);
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Pre { .. })));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Table { .. })));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Frame { .. })));
        let secret = doc.blocks.iter().any(|b| match b {
            Block::Paragraph { spans } => crate::model::spans_plain(spans).contains("secret"),
            _ => false,
        });
        assert!(!secret);
    }

    #[test]
    fn keeps_article_after_many_links() {
        let mut html = String::from("<html><body><main><ul>");
        for i in 0..600 {
            html.push_str(&format!("<li><a href=\"/p{i}\">link {i}</a></li>"));
        }
        html.push_str("</ul><h2>Unique later heading</h2><p>Body after the link flood.</p></main></body></html>");
        let doc = parse_html("https://example.com/long", &html, 1);
        assert!(
            doc.blocks.iter().any(|b| matches!(b, Block::Heading { text, .. } if text.contains("Unique later heading"))),
            "link cap must not drop the rest of the page"
        );
        assert!(
            doc.blocks.iter().any(|b| match b {
                Block::Paragraph { spans } => crate::model::spans_plain(spans).contains("Body after"),
                _ => false,
            })
        );
    }

    #[test]
    fn empty_main_falls_back_to_body() {
        let html = r#"<html><body>
          <main></main>
          <div><h1>Visible body title</h1><p>The article lives outside main.</p></div>
        </body></html>"#;
        let doc = parse_html("https://example.com/shell", html, 1);
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Heading { text, .. } if text.contains("Visible body title"))));
    }

    #[test]
    fn extracts_nav_and_heading_ids() {
        let html = r##"<html><body>
          <nav><a href="/docs">Docs</a><a href="/blog">Blog</a></nav>
          <main>
            <h1 id="top">Hello</h1>
            <p>See <a href="#top">top</a>.</p>
          </main>
        </body></html>"##;
        let doc = parse_html("https://example.com/", html, 1);
        assert!(doc.nav.iter().any(|n| n.title == "Docs"));
        assert!(doc.blocks.iter().any(|b| matches!(
            b,
            Block::Heading { id: Some(id), .. } if id == "top"
        )));
    }

    #[test]
    fn previews_tex_image_alt() {
        let html = r##"<html><body><main>
          <p>Energy <img alt="E=mc^{2}"></p>
          <math>\\frac{a}{b}</math>
        </main></body></html>"##;
        let doc = parse_html("https://example.com/math", html, 1);
        let blob: String = doc.blocks.iter().map(crate::model::block_plain).collect();
        assert!(blob.contains('²') || blob.contains('⁄'), "got {blob:?}");
        assert!(!blob.contains("^{2}"));
    }

    #[test]
    fn extracts_wikipedia_search_form() {
        let html = r#"<html><body>
          <form action="/w/index.php" id="searchform">
            <input type="search" name="search" placeholder="Search Wikipedia">
            <input type="hidden" name="title" value="Special:Search">
          </form>
          <main><h1>Rust</h1><p>A language.</p></main>
        </body></html>"#;
        let doc = parse_html("https://en.wikipedia.org/wiki/Rust", html, 1);
        let form = doc.site_search.clone().expect("site search");
        assert_eq!(form.query_param, "search");
        assert!(form.action.contains("wikipedia.org"));
        assert!(form.hidden.iter().any(|(k, v)| k == "title" && v == "Special:Search"));
        let url = doc.site_search_url("borrow checker").unwrap();
        assert!(url.contains("search=borrow"));
        assert!(url.contains("Special%3ASearch") || url.contains("Special:Search"));
    }

    #[test]
    fn extracts_ddg_cards() {
        let html = r##"<html><body>
          <div class="result web-result">
            <h2 class="result__title">
              <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2F">Example</a>
            </h2>
            <a class="result__url">example.com</a>
            <a class="result__snippet">A clean result card.</a>
          </div>
        </body></html>"##;
        let doc = parse_html("https://html.duckduckgo.com/html/?q=example", html, 1);
        assert_eq!(doc.serp.hits.len(), 1);
        assert!(doc.serp.hits[0].url.contains("example.com"));
        assert!(doc.is_serp());
    }

    #[test]
    fn keeps_space_around_inline_code() {
        let html = r#"<html><body><main>
          <p>prints <code>Hello, world!</code> to the screen.</p>
        </main></body></html>"#;
        let doc = parse_html("https://example.com/", html, 1);
        let text = doc
            .blocks
            .iter()
            .find_map(|b| match b {
                Block::Paragraph { spans } => Some(crate::model::spans_plain(spans)),
                _ => None,
            })
            .unwrap();
        assert!(
            text.contains("Hello, world! to"),
            "expected space after code, got {text:?}"
        );
    }
}
