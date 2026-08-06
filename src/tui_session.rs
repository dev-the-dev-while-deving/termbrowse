//! Grok-density session TUI — structure blocks, accent rails, centered search.
//! Search homes (Google, …) show a middle prompt box like Grok Build's input.

use crate::layout::{self, Layout as DocLayout, Segment, Style as LayStyle};
use crate::model::{Document, Ref};
use crate::session::{LoadSource, Session};
use crate::theme::Theme;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// Centered page search (Google-style).
    Search,
    /// Scroll content / links.
    Content,
    /// `:` open URL bar at bottom.
    OpenUrl,
}

pub struct App {
    session: Session,
    theme: Theme,
    layout: DocLayout,
    scroll: u16,
    selected_link: usize,
    status: String,
    width: u16,
    focus: Focus,
    /// Query typed into the page search form.
    search_buf: String,
    /// Bottom open-url buffer when Focus::OpenUrl.
    open_buf: String,
}

impl App {
    pub async fn open(url: &str, escalate: bool) -> Result<Self> {
        let mut session = Session::new(escalate);
        let page = session.open(url).await?;
        let theme = Theme::groknight();
        let status = status_for(page);
        let focus = if page.doc.is_search_home() || page.doc.primary_search().is_some() {
            // Prefer search when a form exists; center layout when search-home.
            Focus::Search
        } else {
            Focus::Content
        };
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
            focus,
            search_buf: String::new(),
            open_buf: String::new(),
        };
        app.relayout(80);
        Ok(app)
    }

    fn doc(&self) -> &Document {
        &self.session.current().expect("session has page").doc
    }

    fn after_nav(&mut self) {
        self.scroll = 0;
        self.selected_link = 0;
        self.search_buf.clear();
        if self.doc().is_search_home() || self.doc().primary_search().is_some() {
            self.focus = Focus::Search;
        } else {
            self.focus = Focus::Content;
        }
        self.relayout(self.width);
    }

    fn relayout(&mut self, width: u16) {
        self.width = width;
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
        self.after_nav();
        Ok(())
    }

    async fn go(&mut self, url: &str) -> Result<()> {
        self.status = format!("loading {url} …");
        let page = self.session.open(url).await?;
        self.status = status_for(page);
        self.after_nav();
        Ok(())
    }

    async fn submit_search(&mut self) -> Result<()> {
        let q = self.search_buf.trim().to_string();
        if q.is_empty() {
            self.status = "type a query first".into();
            return Ok(());
        }
        let Some(url) = self.doc().search_url(&q) else {
            self.status = "no search form on this page".into();
            return Ok(());
        };
        self.status = format!("searching…");
        let page = self.session.open(&url).await?;
        self.status = status_for(page);
        self.after_nav();
        // After results: keep search buffer with query, focus content to browse hits.
        self.search_buf = q;
        self.focus = Focus::Content;
        Ok(())
    }

    async fn reload(&mut self) -> Result<()> {
        self.status = "reloading…".into();
        let page = self.session.reload().await?;
        self.status = status_for(page);
        self.after_nav();
        Ok(())
    }

    fn back(&mut self) {
        if let Some(page) = self.session.back() {
            self.status = status_for(page);
            self.after_nav();
        } else {
            self.status = "no history back".into();
        }
    }

    fn forward(&mut self) {
        if let Some(page) = self.session.forward() {
            self.status = status_for(page);
            self.after_nav();
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
    let search = if page.doc.primary_search().is_some() {
        " · / search"
    } else {
        ""
    };
    format!(
        "{src} · {}ms · {} links{search} · [ ] history · tab links · q quit",
        page.total_ms,
        page.doc.links.len()
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

        match app.focus {
            Focus::OpenUrl => {
                match key.code {
                    KeyCode::Esc => {
                        app.focus = Focus::Content;
                        app.open_buf.clear();
                        app.status = "cancelled".into();
                    }
                    KeyCode::Enter => {
                        let url = app.open_buf.trim().to_string();
                        app.open_buf.clear();
                        app.focus = Focus::Content;
                        if !url.is_empty() {
                            if let Err(e) = app.go(&url).await {
                                app.status = format!("error: {e:#}");
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        app.open_buf.pop();
                    }
                    KeyCode::Char(c) => app.open_buf.push(c),
                    _ => {}
                }
                continue;
            }
            Focus::Search => {
                match key.code {
                    KeyCode::Esc => {
                        if !app.doc().is_search_home() {
                            app.focus = Focus::Content;
                        }
                    }
                    KeyCode::Enter => {
                        if let Err(e) = app.submit_search().await {
                            app.status = format!("search error: {e:#}");
                        }
                    }
                    KeyCode::Backspace => {
                        app.search_buf.pop();
                    }
                    KeyCode::Tab => {
                        app.focus = Focus::Content;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('[') => app.back(),
                    KeyCode::Char(']') => app.forward(),
                    KeyCode::Char(c) if !c.is_control() => {
                        app.search_buf.push(c);
                    }
                    _ => {}
                }
                continue;
            }
            Focus::Content => {}
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
            KeyCode::Char('/') | KeyCode::Char('i') => {
                if app.doc().primary_search().is_some() {
                    app.focus = Focus::Search;
                    app.status = "search — type query, enter to go".into();
                } else {
                    app.status = "no search form on this page".into();
                }
            }
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
            KeyCode::Char('h') => app.back(),
            KeyCode::Char('[') => app.back(),
            KeyCode::Char(']') => app.forward(),
            KeyCode::Char(':') | KeyCode::Char('o') => {
                app.focus = Focus::OpenUrl;
                app.open_buf.clear();
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
        let has = line
            .segments
            .iter()
            .any(|s| matches!(s, Segment::Link { r#ref, .. } if *r#ref == r));
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
        Span::styled(
            format!(" {src} "),
            Style::new().bg(th.bg_panel).fg(th.success),
        ),
    ]);
    frame.render_widget(Paragraph::new(title).style(th.title_bar()), chunks[0]);

    let body = chunks[1];
    // Fill body background
    frame.render_widget(Block::default().style(th.body_bg()), body);

    let search_home = page.doc.is_search_home()
        || (app.focus == Focus::Search && page.doc.primary_search().is_some());

    if search_home && app.focus == Focus::Search {
        draw_centered_search(frame, app, body);
    } else {
        draw_content(frame, app, body);
        // Compact search strip at top of body when form exists and focused
        if app.focus == Focus::Search && page.doc.primary_search().is_some() {
            draw_top_search_bar(frame, app, body);
        }
    }

    // Status
    let link_info = match app.selected_ref().and_then(|r| app.doc().resolve_link(r)) {
        Some(l) => format!(
            " ◆ [{}] {} → {}",
            l.r#ref,
            truncate(&l.text, 28),
            truncate(&l.href, 36)
        ),
        None => String::new(),
    };

    let prompt_line = match app.focus {
        Focus::OpenUrl => format!(" > open: {}█", app.open_buf),
        Focus::Search => {
            let ph = page
                .doc
                .primary_search()
                .map(|f| f.placeholder.as_str())
                .unwrap_or("Search…");
            if app.search_buf.is_empty() {
                format!(" > search · {ph}")
            } else {
                format!(" > search: {}█", app.search_buf)
            }
        }
        Focus::Content => format!(" > {}{}", app.status, link_info),
    };

    let help = match app.focus {
        Focus::Search => " type query · enter search · tab content · [ ] history · q quit ",
        Focus::OpenUrl => " type url · enter · esc cancel ",
        Focus::Content => {
            " j/k scroll · tab links · enter open · / search · [ ] history · o url · q "
        }
    };

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(prompt_line, th.status_bar())),
            Line::from(Span::styled(
                help,
                Style::new().bg(th.bg_panel).fg(th.accent_dim),
            )),
        ]),
        chunks[2],
    );
}

/// Grok Build–style centered search: title + middle input box + muted hint.
fn draw_centered_search(frame: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let doc = app.doc();
    let form = doc.primary_search();
    let placeholder = form
        .map(|f| f.placeholder.as_str())
        .unwrap_or("Search…");
    let brand = brand_label(doc);

    // Vertical center stack: brand, spacer, box (3 rows), hint
    let box_w = (area.width as usize * 60 / 100).clamp(36, 72) as u16;
    let box_h: u16 = 3;
    let total_h = 1 + 1 + box_h + 1 + 1; // brand, pad, box, pad, hint
    let top = area.y + area.height.saturating_sub(total_h) / 2;
    let left = area.x + area.width.saturating_sub(box_w) / 2;

    // Brand
    let brand_w = brand.chars().count() as u16;
    let brand_x = area.x + area.width.saturating_sub(brand_w) / 2;
    frame.render_widget(
        Paragraph::new(brand).style(
            Style::new()
                .fg(th.accent)
                .bg(th.bg)
                .add_modifier(Modifier::BOLD),
        ),
        Rect::new(brand_x, top, brand_w.min(area.width), 1),
    );

    // Input box (Grok prompt energy)
    let box_area = Rect::new(left, top + 2, box_w.min(area.width), box_h);
    let border_style = Style::new().fg(th.accent).bg(th.bg);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::new().bg(th.bg_panel))
        .title(Span::styled(" search ", Style::new().fg(th.accent).bg(th.bg)));
    let inner = block.inner(box_area);
    frame.render_widget(block, box_area);

    let display = if app.search_buf.is_empty() {
        format!("  {placeholder}")
    } else {
        format!("  {}█", app.search_buf)
    };
    let input_style = if app.search_buf.is_empty() {
        Style::new().fg(th.text_dim).bg(th.bg_panel)
    } else {
        Style::new().fg(th.text).bg(th.bg_panel)
    };
    frame.render_widget(Paragraph::new(display).style(input_style), inner);

    // Hint under box
    let hint = "enter to search  ·  tab for page content";
    let hint_w = hint.len() as u16;
    let hint_x = area.x + area.width.saturating_sub(hint_w) / 2;
    frame.render_widget(
        Paragraph::new(hint).style(Style::new().fg(th.text_dim).bg(th.bg)),
        Rect::new(hint_x, top + 2 + box_h + 1, hint_w.min(area.width), 1),
    );
}

fn draw_top_search_bar(frame: &mut Frame, app: &App, body: Rect) {
    let th = &app.theme;
    let w = (body.width as usize * 70 / 100).clamp(30, 64) as u16;
    let x = body.x + body.width.saturating_sub(w) / 2;
    let y = body.y + 1;
    let area = Rect::new(x, y, w, 3);
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(th.accent))
        .style(Style::new().bg(th.bg_panel))
        .title(" search ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = if app.search_buf.is_empty() {
        "  type query…".into()
    } else {
        format!("  {}█", app.search_buf)
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::new().fg(th.text).bg(th.bg_panel)),
        inner,
    );
}

fn draw_content(frame: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let selected = app.selected_ref();
    let view_h = area.height as usize;
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
                        LayStyle::Heading1 | LayStyle::Heading2 | LayStyle::Heading3 => th.heading(
                            match style {
                                LayStyle::Heading1 => 1,
                                LayStyle::Heading2 => 2,
                                _ => 3,
                            },
                        ),
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
        area,
    );
}

fn brand_label(doc: &Document) -> String {
    let host = url::Url::parse(&doc.url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
        .unwrap_or_else(|| doc.title.clone());
    if host.contains("google") {
        "Google".into()
    } else if host.contains("duckduckgo") {
        "DuckDuckGo".into()
    } else if host.contains("bing") {
        "Bing".into()
    } else if host.contains("youtube") {
        "YouTube".into()
    } else if !doc.title.is_empty() && doc.title.len() < 40 {
        doc.title.clone()
    } else {
        host
    }
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
