//! Search provider trait and selection.

use super::ddg::DdgHtmlProvider;
use super::types::{Query, RawHit};
use anyhow::{Result, bail};
use std::env;
use std::future::Future;
use std::pin::Pin;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Something that can return raw web hits for a query.
pub trait SearchProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn fetch<'a>(&'a self, query: &'a Query) -> BoxFuture<'a, Result<Vec<RawHit>>>;
}

/// Build provider from `PRIVSEARCH_PROVIDER` (default: `ddg`).
pub fn from_env() -> Result<Box<dyn SearchProvider>> {
    let name = env::var("PRIVSEARCH_PROVIDER").unwrap_or_else(|_| "ddg".into());
    match name.to_ascii_lowercase().as_str() {
        "ddg" | "duckduckgo" => Ok(Box::new(DdgHtmlProvider::new())),
        "mock" => Ok(Box::new(MockProvider)),
        other => bail!("unknown PRIVSEARCH_PROVIDER={other} (try ddg|mock)"),
    }
}

/// Deterministic provider for tests and offline demos.
pub struct MockProvider;

impl SearchProvider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn fetch<'a>(&'a self, query: &'a Query) -> BoxFuture<'a, Result<Vec<RawHit>>> {
        Box::pin(async move {
            let q = &query.text;
            Ok(vec![
                RawHit {
                    title: format!("Official docs for {q}"),
                    url: format!("https://docs.example.com/{}", slug(q)),
                    snippet: format!("Authoritative reference covering {q} with examples."),
                    partner_rank: 0,
                },
                RawHit {
                    title: format!("{q} — Wikipedia"),
                    url: format!("https://en.wikipedia.org/wiki/{}", slug(q)),
                    snippet: format!("Encyclopedia overview of {q}."),
                    partner_rank: 1,
                },
                RawHit {
                    title: format!("Buy cheap {q}!!!! click here"),
                    url: format!(
                        "https://spam-content-farm.example/posts/{}/track?utm_source=x&utm_medium=y&utm_campaign=z&ref=123",
                        slug(q)
                    ),
                    snippet: "Amazing deals you won't believe. Click now for free stuff and more spam."
                        .into(),
                    partner_rank: 2,
                },
                RawHit {
                    title: format!("Deep dive: {q}"),
                    url: format!("https://blog.example.dev/{}", slug(q)),
                    snippet: format!("Technical walkthrough of {q} for practitioners."),
                    partner_rank: 3,
                },
            ])
        })
    }
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_ascii_lowercase()
}
