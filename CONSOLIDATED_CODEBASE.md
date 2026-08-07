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

---

### File: `src/render_engine.rs`
```rust
//! Pluggable Terminal Image Renderers & Session-Wide Capability Caching.

use crate::image_decoder::{DynamicImage, to_rgba_matrix};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    HalfBlock,
    Ascii,
    Braille,
    Kitty,
}

#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    #[allow(dead_code)]
    pub supports_truecolor: bool,
    pub supports_kitty: bool,
    #[allow(dead_code)]
    pub preferred_mode: RenderMode,
}

static CAPS_INSTANCE: OnceLock<TerminalCapabilities> = OnceLock::new();

pub fn get_terminal_caps() -> &'static TerminalCapabilities {
    CAPS_INSTANCE.get_or_init(|| {
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        let term = std::env::var("TERM").unwrap_or_default();
        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();

        let supports_truecolor = colorterm.contains("truecolor")
            || colorterm.contains("24bit")
            || term_program.contains("iTerm")
            || term_program.contains("Ghostty")
            || term_program.contains("WezTerm")
            || term.contains("kitty");

        let supports_kitty = term.contains("kitty")
            || term_program.contains("Ghostty")
            || term_program.contains("WezTerm");

        let preferred_mode = if supports_kitty {
            RenderMode::Kitty
        } else {
            RenderMode::HalfBlock
        };

        TerminalCapabilities {
            supports_truecolor,
            supports_kitty,
            preferred_mode,
        }
    })
}

pub fn render_image_to_lines(
    img: &DynamicImage,
    target_cols: u16,
    mode: RenderMode,
) -> Vec<Line<'static>> {
    let target_cols = target_cols.max(10) as u32;

    match mode {
        RenderMode::HalfBlock => render_halfblock(img, target_cols),
        RenderMode::Kitty => {
            let caps = get_terminal_caps();
            if caps.supports_kitty {
                render_kitty(img, target_cols)
            } else {
                render_halfblock(img, target_cols)
            }
        }
        RenderMode::Ascii => render_ascii(img, target_cols),
        RenderMode::Braille => render_braille(img, target_cols),
    }
}

fn render_halfblock(img: &DynamicImage, target_cols: u32) -> Vec<Line<'static>> {
    let (orig_w, orig_h) = (img.width(), img.height());
    if orig_w == 0 || orig_h == 0 {
        return Vec::new();
    }

    let target_pixel_w = target_cols;
    let aspect = orig_h as f32 / orig_w as f32;
    let target_pixel_h = ((target_cols as f32 * aspect * 0.5).max(1.0) * 2.0) as u32;

    let resized = img.resize_exact(
        target_pixel_w,
        target_pixel_h,
        image::imageops::FilterType::Lanczos3,
    );
    let matrix = to_rgba_matrix(&resized);

    let mut lines = Vec::new();
    let row_count = matrix.len();

    let mut y = 0;
    while y + 1 < row_count {
        let top_row = &matrix[y];
        let bot_row = &matrix[y + 1];

        let mut spans = Vec::with_capacity(top_row.len());
        for x in 0..top_row.len() {
            let (top_r, top_g, top_b) = blend_pixel(top_row[x]);
            let (bot_r, bot_g, bot_b) = blend_pixel(bot_row[x]);

            let fg = Color::Rgb(top_r, top_g, top_b);
            let bg = Color::Rgb(bot_r, bot_g, bot_b);

            spans.push(Span::styled(
                "▀",
                Style::default().fg(fg).bg(bg),
            ));
        }
        lines.push(Line::from(spans));
        y += 2;
    }
    lines
}

fn blend_pixel(px: crate::image_decoder::RgbaPixel) -> (u8, u8, u8) {
    let a = px.a as u16;
    if a == 255 {
        (px.r, px.g, px.b)
    } else if a == 0 {
        (13, 13, 16)
    } else {
        let r = ((px.r as u16 * a + 13 * (255 - a)) / 255) as u8;
        let g = ((px.g as u16 * a + 13 * (255 - a)) / 255) as u8;
        let b = ((px.b as u16 * a + 13 * (255 - a)) / 255) as u8;
        (r, g, b)
    }
}
```
