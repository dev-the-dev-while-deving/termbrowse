//! Safari-style start page data: Favorites + Reading List.
//! Stored as JSON under the user config dir (editable from the TUI).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingItem {
    pub title: String,
    pub url: String,
    /// Unix seconds when saved.
    #[serde(default)]
    pub saved_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeData {
    #[serde(default)]
    pub favorites: Vec<Bookmark>,
    #[serde(default)]
    pub reading_list: Vec<ReadingItem>,
}

impl Default for HomeData {
    fn default() -> Self {
        Self {
            favorites: default_favorites(),
            reading_list: Vec::new(),
        }
    }
}

fn default_favorites() -> Vec<Bookmark> {
    // Prefer sites that work in a terminal (no CAPTCHA walls).
    vec![
        Bookmark {
            title: "Search".into(),
            url: "https://html.duckduckgo.com/html/".into(),
        },
        Bookmark {
            title: "Rust Book".into(),
            url: "https://doc.rust-lang.org/book/".into(),
        },
        Bookmark {
            title: "MDN".into(),
            url: "https://developer.mozilla.org/".into(),
        },
        Bookmark {
            title: "Hacker News".into(),
            url: "https://news.ycombinator.com/".into(),
        },
        Bookmark {
            title: "Wikipedia".into(),
            url: "https://en.wikipedia.org/".into(),
        },
        Bookmark {
            title: "Example".into(),
            url: "https://example.com".into(),
        },
    ]
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl HomeData {
    pub fn config_path() -> PathBuf {
        if let Some(dir) = dirs_config() {
            dir.join("termbrowse").join("home.json")
        } else {
            PathBuf::from("termbrowse-home.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => {
                let data = Self::default();
                let _ = data.save();
                data
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create config dir {}", parent.display()))?;
        }
        let s = serde_json::to_string_pretty(self)?;
        fs::write(&path, s).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn add_favorite(&mut self, title: String, url: String) {
        // Replace if same URL
        if let Some(i) = self.favorites.iter().position(|f| f.url == url) {
            self.favorites[i].title = title;
        } else {
            self.favorites.push(Bookmark { title, url });
        }
    }

    pub fn remove_favorite(&mut self, index: usize) {
        if index < self.favorites.len() {
            self.favorites.remove(index);
        }
    }

    pub fn update_favorite(&mut self, index: usize, title: String, url: String) {
        if let Some(f) = self.favorites.get_mut(index) {
            f.title = title;
            f.url = url;
        }
    }

    pub fn add_reading(&mut self, title: String, url: String) {
        if let Some(i) = self.reading_list.iter().position(|r| r.url == url) {
            self.reading_list[i].title = title;
            self.reading_list[i].saved_at = now_secs();
        } else {
            self.reading_list.insert(
                0,
                ReadingItem {
                    title,
                    url,
                    saved_at: now_secs(),
                },
            );
        }
    }

    pub fn remove_reading(&mut self, index: usize) {
        if index < self.reading_list.len() {
            self.reading_list.remove(index);
        }
    }
}

fn dirs_config() -> Option<PathBuf> {
    // Prefer XDG / macOS Application Support without extra deps.
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg));
    }
    if let Ok(home) = std::env::var("HOME") {
        #[cfg(target_os = "macos")]
        {
            return Some(PathBuf::from(home).join("Library/Application Support"));
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Some(PathBuf::from(home).join(".config"));
        }
    }
    None
}
