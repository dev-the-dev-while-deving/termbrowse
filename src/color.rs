//! RGB → ANSI 256 + contrast. Site identity never uses truecolor.

use ratatui::style::Color;

/// Dark canvas we paint on (GrokNight bg).
pub const CANVAS: (u8, u8, u8) = (13, 13, 16);

/// Map an sRGB triple onto the 256-color cube / grayscale ramp.
pub fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    let rd = r.abs_diff(g);
    let gd = g.abs_diff(b);
    if rd < 12 && gd < 12 {
        let gray = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
        return match gray {
            0..=7 => 16,
            8..=238 => (232 + (gray - 8) / 10) as u8,
            _ => 231,
        };
    }
    let r6 = (u16::from(r) * 5 / 255) as u8;
    let g6 = (u16::from(g) * 5 / 255) as u8;
    let b6 = (u16::from(b) * 5 / 255) as u8;
    16 + 36 * r6 + 6 * g6 + b6
}

pub fn indexed(r: u8, g: u8, b: u8) -> Color {
    Color::Indexed(rgb_to_256(r, g, b))
}

/// Dim a color toward the canvas (for inactive chrome).
pub fn dim_rgb(r: u8, g: u8, b: u8) -> Color {
    indexed(r / 2 + 20, g / 2 + 16, b / 2 + 24)
}

fn channel(c: u8) -> f32 {
    let x = f32::from(c) / 255.0;
    if x <= 0.03928 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(r: u8, g: u8, b: u8) -> f32 {
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// WCAG contrast against the dark canvas. Need ≥ 3:1 or we keep the default.
pub fn contrast_ok(r: u8, g: u8, b: u8) -> bool {
    let l1 = luminance(r, g, b);
    let l2 = luminance(CANVAS.0, CANVAS.1, CANVAS.2);
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05) >= 3.0
}

pub fn parse_css_color(raw: &str) -> Option<(u8, u8, u8)> {
    let s = raw.trim().trim_end_matches(';').trim();
    if s.is_empty() || s.eq_ignore_ascii_case("inherit") || s.eq_ignore_ascii_case("transparent") {
        return None;
    }
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    let lower = s.to_ascii_lowercase();
    if let Some(inner) = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))
        .and_then(|t| t.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() >= 3 {
            let r = parse_rgb_channel(parts[0])?;
            let g = parse_rgb_channel(parts[1])?;
            let b = parse_rgb_channel(parts[2])?;
            return Some((r, g, b));
        }
    }
    named_color(&lower)
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim();
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some((r, g, b))
        }
        6 | 8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

fn parse_rgb_channel(s: &str) -> Option<u8> {
    let s = s.trim();
    if let Some(p) = s.strip_suffix('%') {
        let n: f32 = p.trim().parse().ok()?;
        return Some((n.clamp(0.0, 100.0) * 2.55) as u8);
    }
    s.parse::<f32>().ok().map(|n| n.clamp(0.0, 255.0) as u8)
}

fn named_color(name: &str) -> Option<(u8, u8, u8)> {
    Some(match name {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (220, 38, 38),
        "blue" => (37, 99, 235),
        "navy" => (30, 64, 175),
        "green" => (22, 163, 74),
        "teal" => (13, 148, 136),
        "purple" | "magenta" => (168, 85, 247),
        "orange" => (249, 115, 22),
        "gold" | "yellow" => (234, 179, 8),
        "crimson" => (220, 38, 38),
        "dodgerblue" | "royalblue" | "steelblue" => (59, 130, 246),
        "darkblue" => (30, 64, 175),
        "orangered" => (234, 88, 12),
        "mediumblue" => (37, 99, 235),
        "rebeccapurple" => (102, 51, 153),
        _ => return None,
    })
}

/// Accept a stolen color only if it stays readable on the dark canvas.
pub fn steal(raw: &str) -> Option<Color> {
    let (r, g, b) = parse_css_color(raw)?;
    if contrast_ok(r, g, b) {
        Some(indexed(r, g, b))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_rgb() {
        assert_eq!(parse_css_color("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_css_color("#3366cc"), Some((0x33, 0x66, 0xcc)));
        assert_eq!(parse_css_color("rgb(10, 20, 30)"), Some((10, 20, 30)));
    }

    #[test]
    fn rejects_dark_on_dark() {
        assert!(steal("#111111").is_none());
        assert!(steal("#e879f9").is_some());
    }

    #[test]
    fn cube_is_in_range() {
        assert!(rgb_to_256(232, 121, 249) >= 16);
    }
}
