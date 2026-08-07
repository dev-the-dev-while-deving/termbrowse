//! Small URL helpers (no browser engine).

use anyhow::{Result, bail};

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

pub fn resolve_image_url(base_url: &str, img_src: &str) -> String {
    let src = img_src.trim();
    if src.is_empty() {
        return String::new();
    }
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        return src.to_string();
    }
    if let Ok(base) = url::Url::parse(base_url) {
        if let Ok(joined) = base.join(src) {
            return joined.to_string();
        }
    }
    src.to_string()
}
