//! Grok-density session TUI + Safari-style start page (Favorites + Reading List).

use crate::home::{Bookmark, HomeData, ReadingItem};
use crate::layout::{self, Layout as DocLayout, Segment, Style as LayStyle};
use crate::model::{Document, Ref};
use crate::session::Session;
use crate::theme::Theme;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Browse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HomeSection {
    Favorites,
    ReadingList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Search,
    Content,
    OpenUrl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditField {
    Title,
    Url,
}

#[derive(Debug, Clone)]
enum EditKind {
    AddFavorite,
    EditFavorite(usize),
    AddReading,
    EditReading(usize),
}

#[derive(Debug, Clone)]
struct EditState {
    kind: EditKind,
    title: String,
    url: String,
    field: EditField,
}

pub struct App {
    screen: Screen,
    home: HomeData,
    home_section: HomeSection,
    fav_idx: usize,
    read_idx: usize,
    edit: Option<EditState>,

    session: Session,
    theme: Theme,
    layout: DocLayout,
    scroll: u16,
    selected_link: usize,
    status: String,
    width: u16,
    focus: Focus,
    search_buf: String,
    open_buf: String,
}

impl App {
    pub fn home() -> Self {
        let home = HomeData::load();
        Self {
            screen: Screen::Home,
            home,
            home_section: HomeSection::Favorites,
            fav_idx: 0,
            read_idx: 0,
            edit: None,
            session: Session::new(),
            theme: Theme::groknight(),
            layout: DocLayout {
                width: 0,
                lines: vec![],
                link_order: vec![],
            },
            scroll: 0,
            selected_link: 0,
            status: format!(
                "Start Page · {} favorites · {} reading · a add · e edit · d del · enter open",
                0, 0
            ),
            width: 80,
            focus: Focus::Content,
            search_buf: String::new(),
            open_buf: String::new(),
        }
        .with_home_status()
    }

    pub async fn open_url(url: &str) -> Result<Self> {
        let mut app = Self::home();
        app.go(url).await?;
        Ok(app)
    }

    fn with_home_status(mut self) -> Self {
        self.refresh_home_status();
        self
    }

    fn refresh_home_status(&mut self) {
        self.status = format!(
            "Start Page · {} favorites · {} reading · tab section · a add · e edit · d del · enter open · o url",
            self.home.favorites.len(),
            self.home.reading_list.len()
        );
    }

    fn persist_home(&mut self) {
        if let Err(e) = self.home.save() {
            self.status = format!("save failed: {e:#}");
        } else {
            self.refresh_home_status();
        }
    }

    fn go_home(&mut self) {
        self.screen = Screen::Home;
        self.edit = None;
        self.refresh_home_status();
    }

    fn doc(&self) -> Option<&Document> {
        self.session.current().map(|p| &p.doc)
    }

    fn after_nav(&mut self) {
        self.screen = Screen::Browse;
        self.scroll = 0;
        self.selected_link = 0;
        self.search_buf.clear();
        if let Some(doc) = self.doc() {
            if doc.is_search_home() || doc.primary_search().is_some() {
                self.focus = Focus::Search;
            } else {
                self.focus = Focus::Content;
            }
        }
        self.relayout(self.width);
    }

    fn relayout(&mut self, width: u16) {
        self.width = width;
        if self.screen != Screen::Browse {
            return;
        }
        let Some(doc) = self.doc() else {
            return;
        };
        let content_w = width.saturating_sub(4).max(20);
        self.layout = layout::layout_document(doc, content_w);
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
        let Some(doc) = self.doc() else {
            return Ok(());
        };
        let Some(r) = self.selected_ref() else {
            self.status = "no link selected — tab to pick".into();
            return Ok(());
        };
        let href = match doc.resolve_link(r) {
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
        let Some(doc) = self.doc() else {
            return Ok(());
        };
        let Some(url) = doc.search_url(&q) else {
            self.status = "no search form on this page".into();
            return Ok(());
        };
        self.status = "searching…".into();
        let page = self.session.open(&url).await?;
        self.status = status_for(page);
        self.after_nav();
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
            self.go_home();
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

    fn fav_cols(&self, width: u16) -> usize {
        let usable = width.saturating_sub(4) as usize;
        let tile = 16usize;
        (usable / tile).clamp(2, 6)
    }

    fn open_selected_home(&mut self) -> Option<String> {
        match self.home_section {
            HomeSection::Favorites => self
                .home
                .favorites
                .get(self.fav_idx)
                .map(|b| b.url.clone()),
            HomeSection::ReadingList => self
                .home
                .reading_list
                .get(self.read_idx)
                .map(|r| r.url.clone()),
        }
    }

    fn start_add(&mut self) {
        let kind = match self.home_section {
            HomeSection::Favorites => EditKind::AddFavorite,
            HomeSection::ReadingList => EditKind::AddReading,
        };
        self.edit = Some(EditState {
            kind,
            title: String::new(),
            url: String::new(),
            field: EditField::Title,
        });
        self.status = "add · tab field · enter save · esc cancel".into();
    }

    fn start_edit(&mut self) {
        match self.home_section {
            HomeSection::Favorites => {
                if let Some(f) = self.home.favorites.get(self.fav_idx) {
                    self.edit = Some(EditState {
                        kind: EditKind::EditFavorite(self.fav_idx),
                        title: f.title.clone(),
                        url: f.url.clone(),
                        field: EditField::Title,
                    });
                }
            }
            HomeSection::ReadingList => {
                if let Some(r) = self.home.reading_list.get(self.read_idx) {
                    self.edit = Some(EditState {
                        kind: EditKind::EditReading(self.read_idx),
                        title: r.title.clone(),
                        url: r.url.clone(),
                        field: EditField::Title,
                    });
                }
            }
        }
        if self.edit.is_some() {
            self.status = "edit · tab field · enter save · esc cancel".into();
        }
    }

    fn delete_selected(&mut self) {
        match self.home_section {
            HomeSection::Favorites => {
                if self.home.favorites.is_empty() {
                    return;
                }
                self.home.remove_favorite(self.fav_idx);
                if self.fav_idx > 0 && self.fav_idx >= self.home.favorites.len() {
                    self.fav_idx = self.home.favorites.len().saturating_sub(1);
                }
            }
            HomeSection::ReadingList => {
                if self.home.reading_list.is_empty() {
                    return;
                }
                self.home.remove_reading(self.read_idx);
                if self.read_idx > 0 && self.read_idx >= self.home.reading_list.len() {
                    self.read_idx = self.home.reading_list.len().saturating_sub(1);
                }
            }
        }
        self.persist_home();
        self.status = "deleted".into();
    }

    fn commit_edit(&mut self) {
        let Some(ed) = self.edit.take() else {
            return;
        };
        let title = ed.title.trim().to_string();
        let url = ed.url.trim().to_string();
        if title.is_empty() || url.is_empty() {
            self.status = "title and url required".into();
            self.edit = Some(ed);
            return;
        }
        let url = if url.starts_with("http://") || url.starts_with("https://") {
            url
        } else {
            format!("https://{url}")
        };
        match ed.kind {
            EditKind::AddFavorite => self.home.add_favorite(title, url),
            EditKind::EditFavorite(i) => self.home.update_favorite(i, title, url),
            EditKind::AddReading => self.home.add_reading(title, url),
            EditKind::EditReading(i) => {
                if let Some(item) = self.home.reading_list.get_mut(i) {
                    item.title = title;
                    item.url = url;
                }
            }
        }
        self.persist_home();
        self.status = "saved".into();
    }

    fn add_current_to_favorites(&mut self) {
        let Some(doc) = self.doc() else {
            self.status = "nothing to bookmark".into();
            return;
        };
        let title = if doc.title.is_empty() {
            doc.url.clone()
        } else {
            doc.title.clone()
        };
        self.home.add_favorite(title, doc.url.clone());
        self.persist_home();
        self.status = "added to Favorites".into();
    }

    fn add_current_to_reading(&mut self) {
        let Some(doc) = self.doc() else {
            self.status = "nothing to save".into();
            return;
        };
        let title = if doc.title.is_empty() {
            doc.url.clone()
        } else {
            doc.title.clone()
        };
        self.home.add_reading(title, doc.url.clone());
        self.persist_home();
        self.status = "saved to Reading List".into();
    }
}

fn status_for(page: &crate::session::LoadedPage) -> String {
    let search = if page.doc.primary_search().is_some() {
        " · / search"
    } else {
        ""
    };
    format!(
        "custom · {}ms · {} links{search} · H home · f favorite · s reading list · q quit",
        page.total_ms,
        page.doc.links.len()
    )
}

pub async fn run_home() -> Result<()> {
    let mut terminal = ratatui::init();
    let size = terminal.size().unwrap_or(Size {
        width: 100,
        height: 30,
    });
    let mut app = App::home();
    app.width = size.width;
    app.refresh_home_status();
    let result = event_loop(&mut terminal, &mut app).await;
    ratatui::restore();
    result
}

pub async fn run(url: &str) -> Result<()> {
    let mut terminal = ratatui::init();
    let size = terminal.size().unwrap_or(Size {
        width: 100,
        height: 30,
    });
    let mut app = match App::open_url(url).await {
        Ok(mut a) => {
            a.width = size.width;
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
        let width = terminal.size()?.width;
        app.width = width;

        // ── Edit modal ──
        if let Some(ed) = app.edit.as_mut() {
            match key.code {
                KeyCode::Esc => {
                    app.edit = None;
                    app.refresh_home_status();
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    ed.field = match ed.field {
                        EditField::Title => EditField::Url,
                        EditField::Url => EditField::Title,
                    };
                }
                KeyCode::Enter => {
                    app.commit_edit();
                }
                KeyCode::Backspace => match ed.field {
                    EditField::Title => {
                        ed.title.pop();
                    }
                    EditField::Url => {
                        ed.url.pop();
                    }
                },
                KeyCode::Char(c) if !c.is_control() => match ed.field {
                    EditField::Title => ed.title.push(c),
                    EditField::Url => ed.url.push(c),
                },
                _ => {}
            }
            continue;
        }

        // ── Home screen ──
        if app.screen == Screen::Home {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Tab => {
                    app.home_section = match app.home_section {
                        HomeSection::Favorites => HomeSection::ReadingList,
                        HomeSection::ReadingList => HomeSection::Favorites,
                    };
                    app.refresh_home_status();
                }
                KeyCode::Left | KeyCode::Char('h') if app.home_section == HomeSection::Favorites => {
                    if app.fav_idx > 0 {
                        app.fav_idx -= 1;
                    }
                }
                KeyCode::Right | KeyCode::Char('l')
                    if app.home_section == HomeSection::Favorites =>
                {
                    if app.fav_idx + 1 < app.home.favorites.len() {
                        app.fav_idx += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => match app.home_section {
                    HomeSection::Favorites => {
                        let cols = app.fav_cols(width);
                        app.fav_idx = app.fav_idx.saturating_sub(cols);
                    }
                    HomeSection::ReadingList => {
                        app.read_idx = app.read_idx.saturating_sub(1);
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => match app.home_section {
                    HomeSection::Favorites => {
                        let cols = app.fav_cols(width);
                        if app.fav_idx + cols < app.home.favorites.len() {
                            app.fav_idx += cols;
                        } else if !app.home.favorites.is_empty() {
                            app.fav_idx = app.home.favorites.len() - 1;
                        }
                    }
                    HomeSection::ReadingList => {
                        if app.read_idx + 1 < app.home.reading_list.len() {
                            app.read_idx += 1;
                        }
                    }
                },
                KeyCode::Enter => {
                    if let Some(url) = app.open_selected_home() {
                        if let Err(e) = app.go(&url).await {
                            app.status = format!("error: {e:#}");
                        }
                    }
                }
                KeyCode::Char('a') => app.start_add(),
                KeyCode::Char('e') => app.start_edit(),
                KeyCode::Char('d') | KeyCode::Backspace | KeyCode::Delete => {
                    app.delete_selected();
                }
                KeyCode::Char('o') | KeyCode::Char(':') => {
                    app.focus = Focus::OpenUrl;
                    app.open_buf.clear();
                    app.screen = Screen::Browse; // reuse open-url draw path via status
                    // Stay on home visually — handle open url on home
                    app.screen = Screen::Home;
                    app.edit = Some(EditState {
                        kind: EditKind::AddFavorite,
                        title: "New".into(),
                        url: String::new(),
                        field: EditField::Url,
                    });
                    app.status = "open/add · type url · enter".into();
                }
                KeyCode::Char('/') => {
                    // Jump to DDG search home
                    if let Err(e) = app.go("https://html.duckduckgo.com/html/").await {
                        app.status = format!("error: {e:#}");
                    }
                }
                _ => {}
            }
            continue;
        }

        // ── Browse screen ──
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
                    KeyCode::Char(c) if !c.is_control() => app.open_buf.push(c),
                    _ => {}
                }
                continue;
            }
            Focus::Search => {
                match key.code {
                    KeyCode::Esc => {
                        if app.doc().map(|d| d.is_search_home()).unwrap_or(false) {
                            // stay
                        } else {
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
                    KeyCode::Tab => app.focus = Focus::Content,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('H') => app.go_home(),
                    KeyCode::Char('[') => app.back(),
                    KeyCode::Char(']') => app.forward(),
                    KeyCode::Char(c) if !c.is_control() => app.search_buf.push(c),
                    _ => {}
                }
                continue;
            }
            Focus::Content => {}
        }

        match key.code {
            KeyCode::Char('q') => break,
            KeyCode::Esc => {
                app.go_home();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
            KeyCode::Char('H') => app.go_home(),
            KeyCode::Char('f') => app.add_current_to_favorites(),
            KeyCode::Char('s') => app.add_current_to_reading(),
            KeyCode::Char('/') | KeyCode::Char('i') => {
                if app.doc().and_then(|d| d.primary_search()).is_some() {
                    app.focus = Focus::Search;
                    app.status = "search — type query, enter to go".into();
                } else {
                    app.status = "no search form — H home or o url".into();
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

        if width != app.layout.width && app.screen == Screen::Browse {
            app.relayout(width);
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

    // Title
    let title = match app.screen {
        Screen::Home => Line::from(vec![
            Span::styled(" termbrowse ", th.title_accent()),
            Span::styled(" Start Page ", th.title_bar().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!(
                    " {} favorites · {} reading ",
                    app.home.favorites.len(),
                    app.home.reading_list.len()
                ),
                Style::new().bg(th.bg_panel).fg(th.text_dim),
            ),
        ]),
        Screen::Browse => {
            let page = app.session.current();
            let (t, u) = page
                .map(|p| (p.doc.title.as_str(), p.doc.url.as_str()))
                .unwrap_or(("…", ""));
            let back = if app.session.can_back() { "◀" } else { " " };
            let fwd = if app.session.can_forward() { "▶" } else { " " };
            Line::from(vec![
                Span::styled(" termbrowse ", th.title_accent()),
                Span::styled(format!(" {back}{fwd} "), th.title_bar()),
                Span::styled(
                    format!("{} ", truncate(t, 36)),
                    th.title_bar().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {} ", truncate(u, 40)),
                    Style::new().bg(th.bg_panel).fg(th.text_dim),
                ),
            ])
        }
    };
    frame.render_widget(Paragraph::new(title).style(th.title_bar()), chunks[0]);

    let body = chunks[1];
    frame.render_widget(Block::default().style(th.body_bg()), body);

    match app.screen {
        Screen::Home => draw_home(frame, app, body),
        Screen::Browse => draw_browse(frame, app, body),
    }

    // Edit overlay
    if let Some(ed) = &app.edit {
        draw_edit_modal(frame, app, body, ed);
    }

    // Status
    let prompt_line = if app.edit.is_some() {
        app.status.clone()
    } else if app.screen == Screen::Browse && app.focus == Focus::OpenUrl {
        format!(" > open: {}█", app.open_buf)
    } else if app.screen == Screen::Browse && app.focus == Focus::Search {
        let ph = app
            .doc()
            .and_then(|d| d.primary_search())
            .map(|f| f.placeholder.as_str())
            .unwrap_or("Search…");
        if app.search_buf.is_empty() {
            format!(" > search · {ph}")
        } else {
            format!(" > search: {}█", app.search_buf)
        }
    } else {
        format!(" > {}", app.status)
    };

    let help = match (app.screen, app.edit.is_some()) {
        (_, true) => " tab field · enter save · esc cancel ",
        (Screen::Home, _) => {
            " arrows move · tab section · enter open · a add · e edit · d delete · / search · q "
        }
        (Screen::Browse, _) => {
            " j/k scroll · tab links · f favorite · s reading · H home · o url · q "
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

/// Safari-like start page: Favorites grid + Reading List.
fn draw_home(frame: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let mut y = area.y + 1;

    // Section: Favorites
    let fav_active = app.home_section == HomeSection::Favorites;
    let fav_title = if fav_active {
        "◆ Favorites"
    } else {
        "  Favorites"
    };
    frame.render_widget(
        Paragraph::new(fav_title).style(
            Style::new()
                .fg(if fav_active { th.accent } else { th.text_dim })
                .bg(th.bg)
                .add_modifier(Modifier::BOLD),
        ),
        Rect::new(area.x + 2, y, area.width.saturating_sub(4), 1),
    );
    y += 2;

    let cols = app.fav_cols(area.width) as u16;
    let tile_w = ((area.width.saturating_sub(4)) / cols.max(1)).max(12);
    let tile_h: u16 = 4;
    let favs = &app.home.favorites;

    if favs.is_empty() {
        frame.render_widget(
            Paragraph::new("  No favorites yet — press a to add").style(th.dim()),
            Rect::new(area.x + 2, y, area.width.saturating_sub(4), 1),
        );
        y += 2;
    } else {
        for (i, fav) in favs.iter().enumerate() {
            let row = (i as u16) / cols;
            let col = (i as u16) % cols;
            let x = area.x + 2 + col * tile_w;
            let ty = y + row * (tile_h + 1);
            if ty + tile_h > area.y + area.height.saturating_sub(8) {
                break;
            }
            let selected = fav_active && i == app.fav_idx;
            draw_fav_tile(frame, th, fav, Rect::new(x, ty, tile_w.saturating_sub(1), tile_h), selected);
        }
        let rows = (favs.len() as u16).div_ceil(cols);
        y += rows * (tile_h + 1) + 1;
    }

    // Section: Reading List
    if y + 4 < area.y + area.height {
        let read_active = app.home_section == HomeSection::ReadingList;
        let read_title = if read_active {
            "◆ Reading List"
        } else {
            "  Reading List"
        };
        frame.render_widget(
            Paragraph::new(read_title).style(
                Style::new()
                    .fg(if read_active { th.accent } else { th.text_dim })
                    .bg(th.bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Rect::new(area.x + 2, y, area.width.saturating_sub(4), 1),
        );
        y += 2;

        if app.home.reading_list.is_empty() {
            frame.render_widget(
                Paragraph::new("  Empty — open a page and press s to save").style(th.dim()),
                Rect::new(area.x + 2, y, area.width.saturating_sub(4), 1),
            );
        } else {
            let max_items = (area.y + area.height).saturating_sub(y + 1) as usize;
            for (i, item) in app.home.reading_list.iter().enumerate().take(max_items) {
                let selected = read_active && i == app.read_idx;
                draw_reading_row(
                    frame,
                    th,
                    item,
                    Rect::new(area.x + 2, y, area.width.saturating_sub(4), 1),
                    selected,
                );
                y += 1;
            }
        }
    }
}

fn draw_fav_tile(frame: &mut Frame, th: &Theme, fav: &Bookmark, area: Rect, selected: bool) {
    let border = if selected { th.accent } else { th.border };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(border).bg(th.bg_panel))
        .style(Style::new().bg(if selected { th.bg_panel } else { th.bg }));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Icon letter
    let letter = fav
        .title
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let letter_style = Style::new()
        .fg(th.accent)
        .bg(if selected { th.bg_panel } else { th.bg })
        .add_modifier(Modifier::BOLD);
    frame.render_widget(
        Paragraph::new(format!(" {letter} ")).style(letter_style),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let title = truncate(&fav.title, inner.width.saturating_sub(1) as usize);
    frame.render_widget(
        Paragraph::new(title).style(Style::new().fg(th.text).bg(if selected {
            th.bg_panel
        } else {
            th.bg
        })),
        Rect::new(inner.x, inner.y.saturating_add(1), inner.width, 1),
    );
}

fn draw_reading_row(frame: &mut Frame, th: &Theme, item: &ReadingItem, area: Rect, selected: bool) {
    let style = if selected {
        Style::new()
            .fg(th.link_active)
            .bg(th.bg_panel)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(th.text).bg(th.bg)
    };
    let line = format!("  ◆  {}  ·  {}", truncate(&item.title, 40), truncate(&item.url, 36));
    frame.render_widget(Paragraph::new(line).style(style), area);
}

fn draw_edit_modal(frame: &mut Frame, app: &App, area: Rect, ed: &EditState) {
    let th = &app.theme;
    let w = (area.width as usize * 70 / 100).clamp(40, 64) as u16;
    let h: u16 = 9;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let modal = Rect::new(x, y, w, h);
    frame.render_widget(Clear, modal);

    let title = match ed.kind {
        EditKind::AddFavorite => " Add Favorite ",
        EditKind::EditFavorite(_) => " Edit Favorite ",
        EditKind::AddReading => " Add to Reading List ",
        EditKind::EditReading(_) => " Edit Reading List ",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(th.accent))
        .style(Style::new().bg(th.bg_panel))
        .title(Span::styled(
            title,
            Style::new().fg(th.accent).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let t_focus = ed.field == EditField::Title;
    let u_focus = ed.field == EditField::Url;
    let t_line = format!(
        " Title  {}{}",
        ed.title,
        if t_focus { "█" } else { "" }
    );
    let u_line = format!(
        " URL    {}{}",
        ed.url,
        if u_focus { "█" } else { "" }
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                t_line,
                if t_focus {
                    Style::new().fg(th.link_active).bg(th.bg_panel)
                } else {
                    Style::new().fg(th.text).bg(th.bg_panel)
                },
            )),
            Line::from(""),
            Line::from(Span::styled(
                u_line,
                if u_focus {
                    Style::new().fg(th.link_active).bg(th.bg_panel)
                } else {
                    Style::new().fg(th.text).bg(th.bg_panel)
                },
            )),
            Line::from(""),
            Line::from(Span::styled(
                " tab switch field · enter save · esc cancel",
                Style::new().fg(th.text_dim).bg(th.bg_panel),
            )),
        ]),
        inner,
    );
}

fn draw_browse(frame: &mut Frame, app: &App, body: Rect) {
    let page = match app.session.current() {
        Some(p) => p,
        None => return,
    };

    let show_centered = app.focus == Focus::Search
        && page.doc.primary_search().is_some()
        && (page.doc.wants_centered_search()
            || page.doc.is_search_home()
            || app.search_buf.is_empty() && page.doc.links.len() < 15);

    if show_centered {
        draw_centered_search(frame, app, body);
    } else {
        draw_content(frame, app, body);
        if app.focus == Focus::Search && page.doc.primary_search().is_some() {
            draw_top_search_bar(frame, app, body);
        }
    }
}

fn draw_centered_search(frame: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let doc = match app.doc() {
        Some(d) => d,
        None => return,
    };
    let form = doc.primary_search();
    let placeholder = form
        .map(|f| f.placeholder.as_str())
        .unwrap_or("Search…");
    let brand = brand_label(doc);

    frame.render_widget(Block::default().style(Style::new().bg(th.bg)), area);

    let box_w = (area.width as usize * 62 / 100).clamp(40, 78) as u16;
    let box_h: u16 = 5;
    let total_h = 2 + 1 + box_h + 2 + 2;
    let top = area.y + area.height.saturating_sub(total_h) / 2;
    let left = area.x + area.width.saturating_sub(box_w) / 2;

    let brand_line = format!("◆  {brand}");
    let brand_w = brand_line.chars().count() as u16;
    let brand_x = area.x + area.width.saturating_sub(brand_w) / 2;
    frame.render_widget(
        Paragraph::new(brand_line).style(
            Style::new()
                .fg(th.accent)
                .bg(th.bg)
                .add_modifier(Modifier::BOLD),
        ),
        Rect::new(brand_x, top, brand_w.min(area.width), 1),
    );

    let box_area = Rect::new(left, top + 3, box_w.min(area.width), box_h);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(th.accent).bg(th.bg_panel))
        .style(Style::new().bg(th.bg_panel))
        .title(Span::styled(
            " ▎ search ",
            Style::new()
                .fg(th.accent)
                .bg(th.bg_panel)
                .add_modifier(Modifier::BOLD),
        ));
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
        Style::new()
            .fg(th.text)
            .bg(th.bg_panel)
            .add_modifier(Modifier::BOLD)
    };
    let inner_mid = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1) / 2,
        inner.width,
        1,
    );
    frame.render_widget(Paragraph::new(display).style(input_style), inner_mid);

    let hint1 = "enter to search";
    let hint2 = "tab · page content    H · home    q · quit";
    for (i, hint) in [hint1, hint2].iter().enumerate() {
        let hint_w = hint.len() as u16;
        let hint_x = area.x + area.width.saturating_sub(hint_w) / 2;
        frame.render_widget(
            Paragraph::new(*hint).style(Style::new().fg(th.text_dim).bg(th.bg)),
            Rect::new(
                hint_x,
                top + 3 + box_h + 1 + i as u16,
                hint_w.min(area.width),
                1,
            ),
        );
    }
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
                        LayStyle::Heading1 => th.heading(1),
                        LayStyle::Heading2 => th.heading(2),
                        LayStyle::Heading3 => th.heading(3),
                        LayStyle::Dim => th.dim(),
                        LayStyle::Quote => th.quote(),
                        LayStyle::Pre | LayStyle::Code => th.code(),
                        LayStyle::Strong => th.strong(),
                        LayStyle::Em => th.em(),
                        LayStyle::Border => th.border(),
                        LayStyle::Image => th.image(),
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
    if doc.looks_like_captcha() || doc.title.contains("CAPTCHA") {
        return "Search blocked".into();
    }
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
