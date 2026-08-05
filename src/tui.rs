//! Interactive terminal UI — keyboard-first document browser.

use crate::fetch::fetch_url;
use crate::layout::{self, Layout as DocLayout, Segment, Style as DocStyle};
use crate::model::{Document, Ref};
use crate::parse::parse_html;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::time::Duration;

pub struct App {
    doc: Document,
    layout: DocLayout,
    scroll: u16,
    selected_link: usize,
    status: String,
    width: u16,
}

impl App {
    pub fn from_document(doc: Document, width: u16) -> Self {
        let mut layout = layout::layout_document(&doc, width.saturating_sub(2));
        // Stash layout timing.
        // (layout_ms set lightly here)
        let _ = &mut layout;
        let status = format!(
            "links:{}  fetch:{}ms parse:{}ms  j/k scroll  tab link  enter open  r reload  q quit",
            doc.links.len(),
            doc.timing_ms.fetch_ms,
            doc.timing_ms.parse_ms
        );
        Self {
            doc,
            layout,
            scroll: 0,
            selected_link: 0,
            status,
            width,
        }
    }

    fn relayout(&mut self, width: u16) {
        self.width = width;
        self.layout = layout::layout_document(&self.doc, width.saturating_sub(2));
        self.clamp_scroll(0);
    }

    fn clamp_scroll(&mut self, view_h: u16) {
        let max = self.layout.lines.len().saturating_sub(view_h as usize) as u16;
        if self.scroll > max {
            self.scroll = max;
        }
    }

    fn selected_ref(&self) -> Option<Ref> {
        self.layout.link_order.get(self.selected_link).copied()
    }

    async fn navigate(&mut self, href: &str) -> Result<()> {
        let Some(url) = self.doc.resolve_href(href) else {
            self.status = format!("bad url: {href}");
            return Ok(());
        };
        self.status = format!("loading {url} …");
        let fetched = fetch_url(url.as_str()).await?;
        let doc = parse_html(&fetched.url, &fetched.body, fetched.fetch_ms);
        self.doc = doc;
        self.scroll = 0;
        self.selected_link = 0;
        self.relayout(self.width);
        self.status = format!(
            "{}  ·  {} links  ·  {} bytes  ·  {}ms",
            self.doc.title,
            self.doc.links.len(),
            fetched.bytes,
            fetched.fetch_ms
        );
        Ok(())
    }

    async fn reload(&mut self) -> Result<()> {
        let url = self.doc.url.clone();
        self.navigate(&url).await
    }
}

pub async fn run(initial: Document) -> Result<()> {
    let mut terminal = ratatui::init();
    let width = terminal
        .size()
        .map(|s| s.width)
        .unwrap_or(100);
    let mut app = App::from_document(initial, width);

    let result = event_loop(&mut terminal, &mut app).await;
    ratatui::restore();
    result
}

async fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, app))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        let height = terminal.size().map(|s| s.height).unwrap_or(24);
        let view_h = height.saturating_sub(3);
        app.clamp_scroll(view_h);

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
            KeyCode::Char('j') | KeyCode::Down => {
                app.scroll = app.scroll.saturating_add(1);
                app.clamp_scroll(view_h);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.scroll = app.scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                app.scroll = app.scroll.saturating_add(view_h);
                app.clamp_scroll(view_h);
            }
            KeyCode::PageUp => {
                app.scroll = app.scroll.saturating_sub(view_h);
            }
            KeyCode::Home | KeyCode::Char('g') => app.scroll = 0,
            KeyCode::End | KeyCode::Char('G') => {
                app.scroll = app
                    .layout
                    .lines
                    .len()
                    .saturating_sub(view_h as usize) as u16;
            }
            KeyCode::Tab | KeyCode::Char('n') => {
                if !app.layout.link_order.is_empty() {
                    app.selected_link = (app.selected_link + 1) % app.layout.link_order.len();
                    ensure_link_visible(app, view_h);
                }
            }
            KeyCode::BackTab | KeyCode::Char('p') => {
                if !app.layout.link_order.is_empty() {
                    if app.selected_link == 0 {
                        app.selected_link = app.layout.link_order.len() - 1;
                    } else {
                        app.selected_link -= 1;
                    }
                    ensure_link_visible(app, view_h);
                }
            }
            KeyCode::Enter => {
                if let Some(r) = app.selected_ref() {
                    if let Some(link) = app.doc.resolve_link(r).map(|l| l.href.clone()) {
                        if let Err(e) = app.navigate(&link).await {
                            app.status = format!("error: {e:#}");
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                if let Err(e) = app.reload().await {
                    app.status = format!("reload error: {e:#}");
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                // Jump to link eN by typing number then... keep simple: 1-9 select index.
                let n = c.to_digit(10).unwrap() as usize;
                if n >= 1 && n <= app.layout.link_order.len() {
                    app.selected_link = n - 1;
                    ensure_link_visible(app, view_h);
                }
            }
            _ => {}
        }

        // Handle resize via next draw size.
        let w = terminal.size()?.width;
        if w != app.width {
            app.relayout(w);
        }
    }
    Ok(())
}

fn ensure_link_visible(app: &mut App, view_h: u16) {
    let Some(r) = app.selected_ref() else {
        return;
    };
    // Find first line containing this link ref.
    for (i, line) in app.layout.lines.iter().enumerate() {
        let has = line.segments.iter().any(|s| {
            matches!(s, Segment::Link { r#ref, .. } if *r#ref == r)
        });
        if has {
            let i = i as u16;
            if i < app.scroll {
                app.scroll = i;
            } else if i >= app.scroll + view_h {
                app.scroll = i.saturating_sub(view_h.saturating_sub(1));
            }
            break;
        }
    }
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = ratatui::layout::Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    // Title bar
    let title = format!(" termbrowse  │  {}  │  {}", app.doc.title, app.doc.url);
    frame.render_widget(
        Paragraph::new(title).style(
            ratatui::style::Style::new()
                .bg(Color::Blue)
                .fg(Color::White),
        ),
        chunks[0],
    );

    // Body
    let selected = app.selected_ref();
    let view_h = chunks[1].height as usize;
    let start = app.scroll as usize;
    let end = (start + view_h).min(app.layout.lines.len());

    let mut text_lines: Vec<Line> = Vec::new();
    for line in &app.layout.lines[start..end] {
        let mut spans = Vec::new();
        for seg in &line.segments {
            match seg {
                Segment::Text { text, style } => {
                    spans.push(ratatui::text::Span::styled(
                        text.clone(),
                        style_of(*style),
                    ));
                }
                Segment::Link { r#ref, text } => {
                    let mut st = ratatui::style::Style::new()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::UNDERLINED);
                    if Some(*r#ref) == selected {
                        st = st
                            .bg(Color::DarkGray)
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD);
                    }
                    spans.push(ratatui::text::Span::styled(text.clone(), st));
                }
            }
        }
        if spans.is_empty() {
            spans.push(ratatui::text::Span::raw(""));
        }
        text_lines.push(Line::from(spans));
    }

    let body = Paragraph::new(text_lines)
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[1]);

    // Status
    let link_info = match app.selected_ref().and_then(|r| app.doc.resolve_link(r)) {
        Some(l) => format!(" [{}] {} → {}", l.r#ref, l.text, l.href),
        None => String::new(),
    };
    let status = format!("{}{}", app.status, link_info);
    frame.render_widget(
        Paragraph::new(status).style(
            ratatui::style::Style::new()
                .bg(Color::DarkGray)
                .fg(Color::White),
        ),
        chunks[2],
    );
}

fn style_of(style: DocStyle) -> ratatui::style::Style {
    match style {
        DocStyle::Normal => ratatui::style::Style::new(),
        DocStyle::Heading1 => ratatui::style::Style::new()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        DocStyle::Heading2 => ratatui::style::Style::new()
            .fg(Color::LightYellow)
            .add_modifier(Modifier::BOLD),
        DocStyle::Heading3 => ratatui::style::Style::new()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
        DocStyle::Dim => ratatui::style::Style::new().fg(Color::DarkGray),
        DocStyle::Quote => ratatui::style::Style::new()
            .fg(Color::Gray)
            .add_modifier(Modifier::ITALIC),
        DocStyle::Pre => ratatui::style::Style::new().fg(Color::Green),
    }
}
