//! Grok-density session TUI — structure blocks, accent rails, interactive links.
//! Performance: no pixel paint. Same Document as agent snapshot.

use crate::layout::{self, Layout as DocLayout, Segment, Style as LayStyle};
use crate::model::{Document, Ref};
use crate::session::{LoadSource, Session};
use crate::theme::Theme;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Wrap};
use std::time::Duration;

pub struct App {
    session: Session,
    theme: Theme,
    layout: DocLayout,
    scroll: u16,
    selected_link: usize,
    status: String,
    width: u16,
    /// Optional URL input mode.
    input: Option<String>,
}

impl App {
    pub async fn open(url: &str, escalate: bool) -> Result<Self> {
        let mut session = Session::new(escalate);
        let page = session.open(url).await?;
        let theme = Theme::groknight();
        let status = status_for(page);
        let mut app = Self {
            session,
            theme,
            layout: DocLayout {
                width: 0,
                lines: vec![],
                link_order: vec![],
            },
            scroll: 0,
            selected_link: 0,
            status,
            width: 80,
            input: None,
        };
        app.relayout(80);
        Ok(app)
    }

    fn doc(&self) -> &Document {
        &self
            .session
            .current()
            .expect("session has page")
            .doc
    }

    fn relayout(&mut self, width: u16) {
        self.width = width;
        // Content width leaves room for accent rail + pad.
        let content_w = width.saturating_sub(4).max(20);
        self.layout = layout::layout_document(self.doc(), content_w);
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

    async fn navigate_selected(&mut self) -> Result<()> {
        let Some(r) = self.selected_ref() else {
            self.status = "no link selected — tab to pick".into();
            return Ok(());
        };
        let href = match self.doc().resolve_link(r) {
            Some(l) => l.href.clone(),
            None => {
                self.status = "link gone".into();
                return Ok(());
            }
        };
        self.status = format!("opening {href} …");
        let page = self.session.follow_href(&href).await?;
        self.status = status_for(page);
        self.scroll = 0;
        self.selected_link = 0;
        self.relayout(self.width);
        Ok(())
    }

    async fn go(&mut self, url: &str) -> Result<()> {
        self.status = format!("loading {url} …");
        let page = self.session.open(url).await?;
        self.status = status_for(page);
        self.scroll = 0;
        self.selected_link = 0;
        self.relayout(self.width);
        Ok(())
    }

    async fn reload(&mut self) -> Result<()> {
        self.status = "reloading…".into();
        let page = self.session.reload().await?;
        self.status = status_for(page);
        self.scroll = 0;
        self.relayout(self.width);
        Ok(())
    }

    fn back(&mut self) {
        if let Some(page) = self.session.back() {
            self.status = status_for(page);
            self.scroll = 0;
            self.selected_link = 0;
            self.relayout(self.width);
        } else {
            self.status = "no history back".into();
        }
    }

    fn forward(&mut self) {
        if let Some(page) = self.session.forward() {
            self.status = status_for(page);
            self.scroll = 0;
            self.selected_link = 0;
            self.relayout(self.width);
        } else {
            self.status = "no history forward".into();
        }
    }
}

fn status_for(page: &crate::session::LoadedPage) -> String {
    let src = match page.source {
        LoadSource::Structure => "structure",
        LoadSource::Escalated => "escalated",
    };
    format!(
        "{src} · {}ms · {} links · {} blocks · h history · tab link · enter open · : url · q quit",
        page.total_ms,
        page.doc.links.len(),
        page.doc.blocks.len()
    )
}

pub async fn run(url: &str, escalate: bool) -> Result<()> {
    let mut terminal = ratatui::init();
    let size = terminal.size().unwrap_or(Size {
        width: 100,
        height: 30,
    });

    let mut app = match App::open(url, escalate).await {
        Ok(mut a) => {
            a.relayout(size.width);
            a
        }
        Err(e) => {
            ratatui::restore();
            return Err(e);
        }
    };

    let result = event_loop(&mut terminal, &mut app).await;
    ratatui::restore();
    result
}

async fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        if !event::poll(Duration::from_millis(80))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        let height = terminal.size()?.height;
        let view_h = height.saturating_sub(3);
        app.clamp_scroll(view_h);

