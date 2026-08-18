//! Leftover Markdown in page text → preview spans (not source).

use crate::math::preview_inline_math;
use crate::model::{Block, Span};

pub fn clean_heading(text: &str) -> String {
    let t = text.trim();
    let t = t.trim_start_matches('#').trim();
    preview_inline_math(t)
}

/// Expand `**bold**`, `*em*`, `` `code` `` and inline math in a text run.
pub fn inline(text: &str) -> Vec<Span> {
    let text = preview_inline_math(text);
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut i = 0;
    let mut buf = String::new();
    let flush_text = |buf: &mut String, spans: &mut Vec<Span>| {
        if !buf.is_empty() {
            spans.push(Span::Text {
                text: std::mem::take(buf),
            });
        }
    };
    while i < chars.len() {
        if chars[i] == '`' {
            if let Some(end) = find_char(&chars, i + 1, '`') {
                flush_text(&mut buf, &mut spans);
                let code: String = chars[i + 1..end].iter().collect();
                if !code.is_empty() {
                    spans.push(Span::Code { text: code });
                }
                i = end + 1;
                continue;
            }
        }
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_seq(&chars, i + 2, &['*', '*']) {
                flush_text(&mut buf, &mut spans);
                let t: String = chars[i + 2..end].iter().collect();
                if !t.is_empty() {
                    spans.push(Span::Strong { text: t });
                }
                i = end + 2;
                continue;
            }
        }
        if i + 1 < chars.len() && chars[i] == '_' && chars[i + 1] == '_' {
            if let Some(end) = find_seq(&chars, i + 2, &['_', '_']) {
                flush_text(&mut buf, &mut spans);
                let t: String = chars[i + 2..end].iter().collect();
                if !t.is_empty() {
                    spans.push(Span::Strong { text: t });
                }
                i = end + 2;
                continue;
            }
        }
        if chars[i] == '*' {
            if let Some(end) = find_char(&chars, i + 1, '*') {
                flush_text(&mut buf, &mut spans);
                let t: String = chars[i + 1..end].iter().collect();
                if !t.is_empty() {
                    spans.push(Span::Em { text: t });
                }
                i = end + 1;
                continue;
            }
        }
        buf.push(chars[i]);
        i += 1;
    }
    flush_text(&mut buf, &mut spans);
    spans
}

pub fn rewrite_spans(spans: Vec<Span>) -> Vec<Span> {
    let mut out = Vec::new();
    for span in spans {
        match span {
            Span::Text { text } => out.extend(inline(&text)),
            other => out.push(other),
        }
    }
    out
}

/// Whole-file Markdown (README.md, raw .md URLs).
pub fn parse_document(md: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut in_fence = false;
    let mut fence = String::new();
    for raw in md.lines() {
        let line = raw.trim_end();
        if line.starts_with("```") {
            if in_fence {
                if !fence.trim().is_empty() {
                    blocks.push(Block::Pre {
                        text: fence.trim_end().to_string(),
                    });
                    blocks.push(Block::Spacer);
                }
                fence.clear();
                in_fence = false;
            } else {
                in_fence = true;
            }
            continue;
        }
        if in_fence {
            fence.push_str(line);
            fence.push('\n');
            continue;
        }
        let t = line.trim();
        if t.is_empty() {
            if !matches!(blocks.last(), Some(Block::Spacer) | None) {
                blocks.push(Block::Spacer);
            }
            continue;
        }
        if t == "---" || t == "***" || t == "___" {
            blocks.push(Block::Hr);
            blocks.push(Block::Spacer);
            continue;
        }
        if let Some(rest) = heading_line(t) {
            blocks.push(Block::Heading {
                level: rest.0,
                text: clean_heading(rest.1),
                id: None,
            });
            blocks.push(Block::Spacer);
            continue;
        }
        if let Some(rest) = t.strip_prefix("> ") {
            blocks.push(Block::Quote {
                spans: inline(rest),
            });
            continue;
        }
        if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            blocks.push(Block::ListItem {
                spans: inline(rest),
                index: 0,
            });
            continue;
        }
        if let Some((n, rest)) = ordered_item(t) {
            blocks.push(Block::ListItem {
                spans: inline(rest),
                index: n,
            });
            continue;
        }
        blocks.push(Block::Paragraph { spans: inline(t) });
        blocks.push(Block::Spacer);
    }
    if in_fence && !fence.trim().is_empty() {
        blocks.push(Block::Pre {
            text: fence.trim_end().to_string(),
        });
    }
    blocks
}

pub fn looks_like_markdown_file(url: &str, content_type: &str, body: &str) -> bool {
    let url_l = url.to_ascii_lowercase();
    if url_l.ends_with(".md") || url_l.ends_with(".markdown") || url_l.contains("/raw/") && url_l.ends_with(".md")
    {
        return true;
    }
    if content_type.contains("markdown")
        || (content_type.contains("text/plain")
            && body.lines().take(8).any(|l| l.starts_with("# ")))
    {
        return true;
    }
    let html_tags = body.matches('<').count();
    let md_marks = body.lines().filter(|l| l.starts_with("# ") || l.starts_with("```")).count();
    html_tags < 3 && md_marks >= 2 && !body.contains("<html")
}

fn heading_line(t: &str) -> Option<(u8, &str)> {
    let n = t.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&n) {
        let rest = t[n..].trim();
        if !rest.is_empty() && (t.as_bytes().get(n) == Some(&b' ') || t.as_bytes().get(n) == Some(&b'\t')) {
            return Some((n as u8, rest));
        }
    }
    None
}

fn ordered_item(t: &str) -> Option<(u32, &str)> {
    let (num, rest) = t.split_once(". ")?;
    let n: u32 = num.parse().ok()?;
    Some((n, rest))
}

fn find_char(chars: &[char], from: usize, c: char) -> Option<usize> {
    chars[from..].iter().position(|x| *x == c).map(|p| from + p)
}

fn find_seq(chars: &[char], from: usize, seq: &[char]) -> Option<usize> {
    let mut i = from;
    while i + seq.len() <= chars.len() {
        if &chars[i..i + seq.len()] == seq {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_heading_hashes() {
        assert_eq!(clean_heading("## Hello"), "Hello");
    }

    #[test]
    fn bold_and_code() {
        let s = inline("use **bold** and `x` here");
        assert!(s.iter().any(|x| matches!(x, Span::Strong { text } if text == "bold")));
        assert!(s.iter().any(|x| matches!(x, Span::Code { text } if text == "x")));
    }

    #[test]
    fn markdown_doc_headings() {
        let blocks = parse_document("# Title\n\nA **para**.\n");
        assert!(blocks.iter().any(|b| matches!(b, Block::Heading { text, .. } if text == "Title")));
    }
}
