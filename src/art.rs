//! Shared chrome/panel colors for the TUI.

use crate::color::indexed;
use ratatui::style::{Color, Style};

pub fn chrome_bg() -> Color {
    indexed(13, 13, 16)
}

pub fn panel_bg() -> Color {
    indexed(24, 24, 30)
}

pub fn style_chrome_text(fg: Color) -> Style {
    Style::new().fg(fg).bg(chrome_bg())
}

pub fn style_panel(fg: Color) -> Style {
    Style::new().fg(fg).bg(panel_bg())
}
