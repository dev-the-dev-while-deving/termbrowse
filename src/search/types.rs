//! Shared search types for PrivSearch.

use serde::{Deserialize, Serialize};

/// A user search request.
#[derive(Debug, Clone)]
pub struct Query {
    pub text: String,
    /// Max hits to return after ranking (default 10).
    pub limit: usize,
}

impl Query {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into().trim().to_string(),
            limit: 10,
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.clamp(1, 50);
        self
    }
}

/// Raw hit from a partner before our ranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
    /// Partner's original position (0-based), if known.
    pub partner_rank: u32,
}

/// Final ranked hit shown to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub domain: String,
    /// Higher is better.
    pub score: f32,
    pub rank: u32,
}

/// Full response for CLI / TUI / future API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub provider: String,
    pub hits: Vec<Hit>,
    /// Wall time for the full search path in milliseconds.
    pub took_ms: u64,
    pub privacy: PrivacyMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyMeta {
    /// No ad network, no profile graph.
    pub ads: bool,
    pub profiling: bool,
    /// Query text is not persisted by default.
    pub query_logged: bool,
}

impl Default for PrivacyMeta {
    fn default() -> Self {
        Self {
            ads: false,
            profiling: false,
            query_logged: false,
        }
    }
}
