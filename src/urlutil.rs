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
