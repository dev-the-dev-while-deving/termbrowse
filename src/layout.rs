//! Terminal cell layout — rows/cols, not CSS pixels.

use crate::model::{Block, Document, Ref, Span};
use serde::Serialize;
use std::time::Instant;
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
    Text {
        text: String,
        style: Style,
    },
    Link {
        r#ref: Ref,
        text: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Normal,
    Heading1,
    Heading2,
    Heading3,
    Dim,
    Quote,
    Pre,
}

pub fn layout_document(doc: &Document, width: u16) -> Layout {
    let start = Instant::now();
    let width = width.max(20) as usize;
    let mut lines = Vec::new();
    let mut link_order = Vec::new();

    for block in &doc.blocks {
        match block {
            Block::Heading { level, text } => {
                let style = match level {
                    1 => Style::Heading1,
                    2 => Style::Heading2,
                    _ => Style::Heading3,
                };
                let prefix = match level {
                    1 => "# ",
                    2 => "## ",
                    3 => "### ",
                    _ => "#### ",
                };
                push_wrapped(&mut lines, &format!("{prefix}{text}"), style, width, None);
            }
            Block::Paragraph { spans } => {
                layout_spans(&mut lines, spans, Style::Normal, width, &mut link_order, "");
            }
            Block::ListItem { spans } => {
                layout_spans(&mut lines, spans, Style::Normal, width, &mut link_order, "• ");
            }
            Block::Pre { text } => {
                for raw in text.lines() {
                    // Pre: hard-cut, no wrap soft logic beyond width.
                    let mut rest = raw;
                    if rest.is_empty() {
                        lines.push(LayoutLine {
                            segments: vec![Segment::Text {
                                text: String::new(),
                                style: Style::Pre,
                            }],
                        });
                        continue;
                    }
                    while !rest.is_empty() {
                        let (chunk, next) = split_at_width(rest, width);
                        lines.push(LayoutLine {
                            segments: vec![Segment::Text {
                                text: chunk.to_string(),
                                style: Style::Pre,
                            }],
                        });
                        rest = next;
                    }
                }
            }
            Block::Quote { spans } => {
                layout_spans(&mut lines, spans, Style::Quote, width, &mut link_order, "│ ");
            }
            Block::Hr => {
                let rule = "─".repeat(width.min(40));
                lines.push(LayoutLine {
                    segments: vec![Segment::Text {
                        text: rule,
                        style: Style::Dim,
                    }],
                });
            }
            Block::Spacer => {
                lines.push(LayoutLine {
                    segments: vec![Segment::Text {
                        text: String::new(),
                        style: Style::Normal,
                    }],
                });
            }
        }
    }

    let _ = start; // layout_ms filled by caller if needed
    Layout {
        width: width as u16,
        lines,
        link_order,
    }
}

fn layout_spans(
    lines: &mut Vec<LayoutLine>,
    spans: &[Span],
    style: Style,
    width: usize,
    link_order: &mut Vec<Ref>,
    prefix: &str,
) {
    // Build a sequence of (is_link, ref?, text) tokens, then greedy wrap.
    let mut tokens: Vec<Token> = Vec::new();
    if !prefix.is_empty() {
        tokens.push(Token::Text(prefix.to_string()));
    }
    for span in spans {
        match span {
            Span::Text { text } => {
                for (i, word) in text.split_whitespace().enumerate() {
                    if i > 0 {
                        tokens.push(Token::Space);
                    }
                    tokens.push(Token::Text(word.to_string()));
                }
            }
            Span::Link { r#ref, text } => {
                if !link_order.contains(r#ref) {
                    link_order.push(*r#ref);
                }
                // Link shown as: text [eN]
                let label = format!("{text} [{ref}]", ref = r#ref);
                for (i, word) in label.split_whitespace().enumerate() {
                    if i > 0 {
                        tokens.push(Token::Space);
                    }
                    tokens.push(Token::Link {
                        r#ref: *r#ref,
                        text: word.to_string(),
                    });
                }
            }
        }
    }

    if tokens.is_empty() {
        return;
    }

    let mut current: Vec<Segment> = Vec::new();
    let mut col = 0usize;

    for token in tokens {
        let (seg_text, link_ref) = match &token {
            Token::Space => (" ".to_string(), None),
            Token::Text(t) => (t.clone(), None),
            Token::Link { r#ref, text } => (text.clone(), Some(*r#ref)),
        };
        let w = UnicodeWidthStr::width(seg_text.as_str());

        if col > 0 && col + w > width {
            lines.push(LayoutLine {
                segments: std::mem::take(&mut current),
            });
            col = 0;
            if matches!(token, Token::Space) {
                continue;
            }
        }

        // Very long word: hard split.
        if w > width && col == 0 {
            let mut rest = seg_text.as_str();
            while !rest.is_empty() {
                let (chunk, next) = split_at_width(rest, width);
                let seg = match link_ref {
                    Some(r) => Segment::Link {
                        r#ref: r,
                        text: chunk.to_string(),
                    },
                    None => Segment::Text {
                        text: chunk.to_string(),
                        style,
                    },
                };
                lines.push(LayoutLine {
                    segments: vec![seg],
                });
                rest = next;
            }
            col = 0;
            current.clear();
            continue;
        }

        match link_ref {
            Some(r) => current.push(Segment::Link {
                r#ref: r,
                text: seg_text,
            }),
            None => {
                if let Some(Segment::Text {
                    text: prev,
                    style: prev_style,
                }) = current.last_mut()
                {
                    if *prev_style == style {
                        prev.push_str(&seg_text);
                    } else {
                        current.push(Segment::Text {
                            text: seg_text,
                            style,
                        });
                    }
                } else {
                    current.push(Segment::Text {
                        text: seg_text,
                        style,
                    });
                }
            }
        }
        col += w;
    }

    if !current.is_empty() {
        lines.push(LayoutLine { segments: current });
    }
}

enum Token {
    Space,
    Text(String),
    Link { r#ref: Ref, text: String },
}

fn push_wrapped(
    lines: &mut Vec<LayoutLine>,
    text: &str,
    style: Style,
    width: usize,
    _link: Option<Ref>,
) {
    let mut tokens = Vec::new();
    for (i, word) in text.split_whitespace().enumerate() {
        if i > 0 {
            tokens.push(Token::Space);
        }
        tokens.push(Token::Text(word.to_string()));
    }
    // Reuse span layout with a single text style by temporary spans path:
    let mut col = 0usize;
    let mut current = String::new();
    for token in tokens {
        let piece = match token {
            Token::Space => " ".to_string(),
            Token::Text(t) => t,
            Token::Link { text, .. } => text,
        };
        let w = UnicodeWidthStr::width(piece.as_str());
        if col > 0 && col + w > width {
            lines.push(LayoutLine {
                segments: vec![Segment::Text {
                    text: std::mem::take(&mut current),
                    style,
                }],
            });
            col = 0;
            if piece == " " {
                continue;
            }
        }
        current.push_str(&piece);
        col += w;
    }
    if !current.is_empty() {
        lines.push(LayoutLine {
            segments: vec![Segment::Text {
                text: current,
                style,
            }],
        });
    }
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
