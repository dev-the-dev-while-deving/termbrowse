//! Terminal raster of a **720p** framebuffer.
//!
//! Half-block cells map **1 column ≈ 1 source pixel** and **1 row ≈ 2 source pixels**,
//! so a 1280×720 capture becomes a 1280×360 cell buffer. The TUI pans a window over
//! that full-resolution CRT — we no longer crush the whole page into one screen.

use image::imageops::FilterType;
use image::{Rgb, RgbImage};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

use crate::chrome::{VIEW_H, VIEW_W};

/// How colors are mapped onto the “tube”.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phosphor {
    /// Keep screenshot colors, still scanline-dimmed.
    #[default]
    Color,
    /// Classic green phosphor.
    Green,
    /// Amber / orange tube.
    Amber,
    /// White/grey mono.
    Mono,
}

impl Phosphor {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "color" | "rgb" => Some(Self::Color),
            "green" | "crt" => Some(Self::Green),
            "amber" | "orange" => Some(Self::Amber),
            "mono" | "bw" | "white" => Some(Self::Mono),
            _ => None,
        }
    }
}

/// Pre-rendered terminal image (rows of half-block cells) — full 720p raster when possible.
#[derive(Debug, Clone)]
pub struct TermImage {
    pub cols: u16,
    pub rows: u16,
    /// Source pixel size this was built from.
    pub src_w: u32,
    pub src_h: u32,
    /// row-major, length = cols * rows
    pub cells: Vec<HalfCell>,
}

#[derive(Debug, Clone, Copy)]
pub struct HalfCell {
    pub top: Rgb<u8>,
    pub bottom: Rgb<u8>,
}

impl TermImage {
    /// Build a **full-resolution** CRT raster from a 720p (or any) PNG.
    ///
    /// - Width: 1 terminal cell per source pixel (capped at source width).
    /// - Height: 1 terminal row per 2 source pixels (half-block).
    /// - No “fit entire page on screen” downscale — pan instead.
    pub fn from_png_720p(png: &[u8], phosphor: Phosphor) -> anyhow::Result<Self> {
        let img = image::load_from_memory(png)?.to_rgb8();
        Self::from_rgb_native(&img, phosphor)
    }

    /// Optional soft scale if source isn't exactly 720p — still max detail.
    pub fn from_rgb_native(img: &RgbImage, phosphor: Phosphor) -> anyhow::Result<Self> {
        let (ow, oh) = img.dimensions();
        if ow == 0 || oh == 0 {
            anyhow::bail!("empty image");
        }

        // Normalize to 720p canvas when close, or keep native if already that size.
        let img = if ow != VIEW_W || oh != VIEW_H {
            image::imageops::resize(img, VIEW_W, VIEW_H, FilterType::Lanczos3)
        } else {
            img.clone()
        };
        let (ow, oh) = img.dimensions();

        // Even height for half-blocks.
        let oh = oh - (oh % 2);
        let cols = ow as u16;
        let rows = (oh / 2) as u16;
        let mut cells = Vec::with_capacity((cols as usize) * (rows as usize));

        for row in 0..rows as u32 {
            let scan_dim = if row % 2 == 1 { 0.88 } else { 1.0 };
            for col in 0..cols as u32 {
                let top = grade(*img.get_pixel(col, row * 2), phosphor, scan_dim);
                let bottom = grade(
                    *img.get_pixel(col, row * 2 + 1),
                    phosphor,
                    scan_dim * 0.94,
                );
                cells.push(HalfCell { top, bottom });
            }
        }

        Ok(Self {
            cols,
            rows,
            src_w: ow,
            src_h: oh,
            cells,
        })
    }

    /// Legacy fit-to-box (used only if needed). Prefer `from_png_720p`.
    pub fn from_png(
        png: &[u8],
        max_cols: u16,
        max_rows: u16,
        phosphor: Phosphor,
    ) -> anyhow::Result<Self> {
        let img = image::load_from_memory(png)?.to_rgb8();
        Self::from_rgb(&img, max_cols, max_rows, phosphor)
    }

