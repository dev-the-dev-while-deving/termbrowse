# Complete Consolidated Codebase — `termbrowse`

This single document contains the complete, 100% un-truncated source code for `termbrowse` across all Rust modules and configuration files, organized and labeled for external AI review and analysis.

---

## File Sitemap & Table of Contents
1. [`Cargo.toml`](#file-cargotoml)
2. [`src/main.rs`](#file-srcmainrs)
3. [`src/model.rs`](#file-srcmodelrs)
4. [`src/parse.rs`](#file-srcparsers)
5. [`src/urlutil.rs`](#file-srcurlutilrs)
6. [`src/fetch.rs`](#file-srcfetchrs)
7. [`src/image_decoder.rs`](#file-srcimagedecoder-rs)
8. [`src/image_cache.rs`](#file-srcimagecachers)
9. [`src/render_engine.rs`](#file-srcrenderenginers)
10. [`src/layout.rs`](#file-srclayoutrs)
11. [`src/session.rs`](#file-srcsessionrs)
12. [`src/snapshot.rs`](#file-srcsnapshotrs)
13. [`src/theme.rs`](#file-srcthemers)
14. [`src/tui_session.rs`](#file-srctuisessionrs)

---

### File: `Cargo.toml`
```toml
[package]
name = "termbrowse"
version = "0.2.0"
edition = "2024"
description = "Custom interactive terminal web session — structure browser, no Chrome"
license = "MIT"

[dependencies]
anyhow = "1.0.104"
clap = { version = "4.6.5", features = ["derive"] }
crossterm = "0.29.0"
ratatui = "0.30.2"
reqwest = { version = "0.13.4", default-features = false, features = ["rustls", "gzip", "brotli", "deflate"] }
scraper = "0.27.0"
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0.19"
tokio = { version = "1.53.1", features = ["full"] }
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["env-filter"] }
unicode-width = "0.2.2"
url = "2.5.8"
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "webp", "gif"] }
sha2 = "0.10"
```
