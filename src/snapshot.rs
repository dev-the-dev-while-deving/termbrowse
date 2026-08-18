//! Agent JSON of the same Document the TUI paints.

use crate::layout::{Layout, Segment};
use crate::model::Document;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub url: String,
    pub title: String,
    pub timing_ms: crate::model::Timing,
    pub links: Vec<crate::model::Link>,
    pub blocks: Vec<crate::model::Block>,
    pub forms: Vec<crate::model::SearchForm>,
    pub site_search: Option<crate::model::SearchForm>,
    pub results: Vec<crate::serp::SearchHit>,
    pub layout: Option<LayoutSummary>,
}

#[derive(Debug, Serialize)]
pub struct LayoutSummary {
    pub width: u16,
    pub line_count: usize,
    pub link_order: Vec<crate::model::Ref>,
    pub text: String,
}

pub fn snapshot(doc: &Document, layout: Option<&Layout>) -> Snapshot {
    let layout = layout.map(|l| LayoutSummary {
        width: l.width,
        line_count: l.lines.len(),
        link_order: l.link_order.clone(),
        text: lines_to_text(l),
    });
    Snapshot {
        url: doc.url.clone(),
        title: doc.title.clone(),
        timing_ms: doc.timing_ms.clone(),
        links: doc.links.clone(),
        blocks: doc.blocks.clone(),
        forms: doc.forms.clone(),
        site_search: doc.site_search.clone(),
        results: doc.serp.hits.clone(),
        layout,
    }
}

pub fn to_json(snap: &Snapshot) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(snap)?)
}

fn lines_to_text(layout: &Layout) -> String {
    let mut out = String::new();
    for line in &layout.lines {
        for seg in &line.segments {
            match seg {
                Segment::Text { text, .. } | Segment::Link { text, .. } => out.push_str(text),
            }
        }
        out.push('\n');
    }
    out
}
