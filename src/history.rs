//! Local visit history. Same folder as favorites. No telemetry.

use crate::home::HomeData;
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Visit {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    #[serde(default)]
    pub visits: Vec<Visit>,
}

impl History {
    pub fn load() -> Self {
        let path = HomeData::config_dir().join("history.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let dir = HomeData::config_dir();
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("history.json");
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, s);
        }
    }

    pub fn push(&mut self, title: String, url: String) {
        if url.is_empty() {
            return;
        }
        self.visits.retain(|v| v.url != url);
        self.visits.insert(
            0,
            Visit {
                title,
                url,
                at: now_secs(),
            },
        );
        self.visits.truncate(MAX);
        self.save();
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
