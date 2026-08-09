//! PrivSearch — ad-free ranked search core (partner retrieval + our ranking).

mod ddg;
mod provider;
mod rank;
mod types;

pub use provider::{SearchProvider, from_env};
pub use types::{PrivacyMeta, Query, SearchResponse};

use anyhow::Result;
use std::time::Instant;

/// Run a full search: partner fetch → rank → response.
pub async fn run(query: Query) -> Result<SearchResponse> {
    let provider = from_env()?;
    run_with(&*provider, query).await
}

pub async fn run_with(provider: &dyn SearchProvider, query: Query) -> Result<SearchResponse> {
    let start = Instant::now();
    let raw = provider.fetch(&query).await?;
    let hits = rank::rank(&query, raw);
    Ok(SearchResponse {
        query: query.text,
        provider: provider.name().to_string(),
        hits,
        took_ms: start.elapsed().as_millis() as u64,
        privacy: PrivacyMeta::default(),
    })
}

/// Pretty-print results for the terminal CLI.
pub fn format_text(resp: &SearchResponse) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "PrivSearch  ·  {}  ·  {} ms  ·  ads={} profile={} logged={}\n",
        resp.provider,
        resp.took_ms,
        resp.privacy.ads,
        resp.privacy.profiling,
        resp.privacy.query_logged
    ));
    out.push_str(&format!("query: {}\n\n", resp.query));

    if resp.hits.is_empty() {
        out.push_str("No results.\n");
        return out;
    }

    for h in &resp.hits {
        out.push_str(&format!("{}. {}\n", h.rank, h.title));
        out.push_str(&format!("   {}\n", h.url));
        if !h.snippet.is_empty() {
            out.push_str(&format!("   {}\n", h.snippet));
        }
        out.push_str(&format!("   ({})  score={:.1}\n\n", h.domain, h.score));
    }
    out
}