    pub fn from_rgb(
        img: &RgbImage,
        max_cols: u16,
        max_rows: u16,
        phosphor: Phosphor,
    ) -> anyhow::Result<Self> {
        let max_cols = max_cols.max(4) as u32;
        let max_rows = max_rows.max(2) as u32;
        let target_w = max_cols;
        let target_h = max_rows * 2;

        let (ow, oh) = img.dimensions();
        if ow == 0 || oh == 0 {
            anyhow::bail!("empty image");
        }

        let scale = (target_w as f32 / ow as f32).min(target_h as f32 / oh as f32);
        let nw = ((ow as f32 * scale).round() as u32).max(1);
        let nh = ((oh as f32 * scale).round() as u32).max(2);
        let nh = nh + (nh % 2);

        let resized = image::imageops::resize(img, nw, nh, FilterType::Lanczos3);
        let cols = nw as u16;
        let rows = (nh / 2) as u16;
        let mut cells = Vec::with_capacity((cols * rows) as usize);

        for row in 0..rows as u32 {
            let scan_dim = if row % 2 == 1 { 0.82 } else { 1.0 };
            for col in 0..cols as u32 {
                let top = grade(*resized.get_pixel(col, row * 2), phosphor, scan_dim);
                let bottom = grade(
                    *resized.get_pixel(col, row * 2 + 1),
                    phosphor,
                    scan_dim * 0.92,
                );
                cells.push(HalfCell { top, bottom });
            }
        }

        Ok(Self {
            cols,
            rows,
            src_w: ow,
            src_h: oh,
            cells,
        })
    }

    pub fn cell_at(&self, col: u16, row: u16) -> Option<HalfCell> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        self.cells
            .get((row as usize) * (self.cols as usize) + (col as usize))
            .copied()
    }
}

fn luma(c: Rgb<u8>) -> f32 {
    (0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32) / 255.0
}

fn grade(c: Rgb<u8>, phosphor: Phosphor, dim: f32) -> Rgb<u8> {
    let l = luma(c);
    let l = (l.powf(0.9) * dim).clamp(0.0, 1.0);
    let (r, g, b) = match phosphor {
        Phosphor::Color => {
            let f = dim;
            (
                (c[0] as f32 * f).clamp(0.0, 255.0),
                (c[1] as f32 * f).clamp(0.0, 255.0),
                (c[2] as f32 * f).clamp(0.0, 255.0),
            )
        }
        Phosphor::Green => {
            let v = l * 255.0;
            (v * 0.15, v * 1.0, v * 0.25)
        }
        Phosphor::Amber => {
            let v = l * 255.0;
            (v * 1.0, v * 0.65, v * 0.12)
        }
        Phosphor::Mono => {
            let v = l * 255.0;
            (v * 0.95, v * 0.95, v * 1.0)
        }
    };
    Rgb([r as u8, g as u8, b as u8])
}

/// Widget: paints a window into the full 720p CRT buffer.
pub struct TermImageWidget<'a> {
    pub image: &'a TermImage,
    /// Pan origin in image cells (x = columns, y = rows).
    pub pan_x: u16,
    pub pan_y: u16,
    /// CRT progressive scan within the *visible* window.
    pub scan_rows: u16,
    pub show_beam: bool,
}

impl Widget for TermImageWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let img = self.image;
        let full = self.scan_rows == u16::MAX;
        let painted = if full {
            area.height
        } else {
            self.scan_rows.min(area.height)
        };

        // Black tube under unscanned / empty region.
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(dest) = buf.cell_mut((area.x + x, area.y + y)) {
                    dest.set_symbol(" ");
                    dest.set_style(Style::default().bg(Color::Black).fg(Color::Black));
                }
            }
        }

        for y in 0..painted {
            let src_row = self.pan_y as u32 + y as u32;
            if src_row >= img.rows as u32 {
                break;
            }
            let is_beam = self.show_beam && !full && y + 1 == painted;

            for x in 0..area.width {
                let src_col = self.pan_x as u32 + x as u32;
                if src_col >= img.cols as u32 {
                    break;
                }
                let Some(cell) = img.cell_at(src_col as u16, src_row as u16) else {
                    continue;
                };
                let (mut fg, mut bg) = (rgb_to_color(cell.top), rgb_to_color(cell.bottom));
                if is_beam {
                    fg = Color::Rgb(220, 255, 220);
                    bg = Color::Rgb(40, 80, 40);
                }
                if let Some(dest) = buf.cell_mut((area.x + x, area.y + y)) {
                    dest.set_symbol("▀");
                    dest.set_style(Style::default().fg(fg).bg(bg));
                }
            }
        }
    }
}

fn rgb_to_color(c: Rgb<u8>) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}
