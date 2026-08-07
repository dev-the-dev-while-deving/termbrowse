//! Pluggable Terminal Image Renderers & Session-Wide Capability Caching.
//! Supports:
//! - Universal ANSI 24-bit TrueColor Half-Block (`▀`) renderer
//! - Grayscale ASCII art density renderer
//! - 8-dot Unicode Braille art renderer
//! - Kitty Graphics Protocol encoder
//! - Capability detection cached once per session using `std::sync::OnceLock`

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

/// Detect terminal capabilities ONCE per session.
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

/// Render a DynamicImage into a vector of Ratatui `Line`s for a target cell column width.
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

/// Kitty Graphics Protocol Renderer (`\x1b_G...`).
/// Transmits compressed PNG payload to Kitty / Ghostty / WezTerm graphics protocol handler.
fn render_kitty(img: &DynamicImage, target_cols: u32) -> Vec<Line<'static>> {
    let (orig_w, orig_h) = (img.width(), img.height());
    if orig_w == 0 || orig_h == 0 {
        return Vec::new();
    }

    let aspect = orig_h as f32 / orig_w as f32;
    let target_rows = ((target_cols as f32 * aspect * 0.5).max(1.0)) as u32;

    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    if img.write_to(&mut cursor, image::ImageFormat::Png).is_err() {
        return render_halfblock(img, target_cols);
    }

    let id = rand_id(png_bytes.len());
    let escape_seq = encode_kitty_graphics(&png_bytes, id, target_cols, target_rows);

    let mut lines = Vec::new();
    lines.push(Line::from(Span::raw(escape_seq)));

    for _ in 1..target_rows {
        lines.push(Line::from(Span::raw(" ".repeat(target_cols as usize))));
    }
    lines
}

fn rand_id(seed: usize) -> u32 {
    ((seed % 90000) + 10000) as u32
}

