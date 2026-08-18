//! URL normalize / unwrap. No engine.

use anyhow::{Result, bail};
use url::Url;

pub fn ensure_http_url(input: &str) -> Result<String> {
    let t = input.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        return Ok(t.to_string());
    }
    if t.contains('.') && !t.contains(' ') {
        return Ok(format!("https://{t}"));
    }
    bail!("not a URL: {input}");
}

pub fn unwrap_redirect(url: &str) -> String {
    let Ok(u) = Url::parse(url) else {
        return url.to_string();
    };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();

    if host.contains("duckduckgo.") {
        for (k, v) in u.query_pairs() {
            if k == "uddg" || k == "u" {
                let target = v.to_string();
                if target.starts_with("http://") || target.starts_with("https://") {
                    return target;
                }
            }
        }
    }

    if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "url" || k == "target") {
        let target = v.to_string();
        if target.starts_with("http://") || target.starts_with("https://") {
            return target;
        }
    }

    url.to_string()
}

/// If `href` is an in-page `#anchor` (same document), return the fragment.
pub fn same_page_fragment(base_page: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if let Some(rest) = href.strip_prefix('#') {
        return if rest.is_empty() {
            None
        } else {
            Some(rest.to_string())
        };
    }
    let abs = Url::parse(base_page)
        .ok()
        .and_then(|b| b.join(href).ok())
        .or_else(|| Url::parse(href).ok())?;
    let base = Url::parse(base_page).ok()?;
    if abs.host_str() == base.host_str()
        && abs.path() == base.path()
        && abs.query() == base.query()
    {
        abs.fragment().filter(|s| !s.is_empty()).map(|s| s.to_string())
    } else {
        None
    }
}

pub fn resolve_and_unwrap(base_page: &str, href: &str) -> Result<String> {
    let abs = Url::parse(base_page)
        .ok()
        .and_then(|b| b.join(href).ok())
        .or_else(|| Url::parse(href).ok())
        .ok_or_else(|| anyhow::anyhow!("bad href: {href}"))?;
    Ok(unwrap_redirect(abs.as_str()))
}

/// DuckDuckGo is the only search engine. Google/Bing product paths rewrite.
pub fn normalize_search_url(url: &str) -> String {
    let Ok(u) = Url::parse(url) else {
        return url.to_string();
    };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    if host.contains("duckduckgo.") {
        return url.to_string();
    }
    if host.contains("google.") || host.contains("bing.") {
        let q = u
            .query_pairs()
            .find(|(k, _)| k == "q")
            .map(|(_, v)| v.to_string());
        if let Some(q) = q {
            return crate::model::Document::ddg_search_url(&q);
        }
        return "https://html.duckduckgo.com/html/".into();
    }
    unwrap_redirect(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_ddg() {
        let u = "https://duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2F";
        assert_eq!(unwrap_redirect(u), "https://example.com/");
    }

    #[test]
    fn same_page_hash() {
        assert_eq!(
            same_page_fragment("https://ex.com/a", "#sec"),
            Some("sec".into())
        );
        assert_eq!(
            same_page_fragment("https://ex.com/a", "https://ex.com/a#sec"),
            Some("sec".into())
        );
        assert_eq!(same_page_fragment("https://ex.com/a", "https://ex.com/b#sec"), None);
    }

    #[test]
    fn rewrites_google() {
        let u = normalize_search_url("https://www.google.com/search?q=rust");
        assert!(u.contains("duckduckgo.com"));
        assert!(u.contains("q=rust"));
    }
}
