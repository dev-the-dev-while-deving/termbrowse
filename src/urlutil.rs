//! Small URL helpers (no browser engine).

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

/// Unwrap tracking/redirect wrappers so link clicks open the real page.
/// DuckDuckGo HTML results use `uddg=` redirect URLs.
pub fn unwrap_redirect(url: &str) -> String {
    let Ok(u) = Url::parse(url) else {
        return url.to_string();
    };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();

    // DDG: https://duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com
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

    // Generic ?url= / ?q=http redirects (rare)
    if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "url" || k == "target") {
        let target = v.to_string();
        if target.starts_with("http://") || target.starts_with("https://") {
            return target;
        }
    }

    url.to_string()
}

/// Resolve relative href against page base, then unwrap redirects.
pub fn resolve_and_unwrap(base_page: &str, href: &str) -> Result<String> {
    let abs = Url::parse(base_page)
        .ok()
        .and_then(|b| b.join(href).ok())
        .or_else(|| Url::parse(href).ok())
        .ok_or_else(|| anyhow::anyhow!("bad href: {href}"))?;
    Ok(unwrap_redirect(abs.as_str()))
}