/// Helper to encode raw bytes to standard Base64 string.
pub fn encode_base64(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() { data[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(CHARSET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARSET[((triple >> 12) & 0x3F) as usize] as char);
        if i + 1 < data.len() {
            out.push(CHARSET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < data.len() {
            out.push(CHARSET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

/// Encodes binary payload into Kitty APC chunked escape sequences.
pub fn encode_kitty_graphics(
    bytes: &[u8],
    image_id: u32,
    cols: u32,
    rows: u32,
) -> String {
    let b64 = encode_base64(bytes);
    let mut esc = String::new();
    let chunk_size = 4096;
    let chunks: Vec<&str> = b64
        .as_bytes()
        .chunks(chunk_size)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect();

    for (idx, chunk) in chunks.iter().enumerate() {
        let is_last = idx + 1 == chunks.len();
        let m_flag = if is_last { 0 } else { 1 };
        if idx == 0 {
            esc.push_str(&format!(
                "\x1b_Ga=T,f=100,t=d,i={image_id},c={cols},r={rows},m={m_flag};{chunk}\x1b\\"
            ));
        } else {
            esc.push_str(&format!("\x1b_Gm={m_flag};{chunk}\x1b\\"));
        }
    }
    esc
}

/// Universal ANSI 24-bit TrueColor Half-Block Renderer (`▀`).
/// Terminal font cells have an aspect ratio of ~1:2 (height is roughly double width).
/// Each cell displays 2 vertical pixels: top pixel = FG color, bottom pixel = BG color.
fn render_halfblock(img: &DynamicImage, target_cols: u32) -> Vec<Line<'static>> {
    // 1 cell width = 2 pixel rows. Compute scaled pixel dimensions preserving aspect ratio.
    let (orig_w, orig_h) = (img.width(), img.height());
    if orig_w == 0 || orig_h == 0 {
        return Vec::new();
    }

    let target_pixel_w = target_cols;
    let aspect = orig_h as f32 / orig_w as f32;
    // Account for 1:2 cell aspect ratio -> pixel height = target_cols * aspect * 0.5 (rounded up to even)
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

/// Grayscale ASCII Art Density Renderer.
fn render_ascii(img: &DynamicImage, target_cols: u32) -> Vec<Line<'static>> {
    let charset = b"@%#*+=-:. ";
    let (orig_w, orig_h) = (img.width(), img.height());
    if orig_w == 0 || orig_h == 0 {
        return Vec::new();
    }

    let aspect = orig_h as f32 / orig_w as f32;
    let target_rows = (target_cols as f32 * aspect * 0.5).max(1.0) as u32;

    let resized = img.resize_exact(
        target_cols,
        target_rows,
        image::imageops::FilterType::Lanczos3,
    );
    let matrix = to_rgba_matrix(&resized);

    let mut lines = Vec::new();
    for row in matrix {
        let mut text = String::with_capacity(row.len());
        for px in row {
            let lum = (0.299 * px.r as f32 + 0.587 * px.g as f32 + 0.114 * px.b as f32) / 255.0;
            let idx = ((1.0 - lum) * (charset.len() - 1) as f32) as usize;
            text.push(charset[idx.min(charset.len() - 1)] as char);
        }
        lines.push(Line::from(Span::raw(text)));
    }
    lines
}

/// 8-dot Unicode Braille Art Renderer (`\u{2800}`).
fn render_braille(img: &DynamicImage, target_cols: u32) -> Vec<Line<'static>> {
    let (orig_w, orig_h) = (img.width(), img.height());
    if orig_w == 0 || orig_h == 0 {
        return Vec::new();
    }

    // 1 Braille cell = 2x4 dots
    let target_pixel_w = target_cols * 2;
    let aspect = orig_h as f32 / orig_w as f32;
    let target_pixel_h = ((target_cols as f32 * aspect).max(1.0) * 4.0) as u32;

    let resized = img.resize_exact(
        target_pixel_w,
        target_pixel_h,
        image::imageops::FilterType::Lanczos3,
    );
    let matrix = to_rgba_matrix(&resized);

    let mut lines = Vec::new();
    let row_count = matrix.len();
    let col_count = if row_count > 0 { matrix[0].len() } else { 0 };

    let mut y = 0;
    while y + 3 < row_count {
        let mut braille_str = String::new();
        let mut x = 0;
        while x + 1 < col_count {
            let mut pattern: u32 = 0;

            // Dot index offsets for Unicode Braille character encoding
            let dots = [
                (0, 0, 0x01), (0, 1, 0x02), (0, 2, 0x04), (0, 3, 0x40),
                (1, 0, 0x08), (1, 1, 0x10), (1, 2, 0x20), (1, 3, 0x80),
            ];

            for (dx, dy, mask) in dots {
                let px = matrix[y + dy][x + dx];
                let lum = 0.299 * px.r as f32 + 0.587 * px.g as f32 + 0.114 * px.b as f32;
                if lum > 128.0 {
                    pattern |= mask;
                }
            }

            let braille_char = char::from_u32(0x2800 + pattern).unwrap_or(' ');
            braille_str.push(braille_char);
            x += 2;
        }
        lines.push(Line::from(Span::styled(
            braille_str,
            Style::default().fg(Color::Green),
        )));
        y += 4;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_caps_cached_once() {
        let caps1 = get_terminal_caps();
        let caps2 = get_terminal_caps();
        assert_eq!(caps1.supports_truecolor, caps2.supports_truecolor);
    }

    #[test]
    fn test_render_halfblock_lines() {
        let img = DynamicImage::new_rgba8(20, 20);
        let lines = render_image_to_lines(&img, 10, RenderMode::HalfBlock);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_ascii_lines() {
        let img = DynamicImage::new_rgba8(20, 20);
        let lines = render_image_to_lines(&img, 10, RenderMode::Ascii);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_braille_lines() {
        let img = DynamicImage::new_rgba8(20, 20);
        let lines = render_image_to_lines(&img, 10, RenderMode::Braille);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_encode_base64() {
        assert_eq!(encode_base64(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn test_kitty_graphics_encoder() {
        let dummy_bytes = b"fake png payload";
        let esc = encode_kitty_graphics(dummy_bytes, 1001, 40, 20);
        assert!(esc.contains("\x1b_Ga=T,f=100,t=d,i=1001,c=40,r=20"));
        assert!(esc.contains("\x1b\\"));
    }
}
