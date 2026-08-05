//! GrokNight-inspired palette — dense terminal UI, magenta accent, dark base.
//! Looks are secondary; readability + speed of scanning is the point.

use ratatui::style::{Color, Modifier, Style};

/// Product theme slots (aligned with Grok Build language, not pixel-perfect clone).
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
    pub success: Color,
    pub warn: Color,
}

impl Theme {
    /// Default product theme — dark + magenta (GrokNight-ish).
    pub fn groknight() -> Self {
        Self {
            bg: Color::Rgb(13, 13, 16),
            bg_panel: Color::Rgb(24, 24, 30),
            accent: Color::Rgb(232, 121, 249),       // magenta
            accent_dim: Color::Rgb(120, 70, 140),
            text: Color::Rgb(232, 230, 227),
            text_dim: Color::Rgb(120, 118, 115),
            heading: Color::Rgb(250, 250, 249),
            link: Color::Rgb(192, 132, 252),
            link_active: Color::Rgb(253, 224, 71), // yellow focus
            code: Color::Rgb(134, 239, 172),
            quote: Color::Rgb(161, 161, 170),
            border: Color::Rgb(50, 50, 58),
            success: Color::Rgb(74, 222, 128),
            warn: Color::Rgb(251, 191, 36),
        }
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
}
