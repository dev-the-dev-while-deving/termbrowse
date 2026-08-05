//! Agent-facing JSON view of the document.

use crate::layout::Layout;
use crate::model::Document;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub url: String,
    pub title: String,
    pub timing_ms: crate::model::Timing,
    pub links: Vec<crate::model::Link>,
    pub blocks: Vec<crate::model::Block>,
    pub layout: Option<LayoutSummary>,
}

#[derive(Debug, Serialize)]
pub struct LayoutSummary {
    pub width: u16,
    pub line_count: usize,
    pub link_order: Vec<crate::model::Ref>,
    /// Plain text rendering for quick agent reads.
    pub text: String,
}

pub fn snapshot(doc: &Document, layout: Option<&Layout>) -> Snapshot {
    let layout = layout.map(|l| {
        let text = lines_to_text(l);
        LayoutSummary {
            width: l.width,
            line_count: l.lines.len(),
            link_order: l.link_order.clone(),
            text,
        }
    });

    Snapshot {
        url: doc.url.clone(),
        title: doc.title.clone(),
        timing_ms: doc.timing_ms.clone(),
        links: doc.links.clone(),
        blocks: doc.blocks.clone(),
        layout,
    }
}

pub fn to_json(snap: &Snapshot) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(snap)?)
}

fn lines_to_text(layout: &Layout) -> String {
    use crate::layout::Segment;
    let mut out = String::new();
    for line in &layout.lines {
        for seg in &line.segments {
            match seg {
                Segment::Text { text, .. } => out.push_str(text),
                Segment::Link { text, .. } => out.push_str(text),
            }
        }
        out.push('\n');
    }
    out
}
