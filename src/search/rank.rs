//! Re-rank partner hits: quality first, spam down, mild diversity.

use super::types::{Hit, Query, RawHit};
use std::collections::HashMap;
use url::Url;

/// Domains that usually deserve a soft boost for technical queries.
const QUALITY_DOMAINS: &[&str] = &[
    "wikipedia.org",
    "github.com",
    "gitlab.com",
    "stackoverflow.com",
    "stackexchange.com",
    "developer.mozilla.org",
    "mdn.io",
    "docs.rs",
    "doc.rust-lang.org",
    "rust-lang.org",
    "python.org",
    "docs.python.org",
    "go.dev",
    "nodejs.org",
    "arxiv.org",
    "ietf.org",
    "w3.org",
    "nist.gov",
    "gov",
    "edu",
];

/// Hard / soft spam signals in host or path.
const SPAM_HOST_MARKERS: &[&str] = &[
    "content-farm",
    "clickbait",
    "seo-hub",
    "free-download-now",
    "cheap-viagra",
];

pub fn rank(query: &Query, raw: Vec<RawHit>) -> Vec<Hit> {
    let terms: Vec<String> = tokenize(&query.text);
    let mut scored: Vec<(f32, RawHit, String)> = raw
        .into_iter()
        .filter_map(|h| {
            let domain = domain_of(&h.url)?;
            let score = score_hit(&h, &domain, &terms);
            Some((score, h, domain))
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Drop extreme junk when at least one decent hit exists.
    let has_decent = scored.iter().any(|(s, _, _)| *s > 5.0);
    if has_decent {
        scored.retain(|(s, _, _)| *s > -10.0);
    }

    // Diversity: at most 2 hits from the same registrable host in the result set.
    let mut host_counts: HashMap<String, u32> = HashMap::new();
    let mut out = Vec::new();
    for (score, hit, domain) in scored {
        if out.len() >= query.limit {
            break;
        }
        let host_key = registrable_key(&domain);
        let n = host_counts.get(&host_key).copied().unwrap_or(0);
        if n >= 2 {
            continue;
        }
        host_counts.insert(host_key, n + 1);
        out.push(Hit {
            title: hit.title,
            url: hit.url,
            snippet: hit.snippet,
            domain,
            score,
            rank: out.len() as u32 + 1,
        });
    }
    out
}

fn score_hit(hit: &RawHit, domain: &str, terms: &[String]) -> f32 {
    let mut score = 1.0;

    // Partner order: earlier is slightly better.
    score += (20.0 - hit.partner_rank as f32).max(0.0) * 0.5;

    let title_l = hit.title.to_ascii_lowercase();
    let snip_l = hit.snippet.to_ascii_lowercase();
    let url_l = hit.url.to_ascii_lowercase();

    for t in terms {
        if title_l.contains(t) {
            score += 3.0;
        }
        if snip_l.contains(t) {
            score += 1.2;
        }
        if url_l.contains(t) {
            score += 0.8;
        }
    }

    // Quality domain boost.
    let dom_l = domain.to_ascii_lowercase();
    for q in QUALITY_DOMAINS {
        if dom_l == *q || dom_l.ends_with(&format!(".{q}")) || dom_l.ends_with(q) {
            score += 4.0;
            break;
        }
    }
    if dom_l.ends_with(".edu") || dom_l.ends_with(".gov") {
        score += 3.0;
    }

    // Spam / low-quality penalties (must outweigh partner-rank head start).
    let mut spammy = false;
    for m in SPAM_HOST_MARKERS {
        if dom_l.contains(m) || url_l.contains(m) {
            score -= 25.0;
            spammy = true;
        }
    }
    if hit.title.matches('!').count() >= 2 {
        score -= 6.0;
        spammy = true;
    }
    if title_l.contains("buy cheap") || title_l.contains("click here") {
        score -= 15.0;
        spammy = true;
    }
    if spammy {
        score -= 50.0; // floor: keep out of competitive top unless nothing else exists
    }

    // Tracking-param heavy URLs.
    let tracking = ["utm_source", "utm_medium", "utm_campaign", "fbclid", "gclid"];
    let track_n = tracking.iter().filter(|p| url_l.contains(*p)).count();
    score -= track_n as f32 * 1.5;

    // Very long SEO paths.
    if let Ok(u) = Url::parse(&hit.url) {
        let path = u.path();
        if path.matches('/').count() > 6 {
            score -= 2.0;
        }
        if path.len() > 80 {
            score -= 1.5;
        }
    }

    // Empty snippet is less useful.
    if hit.snippet.trim().is_empty() {
        score -= 2.0;
    }

    score
}

fn tokenize(q: &str) -> Vec<String> {
    q.split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_ascii_lowercase()
        })
        .filter(|t| t.len() > 1)
        .collect()
}

fn domain_of(url: &str) -> Option<String> {
    let u = Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    Some(host.trim_start_matches("www.").to_string())
}

fn registrable_key(domain: &str) -> String {
    // Cheap stand-in: last two labels (good enough for diversity v0).
    let parts: Vec<&str> = domain.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else {
        domain.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demotes_spam_and_boosts_docs() {
        let q = Query::new("rust async");
        let raw = vec![
            RawHit {
                title: "Buy cheap rust async!!!! click here".into(),
                url: "https://spam-content-farm.example/x?utm_source=a&utm_medium=b&utm_campaign=c"
                    .into(),
                snippet: "deals deals deals".into(),
                partner_rank: 0,
            },
            RawHit {
                title: "async/await - Rust".into(),
                url: "https://doc.rust-lang.org/std/keyword.async.html".into(),
                snippet: "Rust async keyword documentation".into(),
                partner_rank: 3,
            },
            RawHit {
                title: "Random blog".into(),
                url: "https://medium.com/some-post".into(),
                snippet: "thoughts on rust async".into(),
                partner_rank: 1,
            },
        ];
        let hits = rank(&q, raw);
        assert!(!hits.is_empty());
        assert!(
            hits[0].url.contains("doc.rust-lang.org"),
            "expected docs first, got {:?}",
            hits[0].url
        );
        assert!(
            hits.iter().all(|h| !h.url.contains("spam-content-farm")),
            "spam should be filtered when quality results exist, got {hits:?}"
        );
    }
}
