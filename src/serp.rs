//! DuckDuckGo HTML → result cards. Dedicated SERP, not a flat link soup.

use crate::urlutil::unwrap_redirect;
use scraper::{Html, Selector};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct InstantAnswer {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub display_url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Serp {
    pub query: String,
    pub instant: Option<InstantAnswer>,
    pub hits: Vec<SearchHit>,
}

pub fn is_ddg_results_url(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else {
        return false;
    };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    if !host.contains("duckduckgo.") {
        return false;
    }
    u.query_pairs().any(|(k, v)| k == "q" && !v.trim().is_empty())
}

pub fn query_from_url(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "q")
                .map(|(_, v)| v.to_string())
        })
        .unwrap_or_default()
}

pub fn extract(dom: &Html, page_url: &str) -> Serp {
    let mut serp = Serp {
        query: query_from_url(page_url),
        instant: extract_instant(dom),
        hits: Vec::new(),
    };

    let Ok(card_sel) = Selector::parse(".result.web-result, .result.results_links") else {
        return serp;
    };
    let title_sel = Selector::parse("a.result__a").ok();
    let url_sel = Selector::parse("a.result__url").ok();
    let snip_sel = Selector::parse("a.result__snippet, .result__snippet").ok();

    for card in dom.select(&card_sel) {
        if serp.hits.len() >= 20 {
            break;
        }
        let title_el = title_sel.as_ref().and_then(|s| card.select(s).next());
        let Some(title_el) = title_el else {
            continue;
        };
        let title = normalize(&title_el.text().collect::<String>());
        if title.is_empty() {
            continue;
        }
        let href = title_el.value().attr("href").unwrap_or("");
        let abs = absolutize(href);
        let url = unwrap_redirect(&abs);
        if url.is_empty() {
            continue;
        }
        let display_url = url_sel
            .as_ref()
            .and_then(|s| card.select(s).next())
            .map(|e| normalize(&e.text().collect::<String>()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| display_host(&url));
        let snippet = snip_sel
            .as_ref()
            .and_then(|s| card.select(s).next())
            .map(|e| normalize(&e.text().collect::<String>()))
            .unwrap_or_default();
        serp.hits.push(SearchHit {
            title,
            url,
            display_url,
            snippet,
        });
    }
    serp
}

fn extract_instant(dom: &Html) -> Option<InstantAnswer> {
    let heading = Selector::parse(".zci__heading a, h1.zci__heading a").ok()?;
    let a = dom.select(&heading).next()?;
    let title = normalize(&a.text().collect::<String>());
    if title.is_empty() {
        return None;
    }
    let href = a.value().attr("href").unwrap_or("");
    let url = unwrap_redirect(&absolutize(href));
    let snippet = Selector::parse(".zci__result, #zero_click_abstract")
        .ok()
        .and_then(|s| dom.select(&s).next())
        .map(|e| normalize(&e.text().collect::<String>()))
        .unwrap_or_default();
    Some(InstantAnswer {
        title,
        url,
        snippet,
    })
}

fn absolutize(href: &str) -> String {
    let href = href.trim();
    if href.starts_with("//") {
        format!("https:{href}")
    } else if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if href.starts_with('/') {
        format!("https://duckduckgo.com{href}")
    } else {
        href.to_string()
    }
}

fn display_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
        .unwrap_or_else(|| url.to_string())
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_cards_and_instant() {
        let html = r##"
        <html><body>
          <div class="zci"><h1 class="zci__heading">
            <a href="https://en.wikipedia.org/wiki/Rust_(programming_language)">Rust (programming language)</a>
          </h1>
          <div class="zci__result">Rust is a general-purpose language.</div></div>
          <div class="result results_links results_links_deep web-result">
            <h2 class="result__title">
              <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org%2F">Rust Programming Language</a>
            </h2>
            <a class="result__url">rust-lang.org</a>
            <a class="result__snippet">A language empowering everyone to build reliable software.</a>
          </div>
        </body></html>
        "##;
        let dom = Html::parse_document(html);
        let serp = extract(&dom, "https://html.duckduckgo.com/html/?q=rust");
        assert_eq!(serp.query, "rust");
        assert!(serp.instant.is_some());
        assert_eq!(serp.hits.len(), 1);
        assert!(serp.hits[0].url.contains("rust-lang.org"));
        assert!(serp.hits[0].snippet.contains("empowering"));
    }
}
