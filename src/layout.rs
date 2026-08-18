//! Role → terminal lines. Borders only for bordered roles. Reflows on width.

use crate::model::{Block, Document, Ref, Span};
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Serialize)]
pub struct Layout {
    pub width: u16,
    pub lines: Vec<LayoutLine>,
    pub link_order: Vec<Ref>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayoutLine {
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Segment {
    Text { text: String, style: Style },
    Link { r#ref: Ref, text: String },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Normal,
    Heading1,
    Heading2,
    Heading3,
    Math,
    Dim,
    Quote,
    Pre,
    Strong,
    Em,
    Code,
    Border,
    Image,
}

pub fn layout_document(doc: &Document, width: u16) -> Layout {
    let width = width.max(24) as usize;
    let mut lines = Vec::new();
    let mut link_order = Vec::new();

    for block in &doc.blocks {
        match block {
            Block::Heading { level, text, .. } => {
                let style = match level {
                    1 => Style::Heading1,
                    2 => Style::Heading2,
                    _ => Style::Heading3,
                };
                push_wrapped(&mut lines, text, style, width);
                if *level == 1 {
                    let rule = "─".repeat(text.chars().count().min(width).max(3));
                    lines.push(line_text(rule, Style::Border));
                }
            }
            Block::Paragraph { spans } => {
                layout_spans(&mut lines, spans, Style::Normal, width, &mut link_order, "");
            }
            Block::ListItem { spans, index } => {
                let prefix = if *index > 0 {
                    format!("{index}. ")
                } else {
                    "• ".into()
                };
                layout_spans(
                    &mut lines,
                    spans,
                    Style::Normal,
                    width,
                    &mut link_order,
                    &prefix,
                );
            }
            Block::Pre { text } => {
                let body: Vec<Vec<Segment>> = text
                    .lines()
                    .map(|l| {
                        vec![Segment::Text {
                            text: l.to_string(),
                            style: Style::Pre,
                        }]
                    })
                    .collect();
                push_box(&mut lines, None, &body, width, Style::Pre);
            }
            Block::Quote { spans } => {
                layout_spans(&mut lines, spans, Style::Quote, width, &mut link_order, "│ ");
            }
            Block::Hr => {
                lines.push(line_text("─".repeat(width.min(48)), Style::Border));
            }
            Block::Spacer => {
                lines.push(line_text(String::new(), Style::Normal));
            }
            Block::Image { alt } => {
                let label = if alt.is_empty() {
                    "[ image ]".into()
                } else {
                    format!("[ img: {alt} ]")
                };
                push_wrapped(&mut lines, &label, Style::Image, width);
            }
            Block::Caption { text } => {
                push_wrapped(&mut lines, &format!("  {text}"), Style::Dim, width);
            }
            Block::Math { display, preview } => {
                if *display {
                    push_box(
                        &mut lines,
                        Some("math"),
                        &[vec![Segment::Text {
                            text: preview.clone(),
                            style: Style::Math,
                        }]],
                        width,
                        Style::Math,
                    );
                } else {
                    push_wrapped(&mut lines, preview, Style::Math, width);
                }
            }
            Block::Table { headers, rows } => layout_table(&mut lines, headers, rows, width),
            Block::Frame { title, lines: inner } => {
                let mut body: Vec<Vec<Segment>> = Vec::new();
                for spans in inner {
                    let mut segs = Vec::new();
                    spans_to_segments(spans, &mut segs, &mut link_order, Style::Normal);
                    if segs.is_empty() {
                        segs.push(Segment::Text {
                            text: String::new(),
                            style: Style::Normal,
                        });
                    }
                    for wrapped in wrap_segments(&segs, width.saturating_sub(4).max(8)) {
                        body.push(wrapped);
                    }
                }
                push_box(&mut lines, title.as_deref(), &body, width, Style::Border);
            }
        }
    }

    Layout {
        width: width as u16,
        lines,
        link_order,
    }
}

fn layout_table(
    lines: &mut Vec<LayoutLine>,
    headers: &[String],
    rows: &[Vec<String>],
    width: usize,
) {
    let cols = headers
        .len()
        .max(rows.iter().map(|r| r.len()).max().unwrap_or(0))
        .max(1);
    let inner = width.saturating_sub(cols + 1).max(cols * 3);
    let col_w = (inner / cols).max(3);

    let fmt_row = |cells: &[String]| -> String {
        let mut parts = Vec::new();
        for i in 0..cols {
            let cell = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            parts.push(pad_width(cell, col_w));
        }
        format!("│{}│", parts.join("│"))
    };

    let rule = |left: char, mid: char, right: char| {
        let mut s = String::from(left);
        for i in 0..cols {
            s.push_str(&"─".repeat(col_w));
            if i + 1 < cols {
                s.push(mid);
            }
        }
        s.push(right);
        s
    };

    lines.push(line_text(rule('┌', '┬', '┐'), Style::Border));
    if !headers.is_empty() {
        lines.push(line_text(fmt_row(headers), Style::Strong));
        lines.push(line_text(rule('├', '┼', '┤'), Style::Border));
    }
    for row in rows {
        lines.push(line_text(fmt_row(row), Style::Normal));
    }
    lines.push(line_text(rule('└', '┴', '┘'), Style::Border));
}

fn pad_width(s: &str, width: usize) -> String {
    let mut t: String = s.chars().take(width).collect();
    while UnicodeWidthStr::width(t.as_str()) > width && !t.is_empty() {
        t.pop();
    }
    while UnicodeWidthStr::width(t.as_str()) < width {
        t.push(' ');
    }
    t
}

fn push_box(
    lines: &mut Vec<LayoutLine>,
    title: Option<&str>,
    body: &[Vec<Segment>],
    width: usize,
    content_style: Style,
) {
    let inner_w = width.saturating_sub(2).max(8);
    let title_bit = title
        .map(|t| {
            let t: String = t.chars().take(inner_w.saturating_sub(4)).collect();
            format!(" {t} ")
        })
        .unwrap_or_default();
    let title_w = UnicodeWidthStr::width(title_bit.as_str());
    let fill = inner_w.saturating_sub(title_w);
    lines.push(line_text(
        format!("┌{title_bit}{}┐", "─".repeat(fill)),
        Style::Border,
    ));
    if body.is_empty() {
        lines.push(LayoutLine {
            segments: vec![
                Segment::Text {
                    text: "│".into(),
                    style: Style::Border,
                },
                Segment::Text {
                    text: " ".repeat(inner_w),
                    style: content_style,
                },
                Segment::Text {
                    text: "│".into(),
                    style: Style::Border,
                },
            ],
        });
    } else {
        for row in body {
            let mut segs = vec![Segment::Text {
                text: "│".into(),
                style: Style::Border,
            }];
            let mut used = 0usize;
            for seg in row {
                match seg {
                    Segment::Text { text, style } => {
                        let t = truncate_width(text, inner_w.saturating_sub(used));
                        used += UnicodeWidthStr::width(t.as_str());
                        segs.push(Segment::Text {
                            text: t,
                            style: *style,
                        });
                    }
                    Segment::Link { r#ref, text } => {
                        let t = truncate_width(text, inner_w.saturating_sub(used));
                        used += UnicodeWidthStr::width(t.as_str());
                        segs.push(Segment::Link {
                            r#ref: *r#ref,
                            text: t,
                        });
                    }
                }
            }
            if used < inner_w {
                segs.push(Segment::Text {
                    text: " ".repeat(inner_w - used),
                    style: content_style,
                });
            }
            segs.push(Segment::Text {
                text: "│".into(),
                style: Style::Border,
            });
            lines.push(LayoutLine { segments: segs });
        }
    }
    lines.push(line_text(
        format!("└{}┘", "─".repeat(inner_w)),
        Style::Border,
    ));
}

fn layout_spans(
    lines: &mut Vec<LayoutLine>,
    spans: &[Span],
    default_style: Style,
    width: usize,
    link_order: &mut Vec<Ref>,
    prefix: &str,
) {
    let mut segs = Vec::new();
    if !prefix.is_empty() {
        segs.push(Segment::Text {
            text: prefix.to_string(),
            style: default_style,
        });
    }
    spans_to_segments(spans, &mut segs, link_order, default_style);
    for row in wrap_segments(&segs, width) {
        lines.push(LayoutLine { segments: row });
    }
}

fn spans_to_segments(
    spans: &[Span],
    out: &mut Vec<Segment>,
    link_order: &mut Vec<Ref>,
    default_style: Style,
) {
    for span in spans {
        match span {
            Span::Text { text } if !text.is_empty() => out.push(Segment::Text {
                text: text.clone(),
                style: default_style,
            }),
            Span::Text { .. } => {}
            Span::Strong { text } => out.push(Segment::Text {
                text: text.clone(),
                style: Style::Strong,
            }),
            Span::Em { text } => out.push(Segment::Text {
                text: text.clone(),
                style: Style::Em,
            }),
            Span::Code { text } => out.push(Segment::Text {
                text: text.clone(),
                style: Style::Code,
            }),
            Span::Link { r#ref, text } => {
                if !link_order.contains(r#ref) {
                    link_order.push(*r#ref);
                }
                out.push(Segment::Link {
                    r#ref: *r#ref,
                    text: format!("{text} [{r}]", r = r#ref),
                });
            }
        }
    }
}

fn push_words(
    tokens: &mut Vec<(String, Option<Ref>, Style)>,
    text: &str,
    link: Option<Ref>,
    style: Style,
) {
    if text.starts_with(char::is_whitespace) && !tokens.is_empty() {
        tokens.push((" ".into(), link, style));
    }
    for (i, word) in text.split_whitespace().enumerate() {
        if i > 0 {
            tokens.push((" ".into(), link, style));
        }
        tokens.push((word.to_string(), link, style));
    }
}

fn wrap_segments(segs: &[Segment], width: usize) -> Vec<Vec<Segment>> {
    if width == 0 {
        return vec![segs.to_vec()];
    }
    let mut tokens: Vec<(String, Option<Ref>, Style)> = Vec::new();
    for seg in segs {
        match seg {
            Segment::Text { text, style } => {
                push_words(&mut tokens, text, None, *style);
            }
            Segment::Link { r#ref, text } => {
                push_words(&mut tokens, text, Some(*r#ref), Style::Normal);
            }
        }
    }

    let mut rows: Vec<Vec<Segment>> = Vec::new();
    let mut cur: Vec<Segment> = Vec::new();
    let mut col = 0usize;

    for (text, link, style) in tokens {
        let w = UnicodeWidthStr::width(text.as_str());
        if col > 0 && col + w > width {
            if !cur.is_empty() {
                rows.push(std::mem::take(&mut cur));
            }
            col = 0;
            if text == " " {
                continue;
            }
        }
        if w > width && col == 0 {
            let mut rest = text.as_str();
            while !rest.is_empty() {
                let (chunk, next) = split_at_width(rest, width);
                rows.push(vec![seg(link, chunk.to_string(), style)]);
                rest = next;
            }
            continue;
        }
        if let (None, Some(Segment::Text { text: prev, style: ps })) = (link, cur.last_mut()) {
            if *ps == style {
                prev.push_str(&text);
                col += w;
                continue;
            }
        }
        cur.push(seg(link, text, style));
        col += w;
    }
    if !cur.is_empty() {
        rows.push(cur);
    }
    if rows.is_empty() {
        rows.push(vec![]);
    }
    rows
}

fn seg(link: Option<Ref>, text: String, style: Style) -> Segment {
    match link {
        Some(r) => Segment::Link { r#ref: r, text },
        None => Segment::Text { text, style },
    }
}

fn push_wrapped(lines: &mut Vec<LayoutLine>, text: &str, style: Style, width: usize) {
    let segs = vec![Segment::Text {
        text: text.to_string(),
        style,
    }];
    for row in wrap_segments(&segs, width) {
        lines.push(LayoutLine { segments: row });
    }
}

fn line_text(text: String, style: Style) -> LayoutLine {
    LayoutLine {
        segments: vec![Segment::Text { text, style }],
    }
}

fn truncate_width(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0;
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}

fn split_at_width(s: &str, width: usize) -> (&str, &str) {
    if UnicodeWidthStr::width(s) <= width {
        return (s, "");
    }
    let mut acc = 0usize;
    for (idx, ch) in s.char_indices() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc + cw > width {
            return (&s[..idx], &s[idx..]);
        }
        acc += cw;
    }
    (s, "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SearchForm, SiteIdentity, Timing};

    #[test]
    fn wraps_to_width() {
        let doc = Document {
            url: "https://example.com".into(),
            title: "t".into(),
            blocks: vec![Block::Paragraph {
                spans: vec![Span::Text {
                    text: "hello world this is a wrapping test".into(),
                }],
            }],
            links: vec![],
            nav: vec![],
            forms: vec![SearchForm {
                action: String::new(),
                method: "get".into(),
                query_param: "q".into(),
                placeholder: String::new(),
                hidden: vec![],
            }],
            site_search: None,
            timing_ms: Timing::default(),
            identity: SiteIdentity::default(),
            serp: Default::default(),
        };
        let lay = layout_document(&doc, 24);
        assert!(
            lay.lines.len() >= 2,
            "expected wrap, got {} lines",
            lay.lines.len()
        );
        assert!(lay.lines.iter().all(|l| {
            let w: usize = l
                .segments
                .iter()
                .map(|s| match s {
                    Segment::Text { text, .. } | Segment::Link { text, .. } => {
                        UnicodeWidthStr::width(text.as_str())
                    }
                })
                .sum();
            w <= 24
        }));
    }

    #[test]
    fn keeps_gap_between_styles() {
        let doc = Document {
            url: "https://example.com".into(),
            title: "t".into(),
            blocks: vec![Block::Paragraph {
                spans: vec![
                    Span::Text {
                        text: "prints the text".into(),
                    },
                    Span::Code {
                        text: " Hello, world!".into(),
                    },
                    Span::Text {
                        text: " to the screen".into(),
                    },
                ],
            }],
            links: vec![],
            nav: vec![],
            forms: vec![],
            site_search: None,
            timing_ms: Timing::default(),
            identity: SiteIdentity::default(),
            serp: Default::default(),
        };
        let lay = layout_document(&doc, 80);
        let mut s = String::new();
        for line in &lay.lines {
            for seg in &line.segments {
                match seg {
                    Segment::Text { text, .. } | Segment::Link { text, .. } => s.push_str(text),
                }
            }
        }
        assert!(
            s.contains("Hello, world! to the"),
            "lost inter-span space: {s:?}"
        );
    }

    #[test]
    fn headings_are_not_markdown_source() {
        let doc = Document {
            url: "https://example.com".into(),
            title: "t".into(),
            blocks: vec![Block::Heading {
                level: 1,
                text: "Hello".into(),
                id: None,
            }],
            links: vec![],
            nav: vec![],
            forms: vec![],
            site_search: None,
            timing_ms: Timing::default(),
            identity: SiteIdentity::default(),
            serp: Default::default(),
        };
        let lay = layout_document(&doc, 40);
        let mut s = String::new();
        for line in &lay.lines {
            for seg in &line.segments {
                match seg {
                    Segment::Text { text, .. } | Segment::Link { text, .. } => s.push_str(text),
                }
            }
        }
        assert!(!s.contains("# Hello"), "{s:?}");
        assert!(s.contains("Hello"));
    }
}