        // URL input mode
        if let Some(buf) = app.input.as_mut() {
            match key.code {
                KeyCode::Esc => {
                    app.input = None;
                    app.status = "cancelled".into();
                }
                KeyCode::Enter => {
                    let url = buf.trim().to_string();
                    app.input = None;
                    if !url.is_empty() {
                        if let Err(e) = app.go(&url).await {
                            app.status = format!("error: {e:#}");
                        }
                    }
                }
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Char(c) => buf.push(c),
                _ => {}
            }
            continue;
        }

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
                if let Err(e) = app.navigate_selected().await {
                    app.status = format!("nav error: {e:#}");
                }
            }
            KeyCode::Char('r') => {
                if let Err(e) = app.reload().await {
                    app.status = format!("reload error: {e:#}");
                }
            }
            KeyCode::Char('h') | KeyCode::Left
                if key.modifiers.contains(KeyModifiers::ALT)
                    || matches!(key.code, KeyCode::Char('h')) =>
            {
                // 'h' alone = history back (vim-ish); avoid clobbering left if we use it later
                if matches!(key.code, KeyCode::Char('h')) {
                    app.back();
                }
            }
            KeyCode::Char('[') => app.back(),
            KeyCode::Char(']') => app.forward(),
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.forward();
            }
            KeyCode::Char(':') | KeyCode::Char('o') => {
                app.input = Some(String::new());
                app.status = "open url — type and enter".into();
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let n = c.to_digit(10).unwrap() as usize;
                if n >= 1 && n <= app.layout.link_order.len() {
                    app.selected_link = n - 1;
                    ensure_link_visible(app, view_h);
                }
            }
            _ => {}
        }

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
    let th = &app.theme;
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);

    // ── Title ──
    let page = app.session.current().unwrap();
    let src = match page.source {
        LoadSource::Structure => "structure",
        LoadSource::Escalated => "escalated",
    };
    let back = if app.session.can_back() { "◀" } else { " " };
    let fwd = if app.session.can_forward() { "▶" } else { " " };
    let title = Line::from(vec![
        Span::styled(" termbrowse ", th.title_accent()),
        Span::styled(format!(" {back}{fwd} "), th.title_bar()),
        Span::styled(
            format!("{} ", truncate(&page.doc.title, 40)),
            th.title_bar().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} ", truncate(&page.doc.url, 48)),
            Style::new().bg(th.bg_panel).fg(th.text_dim),
        ),
        Span::styled(format!(" {src} "), Style::new().bg(th.bg_panel).fg(th.success)),
    ]);
    frame.render_widget(Paragraph::new(title).style(th.title_bar()), chunks[0]);

    // ── Body with accent rails ──
    let selected = app.selected_ref();
    let view_h = chunks[1].height as usize;
    let start = app.scroll as usize;
    let end = (start + view_h).min(app.layout.lines.len());

    let mut lines: Vec<Line> = Vec::new();
    for line in &app.layout.lines[start..end] {
        let mut spans = vec![
            Span::styled("▎", th.accent_rail()),
            Span::styled(" ", th.body_bg()),
        ];
        for seg in &line.segments {
            match seg {
                Segment::Text { text, style } => {
                    let st = match style {
                        LayStyle::Heading1 | LayStyle::Heading2 | LayStyle::Heading3 => {
                            // level approximated by style variant
                            th.heading(match style {
                                LayStyle::Heading1 => 1,
                                LayStyle::Heading2 => 2,
                                _ => 3,
                            })
                        }
                        LayStyle::Dim => th.dim(),
                        LayStyle::Quote => th.quote(),
                        LayStyle::Pre => th.code(),
                        LayStyle::Normal => th.text(),
                    };
                    spans.push(Span::styled(text.clone(), st));
                }
                Segment::Link { r#ref, text } => {
                    let active = Some(*r#ref) == selected;
                    spans.push(Span::styled(text.clone(), th.link(active)));
                }
            }
        }
        if spans.len() == 2 {
            spans.push(Span::styled("", th.text()));
        }
        lines.push(Line::from(spans));
    }

    // Fill remaining with bg
    while lines.len() < view_h {
        lines.push(Line::from(vec![
            Span::styled("▎", th.accent_rail_dim()),
            Span::styled(" ", th.body_bg()),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .style(th.body_bg())
            .wrap(Wrap { trim: false }),
        chunks[1],
    );

    // ── Status / prompt ──
    let link_info = match app.selected_ref().and_then(|r| app.doc().resolve_link(r)) {
        Some(l) => format!(" ◆ [{}] {} → {}", l.r#ref, truncate(&l.text, 32), truncate(&l.href, 40)),
        None => String::new(),
    };

    let prompt_line = if let Some(buf) = &app.input {
        format!(" > open: {buf}█")
    } else {
        format!(" > {}{}", app.status, link_info)
    };

    let help = " j/k scroll · tab/n link · enter open · [ ] history · o/: url · r reload · q ";
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(prompt_line, th.status_bar())),
            Line::from(Span::styled(help, Style::new().bg(th.bg_panel).fg(th.accent_dim))),
        ]),
        chunks[2],
    );
}

fn truncate(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}
