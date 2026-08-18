//! GrokNight chrome + optional site identity overlay.

use crate::color::{dim_rgb, indexed};
use crate::model::SiteIdentity;
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub bg_panel: Color,
    pub accent: Color,
    pub accent_dim: Color,
    pub text: Color,
    pub text_dim: Color,
    pub heading: Color,
    pub link: Color,
    pub link_active: Color,
    pub code: Color,
    pub quote: Color,
    pub border: Color,
    #[allow(dead_code)]
    pub success: Color,
    #[allow(dead_code)]
    pub warn: Color,
}

#[allow(dead_code)]
impl Theme {
    pub fn groknight() -> Self {
        Self {
            bg: indexed(13, 13, 16),
            bg_panel: indexed(24, 24, 30),
            accent: indexed(232, 121, 249),
            accent_dim: indexed(120, 70, 140),
            text: indexed(232, 230, 227),
            text_dim: indexed(120, 118, 115),
            heading: indexed(250, 250, 249),
            link: indexed(192, 132, 252),
            link_active: indexed(253, 224, 71),
            code: indexed(134, 239, 172),
            quote: indexed(161, 161, 170),
            border: indexed(50, 50, 58),
            success: indexed(74, 222, 128),
            warn: indexed(251, 191, 36),
        }
    }

    /// Keep the dark canvas. Overlay stolen site colors when present.
    pub fn with_identity(mut self, id: &SiteIdentity) -> Self {
        if let Some(c) = id.link {
            self.link = c;
        }
        if let Some(c) = id.heading {
            self.heading = c;
        }
        if let Some(c) = id.accent {
            self.accent = c;
            if let Color::Indexed(n) = c {
                let (r, g, b) = ansi256_to_approx_rgb(n);
                self.accent_dim = dim_rgb(r, g, b);
            }
        }
        self
    }

    pub fn title_bar(&self) -> Style {
        Style::new().bg(self.bg_panel).fg(self.text)
    }

    pub fn title_accent(&self) -> Style {
        Style::new()
            .bg(self.bg_panel)
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_bar(&self) -> Style {
        Style::new().bg(self.bg_panel).fg(self.text_dim)
    }

    pub fn body_bg(&self) -> Style {
        Style::new().bg(self.bg).fg(self.text)
    }

    pub fn accent_rail(&self) -> Style {
        Style::new().fg(self.accent).bg(self.bg)
    }

    pub fn accent_rail_dim(&self) -> Style {
        Style::new().fg(self.accent_dim).bg(self.bg)
    }

    pub fn heading(&self, level: u8) -> Style {
        let base = Style::new().fg(self.heading).bg(self.bg);
        if level <= 1 {
            base.add_modifier(Modifier::BOLD)
        } else {
            base
        }
    }

    pub fn text(&self) -> Style {
        Style::new().fg(self.text).bg(self.bg)
    }

    pub fn dim(&self) -> Style {
        Style::new().fg(self.text_dim).bg(self.bg)
    }

    pub fn link(&self, active: bool) -> Style {
        if active {
            Style::new()
                .fg(self.link_active)
                .bg(self.bg_panel)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::new()
                .fg(self.link)
                .bg(self.bg)
                .add_modifier(Modifier::UNDERLINED)
        }
    }

    pub fn code(&self) -> Style {
        Style::new().fg(self.code).bg(self.bg)
    }

    pub fn quote(&self) -> Style {
        Style::new()
            .fg(self.quote)
            .bg(self.bg)
            .add_modifier(Modifier::ITALIC)
    }

    pub fn strong(&self) -> Style {
        Style::new()
            .fg(self.heading)
            .bg(self.bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn em(&self) -> Style {
        Style::new()
            .fg(self.text)
            .bg(self.bg)
            .add_modifier(Modifier::ITALIC)
    }

    pub fn border(&self) -> Style {
        Style::new().fg(self.border).bg(self.bg)
    }

    pub fn image(&self) -> Style {
        Style::new().fg(self.text_dim).bg(self.bg)
    }
}

/// Reverse the 256 cube well enough to dim an Indexed accent.
fn ansi256_to_approx_rgb(n: u8) -> (u8, u8, u8) {
    match n {
        0..=15 => (128, 128, 128),
        16..=231 => {
            let i = n - 16;
            let r = i / 36;
            let g = (i % 36) / 6;
            let b = i % 6;
            let step = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            (step(r), step(g), step(b))
        }
        232..=255 => {
            let g = 8 + 10 * (n - 232);
            (g, g, g)
        }
    }
}
