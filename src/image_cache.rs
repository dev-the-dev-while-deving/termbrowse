//! Three-Tier Image Caching System:
//! - Tier 1: Disk Cache (Raw bytes stored in ~/.cache/termbrowse/images using SHA-256 URL keys)
//! - Tier 2: In-Memory Full-Res Cache (Unscaled DynamicImage objects for instant re-scaling)
//! - Tier 3: Render Cache (Per-terminal-column cell buffers, invalidated on window resize)

use image::DynamicImage;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub struct ImageCacheManager {
    // Tier 2: Memory cache holding full-resolution DynamicImages
    mem_cache: Mutex<HashMap<String, DynamicImage>>,
    // Tier 3: Per-terminal-width render cache holding pre-rendered ColoredSpan cell lines
    render_cache: Mutex<HashMap<(String, u16), Vec<Vec<crate::layout::ColoredSpan>>>>,
    // Disk cache path
    cache_dir: Option<PathBuf>,
}

static CACHE_INSTANCE: OnceLock<ImageCacheManager> = OnceLock::new();

pub fn get_image_cache() -> &'static ImageCacheManager {
    CACHE_INSTANCE.get_or_init(|| {
        let dir = dirs_next_cache_dir().map(|d| d.join("termbrowse").join("images"));
        if let Some(ref d) = dir {
            let _ = fs::create_dir_all(d);
        }
        ImageCacheManager {
            mem_cache: Mutex::new(HashMap::new()),
            render_cache: Mutex::new(HashMap::new()),
            cache_dir: dir,
        }
    })
}

fn dirs_next_cache_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".cache"))
        })
}

impl ImageCacheManager {
    /// SHA-256 hash helper for disk filenames.
    fn hash_url(url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Tier 1: Get raw bytes from disk cache if present.
    pub fn get_disk_bytes(&self, url: &str) -> Option<Vec<u8>> {
        let dir = self.cache_dir.as_ref()?;
        let path = dir.join(Self::hash_url(url));
        fs::read(path).ok()
    }

    /// Tier 1: Save raw bytes to disk cache.
    pub fn put_disk_bytes(&self, url: &str, bytes: &[u8]) {
        if let Some(ref dir) = self.cache_dir {
            let path = dir.join(Self::hash_url(url));
            let _ = fs::write(path, bytes);
        }
    }

    /// Tier 2: Get unscaled DynamicImage from memory cache.
    pub fn get_mem_image(&self, url: &str) -> Option<DynamicImage> {
        let guard = self.mem_cache.lock().ok()?;
        guard.get(url).cloned()
    }

    /// Tier 2: Store unscaled DynamicImage in memory cache (capacity capped at 50 images).
    pub fn put_mem_image(&self, url: &str, img: DynamicImage) {
        if let Ok(mut guard) = self.mem_cache.lock() {
            if guard.len() >= 50 {
                guard.clear(); // Evict all when memory ceiling reached
            }
            guard.insert(url.to_string(), img);
        }
    }

    /// Tier 3: Get pre-rendered cell lines for a given image and terminal column width.
    pub fn get_rendered_spans(&self, url: &str, cols: u16) -> Option<Vec<Vec<crate::layout::ColoredSpan>>> {
        let guard = self.render_cache.lock().ok()?;
        guard.get(&(url.to_string(), cols)).cloned()
    }

    /// Tier 3: Store pre-rendered cell lines.
    pub fn put_rendered_spans(&self, url: &str, cols: u16, lines: Vec<Vec<crate::layout::ColoredSpan>>) {
        if let Ok(mut guard) = self.render_cache.lock() {
            if guard.len() >= 100 {
                guard.clear();
            }
            guard.insert((url.to_string(), cols), lines);
        }
    }

    /// Invalidate Tier 3 Render Cache (e.g. on terminal resize event).
    #[allow(dead_code)]
    pub fn invalidate_render_cache(&self) {
        if let Ok(mut guard) = self.render_cache.lock() {
            guard.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_url_deterministic() {
        let h1 = ImageCacheManager::hash_url("https://example.com/test.png");
        let h2 = ImageCacheManager::hash_url("https://example.com/test.png");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_tier2_mem_cache() {
        let cache = get_image_cache();
        let img = DynamicImage::new_rgba8(4, 4);
        cache.put_mem_image("test_mem_url", img.clone());

        let retrieved = cache.get_mem_image("test_mem_url");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().width(), 4);
    }

    #[test]
    fn test_tier3_render_cache_invalidation() {
        let cache = get_image_cache();
        let span = crate::layout::ColoredSpan {
            text: "▀".into(),
            fg_rgb: (255, 0, 0),
            bg_rgb: (0, 0, 0),
        };
        cache.put_rendered_spans("test_render_url", 80, vec![vec![span]]);

        assert!(cache.get_rendered_spans("test_render_url", 80).is_some());

        cache.invalidate_render_cache();
        assert!(cache.get_rendered_spans("test_render_url", 80).is_none());
    }

    #[test]
    fn test_tier1_disk_cache() {
        let cache = get_image_cache();
        let dummy_bytes = b"fake image bytes";
        cache.put_disk_bytes("https://example.com/disk_test.png", dummy_bytes);

        let retrieved = cache.get_disk_bytes("https://example.com/disk_test.png");
        if cache.cache_dir.is_some() {
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap(), dummy_bytes);
        }
    }
}
