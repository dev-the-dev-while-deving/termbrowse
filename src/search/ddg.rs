//! DuckDuckGo HTML partner — privacy-aligned default for v0.

use super::provider::SearchProvider;
use super::types::{Query, RawHit};
use anyhow::{Context, Result, bail};
use scraper::{Html, Selector};
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;
use url::Url;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

const DDG_HTML: &str = "https://html.duckduckgo.com/html/";

pub struct DdgHtmlProvider {
    client: reqwest::Client,
}

impl DdgHtmlProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/122.0.0.0 Safari/537.36",
            )
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("reqwest client");
        Self { client }
    }
}

impl Default for DdgHtmlProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchProvider for DdgHtmlProvider {
    fn name(&self) -> &'static str {
        "ddg-html"
    }

    fn fetch<'a>(&'a self, query: &'a Query) -> BoxFuture<'a, Result<Vec<RawHit>>> {
        Box::pin(async move {
            if query.text.is_empty() {
                bail!("empty query");
            }

            let start = Instant::now();
            // POST matches DDG HTML form; GET often returns lite shell without results.
            let response = self
                .client
                .post(DDG_HTML)
                .header(
                    reqwest::header::ACCEPT,
                    "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8",
                )
                .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
                .form(&[("q", query.text.as_str())])
                .send()
                .await
                .context("ddg request failed")?;

            let status = response.status();
            if !status.is_success() {
                bail!("ddg HTTP {status}");
            }

            let body = response.text().await.context("ddg read body")?;
            if looks_like_ddg_block(&body) {
                bail!(
                    "DuckDuckGo blocked this request (bot/CAPTCHA challenge). \
                     Retry later, switch network, or set PRIVSEARCH_PROVIDER=mock for offline demo."
                );
            }
            let hits = parse_ddg_html(&body)?;
            tracing::debug!(
                count = hits.len(),
                ms = start.elapsed().as_millis() as u64,
                "ddg partner fetch"
            );
            if hits.is_empty() {
                bail!(
                    "DuckDuckGo returned zero parseable results. \
                     The HTML partner may have changed; try again or use PRIVSEARCH_PROVIDER=mock."
                );
            }
            Ok(hits)
        })
    }
}

fn looks_like_ddg_block(body: &str) -> bool {
    body.contains("anomaly-modal")
        || body.contains("challenge-form")
        || body.contains("Unfortunately, bots use DuckDuckGo too")
}

/// Parse DDG HTML SERP into raw hits.
pub fn parse_ddg_html(body: &str) -> Result<Vec<RawHit>> {
    let dom = Html::parse_document(body);
    let result_sel =
        Selector::parse("div.result.web-result").expect("selector");
    let title_sel = Selector::parse("a.result__a").expect("selector");
    let snippet_sel = Selector::parse("a.result__snippet").expect("selector");

    let mut hits = Vec::new();
    for (i, node) in dom.select(&result_sel).enumerate() {
        // Skip ads if DDG ever marks them in this template.
        let classes = node.value().attr("class").unwrap_or("");
        if classes.contains("result--ad") || classes.contains("is-ad") {
            continue;
        }

        let Some(a) = node.select(&title_sel).next() else {
            continue;
        };
        let title = a.text().collect::<String>().trim().to_string();
        if title.is_empty() {
            continue;
        }
        let href = a.value().attr("href").unwrap_or("").trim();
        let url = normalize_ddg_url(href);
        if url.is_empty() {
            continue;
        }

        let snippet = node
            .select(&snippet_sel)
            .next()
            .map(|n| n.text().collect::<String>())
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        hits.push(RawHit {
            title,
            url,
            snippet,
            partner_rank: i as u32,
        });
    }

    Ok(hits)
}

/// Unwrap DDG redirect links (`//duckduckgo.com/l/?uddg=...`) when present.
fn normalize_ddg_url(href: &str) -> String {
    if href.is_empty() {
        return String::new();
    }
    let absolute = if href.starts_with("//") {
        format!("https:{href}")
    } else if href.starts_with('/') {
        format!("https://duckduckgo.com{href}")
    } else {
        href.to_string()
    };

    let Ok(u) = Url::parse(&absolute) else {
        return absolute;
    };

    // Classic redirect: /l/?uddg=<encoded>
    if u.path().contains("/l/")
        && let Some((_, uddg)) = u.query_pairs().find(|(k, _)| k == "uddg")
    {
        return uddg.to_string();
    }

    absolute
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample_results() {
        let html = r#"
        <div id="links" class="results">
          <div class="result results_links results_links_deep web-result ">
            <h2 class="result__title">
              <a rel="nofollow" class="result__a" href="https://doc.rust-lang.org/book/">The Rust Book</a>
            </h2>
            <a class="result__snippet" href="https://doc.rust-lang.org/book/">Learn Rust with the official book.</a>
          </div>
          <div class="result results_links web-result result--ad">
            <h2 class="result__title">
              <a class="result__a" href="https://ads.example/x">Ad</a>
            </h2>
          </div>
        </div>
        "#;
        let hits = parse_ddg_html(html).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "The Rust Book");
        assert_eq!(hits[0].url, "https://doc.rust-lang.org/book/");
        assert!(hits[0].snippet.contains("official book"));
    }

    #[test]
    fn unwraps_uddg_redirect() {
        let enc = urlencoding_like("https://example.com/path");
        let href = format!("https://duckduckgo.com/l/?uddg={enc}&rut=abc");
        assert_eq!(normalize_ddg_url(&href), "https://example.com/path");
    }

    fn urlencoding_like(s: &str) -> String {
        // percent-encode minimal set for test
        s.replace(':', "%3A").replace('/', "%2F")
    }
}
