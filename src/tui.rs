//! Terminal browser chrome: tabs, back/forward, omnibox, SERP cards.

use crate::art::{chrome_bg, panel_bg, style_chrome_text, style_panel};
use crate::keys::{self, HintTarget};
use crate::history::History;
use crate::home::HomeData;
use crate::layout::{self, Layout as DocLayout, Segment, Style as LayStyle};
use crate::model::{Document, Ref};
use crate::serp::SearchHit;
use crate::session::Session;
use crate::theme::Theme;
use crate::update;
use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::execute;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Browse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Content,
    Omnibox,
    SiteSearch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Overlay {
    None,
    Help,
    History,
    Find,
    Hints { new_tab: bool, typed: String },
}

#[derive(Debug, Clone, Copy)]
enum Hit {
    Back,
    Forward,
    Reload,
    HomeBtn,
    NewTab,
    Tab(usize),
    CloseTab(usize),
    Omnibox,
    Result(usize),
    Instant,
    HistoryItem(usize),
    PageLink(u32),
    SiteNav(usize),
    SiteSearch,
}

struct Tab {
    screen: Screen,
    session: Session,
    theme: Theme,
    layout: DocLayout,
    scroll: u16,
    selected_link: usize,
    selected_result: usize,
    focus: Focus,
    search_buf: String,
    site_buf: String,
    find_idx: usize,
}

impl Tab {
    fn new() -> Self {
        Self {
            screen: Screen::Home,
            session: Session::new(),
            theme: Theme::groknight(),
            layout: empty_layout(),
            scroll: 0,
            selected_link: 0,
            selected_result: 0,
            focus: Focus::Omnibox,
            search_buf: String::new(),
            site_buf: String::new(),
            find_idx: 0,
        }
    }

    fn title(&self) -> String {
        match self.screen {
            Screen::Home => "Start".into(),
            Screen::Browse => self
                .session
                .current()
                .map(|p| {
                    if p.doc.is_serp() && !p.doc.serp.query.is_empty() {
                        format!("⌕ {}", p.doc.serp.query)
                    } else if p.doc.title.is_empty() {
                        "Untitled".into()
                    } else {
                        p.doc.title.clone()
                    }
                })
                .unwrap_or_else(|| "New Tab".into()),
        }
    }

    fn url(&self) -> String {
        self.session
            .current()
            .map(|p| p.doc.url.clone())
            .unwrap_or_default()
    }

    fn doc(&self) -> Option<&Document> {
        self.session.current().map(|p| &p.doc)
    }
}

struct App {
    tabs: Vec<Tab>,
    tab: usize,
    home: HomeData,
    visits: History,
    hist_idx: usize,
    overlay: Overlay,
    status: String,
    width: u16,
    height: u16,
    omnibox: String,
    find_buf: String,
    hits: Vec<(Rect, Hit)>,
    pending: Option<char>,
    hints: Vec<(String, HintTarget)>,
    update_notice: Arc<Mutex<Option<String>>>,
}

fn empty_layout() -> DocLayout {
    DocLayout {
        width: 0,
        lines: vec![],
        link_order: vec![],
    }
}

impl App {
    fn new() -> Self {
        Self {
            tabs: vec![Tab::new()],
            tab: 0,
            home: HomeData::load(),
            visits: History::load(),
            hist_idx: 0,
            overlay: Overlay::None,
            status: String::new(),
            width: 80,
            height: 24,
            omnibox: String::new(),
            find_buf: String::new(),
            hits: Vec::new(),
            pending: None,
            hints: Vec::new(),
            update_notice: {
                let cache = update::load_cache();
                Arc::new(Mutex::new(update::notice_if_newer(
                    &cache,
                    env!("CARGO_PKG_VERSION"),
                )))
            },
        }
        .with_home_status()
    }

    async fn open_url(url: &str) -> Result<Self> {
        let mut app = Self::new();
        app.go(url).await;
        Ok(app)
    }

    fn t(&self) -> &Tab {
        &self.tabs[self.tab]
    }

    fn tm(&mut self) -> &mut Tab {
        &mut self.tabs[self.tab]
    }

    fn with_home_status(mut self) -> Self {
        self.refresh_home_status();
        self
    }

    fn refresh_home_status(&mut self) {
        if let Ok(g) = self.update_notice.lock() {
            if let Some(msg) = g.as_ref() {
                self.status = msg.clone();
                return;
            }
        }
        self.status = "Search DuckDuckGo · type a query · enter".into();
    }

    fn cancel_modes(&mut self) {
        self.pending = None;
        self.hints.clear();
        if matches!(
            self.overlay,
            Overlay::Hints { .. } | Overlay::Find | Overlay::Help | Overlay::History
        ) {
            self.overlay = Overlay::None;
        }
        if self.t().focus != Focus::Content && self.t().screen != Screen::Home {
            self.tm().focus = Focus::Content;
        }
        self.status = "normal".into();
    }

    fn start_hints(&mut self, new_tab: bool) {
        let mut targets: Vec<HintTarget> = Vec::new();
        if self.t().doc().map(|d| d.is_serp()).unwrap_or(false) {
            if let Some(n) = self.t().doc().map(|d| d.serp.hits.len()) {
                targets.extend((0..n).map(HintTarget::Result));
            }
        } else {
            targets.extend(self.visible_link_ids().into_iter().map(HintTarget::Link));
            if let Some(n) = self.t().doc().map(|d| d.nav.len()) {
                targets.extend((0..n.min(10)).map(HintTarget::Nav));
            }
        }
        if targets.is_empty() {
            self.status = "no visible targets".into();
            return;
        }
        let labels = keys::generate_hints(targets.len());
        self.hints = labels.into_iter().zip(targets).collect();
        self.overlay = Overlay::Hints {
            new_tab,
            typed: String::new(),
        };
        self.status = if new_tab {
            "hints (new tab) · type label · esc cancel".into()
        } else {
            "hints · type label · esc cancel".into()
        };
    }

    fn visible_link_ids(&self) -> Vec<u32> {
        let start = self.t().scroll as usize;
        let end = start.saturating_add(self.height.saturating_sub(6) as usize);
        let mut ids = Vec::new();
        for line in self.t().layout.lines.iter().skip(start).take(end.saturating_sub(start)) {
            for seg in &line.segments {
                if let Segment::Link { r#ref, .. } = seg {
                    if !ids.contains(&r#ref.0) {
                        ids.push(r#ref.0);
                    }
                }
            }
        }
        ids
    }

    async fn apply_hint(&mut self, target: HintTarget, new_tab: bool) {
        self.overlay = Overlay::None;
        self.hints.clear();
        match target {
            HintTarget::Link(id) => {
                if let Some(i) = self.t().layout.link_order.iter().position(|r| r.0 == id) {
                    self.tm().selected_link = i;
                }
                if let Some(href) = self
                    .t()
                    .doc()
                    .and_then(|d| d.resolve_link(Ref(id)))
                    .map(|l| l.href.clone())
                {
                    self.activate_href(&href, new_tab).await;
                }
            }
            HintTarget::Result(i) => {
                if let Some(url) = self
                    .t()
                    .doc()
                    .and_then(|d| d.serp.hits.get(i).map(|h| h.url.clone()))
                {
                    if new_tab {
                        self.open_url_in_new_tab(&url).await;
                    } else {
                        self.go(&url).await;
                    }
                }
            }
            HintTarget::Nav(i) => {
                if let Some(url) = self.t().doc().and_then(|d| d.nav.get(i).map(|n| n.url.clone())) {
                    if new_tab {
                        self.open_url_in_new_tab(&url).await;
                    } else {
                        self.go(&url).await;
                    }
                }
            }
        }
    }

    fn yank_url(&mut self) {
        let url = self.t().url();
        if url.is_empty() {
            self.status = "nothing to yank".into();
            return;
        }
        if copy_to_clipboard(&url) {
            self.status = format!("yanked {url}");
        } else {
            self.status = format!("url: {url}");
        }
    }

    fn persist_home(&mut self) {
        if let Err(e) = self.home.save() {
            self.status = format!("save failed: {e:#}");
        } else {
            self.refresh_home_status();
        }
    }

    fn record_visit(&mut self) {
        if let Some(doc) = self.t().doc() {
            let title = if doc.title.is_empty() {
                doc.url.clone()
            } else {
                doc.title.clone()
            };
            self.visits.push(title, doc.url.clone());
        }
    }

    fn sync_omnibox(&mut self) {
        self.omnibox = self.t().url();
        if self.omnibox.is_empty() {
            self.omnibox.clear();
        }
    }

    fn go_home(&mut self) {
        self.tm().screen = Screen::Home;
        self.tm().theme = Theme::groknight();
        self.tm().focus = Focus::Omnibox;
        self.overlay = Overlay::None;
        self.omnibox.clear();
        self.refresh_home_status();
    }

    fn apply_page_theme(&mut self) {
        let id = self.t().doc().map(|d| d.identity.clone());
        self.tm().theme = match id {
            Some(id) => Theme::groknight().with_identity(&id),
            None => Theme::groknight(),
        };
    }

    fn after_nav(&mut self) {
        self.tm().screen = Screen::Browse;
        self.tm().scroll = 0;
        self.tm().selected_link = 0;
        self.tm().selected_result = 0;
        self.apply_page_theme();
        let search_home = self
            .t()
            .doc()
            .map(|d| d.wants_centered_search())
            .unwrap_or(false);
        self.tm().focus = if search_home {
            Focus::Omnibox
        } else {
            Focus::Content
        };
        if search_home && self.t().search_buf.is_empty() {
            // keep empty for typing a query
        }
        self.sync_omnibox();
        self.relayout(self.width);
        if let Some(doc) = self.t().doc() {
            if doc.is_serp() {
                self.status = format!(
                    "{} results for “{}” · ↑↓ select · enter open · t new tab",
                    doc.serp.hits.len(),
                    doc.serp.query
                );
            }
        }
    }

    fn relayout(&mut self, width: u16) {
        self.width = width;
        if self.t().screen != Screen::Browse {
            return;
        }
        let Some(doc) = self.t().doc() else {
            return;
        };
        let content_w = width.saturating_sub(6).max(20);
        let lay = layout::layout_document(doc, content_w);
        self.tm().layout = lay;
        self.clamp_scroll(0);
    }

    fn clamp_scroll(&mut self, view_h: u16) {
        let max = self
            .t()
            .layout
            .lines
            .len()
            .saturating_sub(view_h as usize) as u16;
        if self.tm().scroll > max {
            self.tm().scroll = max;
        }
    }

    fn selected_ref(&self) -> Option<Ref> {
        self.t().layout.link_order.get(self.t().selected_link).copied()
    }

    fn selected_href(&self) -> Option<String> {
        let doc = self.t().doc()?;
        let r = self.selected_ref()?;
        doc.resolve_link(r).map(|l| l.href.clone())
    }

    fn preview_link(&mut self) {
        if let Some(href) = self.selected_href() {
            self.status = format!("→ {href}  ·  enter open  ·  T new tab");
        }
    }

    fn jump_to_fragment(&mut self, frag: &str) -> bool {
        let Some(doc) = self.t().doc() else {
            return false;
        };
        let needle = doc.blocks.iter().find_map(|b| match b {
            crate::model::Block::Heading { text, id, .. } => {
                if id.as_deref() == Some(frag) || slug(text) == frag {
                    Some(text.clone())
                } else {
                    None
                }
            }
            _ => None,
        });
        let Some(text) = needle else {
            return false;
        };
        for (i, line) in self.t().layout.lines.iter().enumerate() {
            let hay: String = line
                .segments
                .iter()
                .map(|s| match s {
                    Segment::Text { text, .. } | Segment::Link { text, .. } => text.as_str(),
                })
                .collect();
            if hay.contains(&text) {
                self.tm().scroll = i as u16;
                self.status = format!("#{frag}");
                return true;
            }
        }
        false
    }

    async fn activate_href(&mut self, href: &str, new_tab: bool) {
        let base = self.t().url();
        if let Some(frag) = crate::urlutil::same_page_fragment(&base, href) {
            if self.jump_to_fragment(&frag) {
                return;
            }
        }
        if new_tab {
            self.open_url_in_new_tab(href).await;
            return;
        }
        self.status = format!("opening {href} …");
        match self.tm().session.follow_href(href).await {
            Ok(page) => {
                self.status = status_for(page);
                self.after_nav();
                self.record_visit();
            }
            Err(e) => self.status = format!("failed: {e:#}"),
        }
    }

    async fn navigate_selected(&mut self) {
        if self.t().doc().map(|d| d.is_serp()).unwrap_or(false) {
            self.open_selected_result().await;
            return;
        }
        let Some(href) = self.selected_href() else {
            self.status = "no link selected — tab / click a link".into();
            return;
        };
        self.activate_href(&href, false).await;
    }

    async fn open_selected_in_new_tab(&mut self) {
        if self.t().doc().map(|d| d.is_serp()).unwrap_or(false) {
            if let Some(url) = self
                .t()
                .doc()
                .and_then(|d| d.serp.hits.get(self.t().selected_result))
                .map(|h| h.url.clone())
            {
                self.open_url_in_new_tab(&url).await;
            }
            return;
        }
        if let Some(href) = self.selected_href() {
            self.activate_href(&href, true).await;
        }
    }

    async fn open_url_in_new_tab(&mut self, url: &str) {
        if self.tabs.len() >= 12 {
            self.status = "tab limit (12)".into();
            return;
        }
        self.tabs.push(Tab::new());
        self.tab = self.tabs.len() - 1;
        let abs = {
            let prev = self.tabs.len().saturating_sub(2);
            let base = self
                .tabs
                .get(prev)
                .and_then(|t| t.session.current())
                .map(|p| p.doc.url.as_str())
                .unwrap_or("");
            crate::urlutil::resolve_and_unwrap(base, url).unwrap_or_else(|_| url.to_string())
        };
        self.go(&abs).await;
    }

    async fn open_selected_result(&mut self) {
        let url = self
            .t()
            .doc()
            .and_then(|d| d.serp.hits.get(self.t().selected_result))
            .map(|h| h.url.clone());
        let Some(url) = url else {
            self.status = "no result selected".into();
            return;
        };
        self.go(&url).await;
    }

    async fn go(&mut self, url: &str) {
        self.status = format!("loading {url} …");
        match self.tm().session.open(url).await {
            Ok(page) => {
                self.status = status_for(page);
                self.after_nav();
                self.record_visit();
            }
            Err(e) => self.status = format!("failed: {e:#}"),
        }
    }

    fn focus_site_search(&mut self) -> bool {
        if self.t().doc().and_then(|d| d.site_search.as_ref()).is_some() {
            self.tm().focus = Focus::SiteSearch;
            let ph = self
                .t()
                .doc()
                .and_then(|d| d.site_search.as_ref().map(|f| f.placeholder.clone()))
                .unwrap_or_else(|| "Search this site".into());
            self.status = format!("{ph} · enter submit · esc cancel");
            true
        } else {
            false
        }
    }

    async fn submit_site_search(&mut self) {
        let q = self.t().site_buf.trim().to_string();
        let url = self.t().doc().and_then(|d| d.site_search_url(&q));
        let Some(url) = url else {
            self.status = "type a site search query".into();
            return;
        };
        self.tm().focus = Focus::Content;
        self.go(&url).await;
    }

    async fn submit_omnibox(&mut self) {
        let q = self.omnibox.trim().to_string();
        if q.is_empty() {
            let alt = self.t().search_buf.trim().to_string();
            if alt.is_empty() {
                self.status = "type a url or search".into();
                return;
            }
            self.go(&Document::ddg_search_url(&alt)).await;
            return;
        }
        if looks_like_url(&q) {
            self.go(&q).await;
        } else {
            self.go(&Document::ddg_search_url(&q)).await;
        }
        self.tm().focus = Focus::Content;
    }

    async fn reload(&mut self) {
        if self.t().session.current().is_none() {
            return;
        }
        self.status = "reloading…".into();
        match self.tm().session.reload().await {
            Ok(page) => {
                self.status = status_for(page);
                self.after_nav();
            }
            Err(e) => self.status = format!("failed: {e:#}"),
        }
    }

    fn back(&mut self) {
        let moved = self.tm().session.back().is_some();
        if moved {
            let status = self.t().session.current().map(status_for).unwrap_or_default();
            self.status = status;
            self.after_nav();
        } else if self.t().screen == Screen::Browse {
            self.go_home();
        }
    }

    fn forward(&mut self) {
        let moved = self.tm().session.forward().is_some();
        if moved {
            let status = self.t().session.current().map(status_for).unwrap_or_default();
            self.status = status;
            self.after_nav();
        } else {
            self.status = "no forward page".into();
        }
    }

    fn new_tab(&mut self) {
        if self.tabs.len() >= 12 {
            self.status = "tab limit (12)".into();
            return;
        }
        self.tabs.push(Tab::new());
        self.tab = self.tabs.len() - 1;
        self.go_home();
        self.status = "new tab".into();
    }

    fn close_tab(&mut self) {
        if self.tabs.len() == 1 {
            self.go_home();
            self.tm().session = Session::new();
            self.status = "cleared last tab".into();
            return;
        }
        self.tabs.remove(self.tab);
        if self.tab >= self.tabs.len() {
            self.tab = self.tabs.len() - 1;
        }
        self.sync_omnibox();
        self.status = "tab closed".into();
    }

    fn next_tab(&mut self) {
        self.tab = (self.tab + 1) % self.tabs.len();
        self.sync_omnibox();
    }

    fn prev_tab(&mut self) {
        self.tab = if self.tab == 0 {
            self.tabs.len() - 1
        } else {
            self.tab - 1
        };
        self.sync_omnibox();
    }

    fn add_current_to_favorites(&mut self) {
        let Some(doc) = self.t().doc() else {
            self.status = "nothing to bookmark".into();
            return;
        };
        let title = if doc.title.is_empty() {
            doc.url.clone()
        } else {
            doc.title.clone()
        };
        let url = doc.url.clone();
        self.home.add_favorite(title, url);
        self.persist_home();
        self.status = "added to Favorites".into();
    }

    fn add_current_to_reading(&mut self) {
        let Some(doc) = self.t().doc() else {
            self.status = "nothing to save".into();
            return;
        };
        let title = if doc.title.is_empty() {
            doc.url.clone()
        } else {
            doc.title.clone()
        };
        let url = doc.url.clone();
        self.home.add_reading(title, url);
        self.persist_home();
        self.status = "saved to Reading List".into();
    }

    fn find_matches(&self) -> Vec<usize> {
        let q = self.find_buf.to_ascii_lowercase();
        if q.is_empty() {
            return vec![];
        }
        self.t()
            .layout
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                line.segments.iter().any(|s| {
                    let t = match s {
                        Segment::Text { text, .. } | Segment::Link { text, .. } => text,
                    };
                    t.to_ascii_lowercase().contains(&q)
                })
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn find_next(&mut self) {
        let hits = self.find_matches();
        if hits.is_empty() {
            self.status = "no matches".into();
            return;
        }
        let cur = self.t().find_idx;
        let next = hits.iter().copied().find(|&i| i > cur).unwrap_or(hits[0]);
        self.tm().find_idx = next;
        self.tm().scroll = next as u16;
        self.status = format!("find · {} matches", hits.len());
    }

    async fn handle_hit(&mut self, hit: Hit) {
        match hit {
            Hit::Back => self.back(),
            Hit::Forward => self.forward(),
            Hit::Reload => self.reload().await,
            Hit::HomeBtn => self.go_home(),
            Hit::NewTab => self.new_tab(),
            Hit::Tab(i) if i < self.tabs.len() => {
                self.tab = i;
                self.sync_omnibox();
            }
            Hit::CloseTab(i) if i < self.tabs.len() => {
                self.tab = i;
                self.close_tab();
            }
            Hit::Omnibox => {
                self.tm().focus = Focus::Omnibox;
            }
            Hit::Result(i) => {
                if let Some(n) = self.t().doc().map(|d| d.serp.hits.len()) {
                    if i < n {
                        self.tm().selected_result = i;
                        self.open_selected_result().await;
                    }
                }
            }
            Hit::Instant => {
                if let Some(url) = self
                    .t()
                    .doc()
                    .and_then(|d| d.serp.instant.as_ref().map(|a| a.url.clone()))
                {
                    if !url.is_empty() {
                        self.go(&url).await;
                    }
                }
            }
            Hit::HistoryItem(i) => {
                if let Some(u) = self.visits.visits.get(i).map(|v| v.url.clone()) {
                    self.overlay = Overlay::None;
                    self.go(&u).await;
                }
            }
            Hit::PageLink(n) => {
                if let Some(i) = self.t().layout.link_order.iter().position(|r| r.0 == n) {
                    self.tm().selected_link = i;
                }
                if let Some(href) = self
                    .t()
                    .doc()
                    .and_then(|d| d.resolve_link(Ref(n)))
                    .map(|l| l.href.clone())
                {
                    self.activate_href(&href, false).await;
                }
            }
            Hit::SiteNav(i) => {
                if let Some(url) = self.t().doc().and_then(|d| d.nav.get(i).map(|n| n.url.clone())) {
                    self.go(&url).await;
                }
            }
            Hit::SiteSearch => {
                self.focus_site_search();
            }
            _ => {}
        }
    }
}

fn slug(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-')
        .map(|c| if c == ' ' { '-' } else { c.to_ascii_lowercase() })
        .collect()
}

fn hit_at(app: &App, col: u16, row: u16) -> Option<Hit> {
    app.hits.iter().rev().find(|(r, _)| {
        col >= r.x
            && col < r.x.saturating_add(r.width)
            && row >= r.y
            && row < r.y.saturating_add(r.height)
    }).map(|(_, h)| *h)
}

fn copy_to_clipboard(s: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let tries: &[(&str, &[&str])] = &[
        ("pbcopy", &[]),
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
    ];
    for (bin, args) in tries {
        let Ok(mut child) = Command::new(bin).args(*args).stdin(Stdio::piped()).spawn() else {
            continue;
        };
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(s.as_bytes());
        }
        if child.wait().map(|st| st.success()).unwrap_or(false) {
            return true;
        }
    }
    false
}

fn looks_like_url(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("http://")
        || t.starts_with("https://")
        || (t.contains('.') && !t.contains(' ') && t.chars().all(|c| !c.is_whitespace()))
}

fn status_for(page: &crate::session::LoadedPage) -> String {
    format!(
        "{}ms · {} links · ◀▶ history · t tab · ? help",
        page.total_ms,
        page.doc.links.len()
    )
}

pub async fn run_home() -> Result<()> {
    run_app(App::new()).await
}

pub async fn run(url: &str) -> Result<()> {
    run_app(App::open_url(url).await?).await
}

fn kick_update_check(notice: Arc<Mutex<Option<String>>>) {
    let cache = update::load_cache();
    if update::cache_is_fresh(&cache, update::now_secs()) {
        return;
    }
    tokio::spawn(async move {
        if let Ok(next) = update::refresh_latest_cache().await {
            if let Some(msg) = update::notice_if_newer(&next, env!("CARGO_PKG_VERSION")) {
                if let Ok(mut g) = notice.lock() {
                    *g = Some(msg);
                }
            }
        }
    });
}

async fn run_app(mut app: App) -> Result<()> {
    let mut terminal = ratatui::init();
    kick_update_check(Arc::clone(&app.update_notice));
    let _ = execute!(stdout(), EnableMouseCapture);
    if let Ok(size) = terminal.size() {
        app.width = size.width;
        app.height = size.height;
    }
    let result = event_loop(&mut terminal, &mut app).await;
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

async fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;
        if !event::poll(Duration::from_millis(80))? {
            if let Ok(size) = terminal.size() {
                if size.width != app.width {
                    app.height = size.height;
                    app.relayout(size.width);
                }
            }
            if app.t().screen == Screen::Home {
                app.refresh_home_status();
            }
            continue;
        }
        match event::read()? {
            Event::Resize(w, h) => {
                app.height = h;
                app.relayout(w);
            }
            Event::Mouse(m) => {
                let hit = hit_at(app, m.column, m.row);
                if m.kind == MouseEventKind::Down(MouseButton::Left) {
                    if let Some(hit) = hit {
                        app.handle_hit(hit).await;
                    }
                } else if m.kind == MouseEventKind::Down(MouseButton::Middle) {
                    if let Some(hit) = hit {
                        match hit {
                            Hit::PageLink(n) => {
                                if let Some(href) = app
                                    .t()
                                    .doc()
                                    .and_then(|d| d.resolve_link(Ref(n)))
                                    .map(|l| l.href.clone())
                                {
                                    app.activate_href(&href, true).await;
                                }
                            }
                            Hit::Result(i) => {
                                if let Some(url) = app
                                    .t()
                                    .doc()
                                    .and_then(|d| d.serp.hits.get(i).map(|h| h.url.clone()))
                                {
                                    app.open_url_in_new_tab(&url).await;
                                }
                            }
                            Hit::SiteNav(i) => {
                                if let Some(url) =
                                    app.t().doc().and_then(|d| d.nav.get(i).map(|n| n.url.clone()))
                                {
                                    app.open_url_in_new_tab(&url).await;
                                }
                            }
                            other => app.handle_hit(other).await,
                        }
                    }
                } else if m.kind == MouseEventKind::Moved {
                    if let Some(Hit::PageLink(n)) = hit {
                        if let Some(href) = app
                            .t()
                            .doc()
                            .and_then(|d| d.resolve_link(Ref(n)))
                            .map(|l| l.href.clone())
                        {
                            app.status = format!("→ {href}  ·  click open  ·  middle-click new tab");
                        }
                    } else if let Some(Hit::SiteNav(i)) = hit {
                        if let Some(item) = app.t().doc().and_then(|d| d.nav.get(i)) {
                            app.status = format!("→ {}  ·  {}", item.title, item.url);
                        }
                    }
                } else if matches!(
                    m.kind,
                    MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
                ) {
                    let view_h = app.height.saturating_sub(6);
                    if m.kind == MouseEventKind::ScrollDown {
                        app.tm().scroll = app.tm().scroll.saturating_add(3);
                    } else {
                        app.tm().scroll = app.tm().scroll.saturating_sub(3);
                    }
                    app.clamp_scroll(view_h);
                }
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if handle_key(app, key.code, key.modifiers).await? {
                    break;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

async fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) -> Result<bool> {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let view_h = app.height.saturating_sub(6);

    if let Overlay::Hints { new_tab, typed } = app.overlay.clone() {
        match code {
            KeyCode::Esc => app.cancel_modes(),
            KeyCode::Backspace => {
                if let Overlay::Hints { typed, .. } = &mut app.overlay {
                    typed.pop();
                }
            }
            KeyCode::Char(c) if c.is_ascii_lowercase() => {
                let typed = format!("{typed}{c}");
                let hits = keys::matches(&app.hints, &typed);
                if hits.len() == 1 {
                    let target = hits[0].1;
                    app.apply_hint(target, new_tab).await;
                } else if hits.is_empty() {
                    app.status = "no hint matches".into();
                    if let Overlay::Hints { typed: t, .. } = &mut app.overlay {
                        *t = typed;
                    }
                } else if let Overlay::Hints { typed: t, .. } = &mut app.overlay {
                    *t = typed;
                    app.status = format!("hints · {}", hits.iter().map(|(h, _)| h.as_str()).take(6).collect::<Vec<_>>().join(" "));
                }
            }
            _ => {}
        }
        return Ok(false);
    }

    if app.pending == Some('g') {
        app.pending = None;
        match code {
            KeyCode::Char('g') => app.tm().scroll = 0,
            KeyCode::Char('h') => app.go_home(),
            KeyCode::Char('i') => {
                if !app.focus_site_search() {
                    app.tm().focus = Focus::Omnibox;
                }
            }
            KeyCode::Char('t') => app.next_tab(),
            KeyCode::Char('T') => app.prev_tab(),
            KeyCode::Char('o') => app.tm().focus = Focus::Omnibox,
            _ => app.status = "g · gg top  gh home  gi search  gt/gT tabs  go open".into(),
        }
        return Ok(false);
    }
    if app.pending == Some('y') {
        app.pending = None;
        if matches!(code, KeyCode::Char('y')) {
            app.yank_url();
        }
        return Ok(false);
    }

    if app.overlay == Overlay::Find {
        match code {
            KeyCode::Esc => app.overlay = Overlay::None,
            KeyCode::Enter | KeyCode::Char('n') if !ctrl => app.find_next(),
            KeyCode::Backspace => {
                app.find_buf.pop();
            }
            KeyCode::Char(c) if !c.is_control() && c != 'n' => {
                app.find_buf.push(c);
                app.tm().find_idx = 0;
                app.find_next();
            }
            _ => {}
        }
        return Ok(false);
    }

    if app.overlay == Overlay::Help {
        if matches!(code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')) {
            app.cancel_modes();
        }
        return Ok(false);
    }

    if app.overlay == Overlay::History {
        match code {
            KeyCode::Esc | KeyCode::Char('h') if !ctrl => app.overlay = Overlay::None,
            KeyCode::Up | KeyCode::Char('k') => {
                app.hist_idx = app.hist_idx.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.hist_idx + 1 < app.visits.visits.len() {
                    app.hist_idx += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(u) = app.visits.visits.get(app.hist_idx).map(|v| v.url.clone()) {
                    app.overlay = Overlay::None;
                    app.go(&u).await;
                }
            }
            _ => {}
        }
        return Ok(false);
    }

    if mods.contains(KeyModifiers::ALT) && matches!(code, KeyCode::Char('v')) {
        app.tm().scroll = app.tm().scroll.saturating_sub(view_h.saturating_sub(1));
        return Ok(false);
    }

    if ctrl {
        match code {
            KeyCode::Char('g') => app.cancel_modes(),
            KeyCode::Char('n') => {
                app.tm().scroll = app.tm().scroll.saturating_add(1);
                app.clamp_scroll(view_h);
            }
            KeyCode::Char('p') => {
                app.tm().scroll = app.tm().scroll.saturating_sub(1);
            }
            KeyCode::Char('v') => {
                app.tm().scroll = app.tm().scroll.saturating_add(view_h.saturating_sub(1));
                app.clamp_scroll(view_h);
            }
            KeyCode::Char('d') => {
                app.tm().scroll = app.tm().scroll.saturating_add(view_h / 2);
                app.clamp_scroll(view_h);
            }
            KeyCode::Char('u') => {
                app.tm().scroll = app.tm().scroll.saturating_sub(view_h / 2);
            }
            KeyCode::Char('s') | KeyCode::Char('f') => {
                app.overlay = Overlay::Find;
                app.find_buf.clear();
                app.status = "find · type · n next · esc".into();
            }
            KeyCode::Char('t') => app.new_tab(),
            KeyCode::Char('w') => app.close_tab(),
            KeyCode::Char('l') => app.tm().focus = Focus::Omnibox,
            KeyCode::Char('h') => {
                app.overlay = Overlay::History;
                app.hist_idx = 0;
            }
            KeyCode::Char('r') => app.reload().await,
            KeyCode::Tab => {
                if mods.contains(KeyModifiers::SHIFT) {
                    app.prev_tab();
                } else {
                    app.next_tab();
                }
            }
            KeyCode::Char('c') => return Ok(true),
            _ => {}
        }
        return Ok(false);
    }

    if app.t().focus == Focus::SiteSearch {
        match code {
            KeyCode::Esc => app.tm().focus = Focus::Content,
            KeyCode::Enter => app.submit_site_search().await,
            KeyCode::Backspace => {
                app.tm().site_buf.pop();
            }
            KeyCode::Char(c) if !c.is_control() => app.tm().site_buf.push(c),
            _ => {}
        }
        return Ok(false);
    }

    if app.t().focus == Focus::Omnibox {
        match code {
            KeyCode::Esc => {
                if app.t().screen != Screen::Home {
                    app.tm().focus = Focus::Content;
                }
            }
            KeyCode::Enter => app.submit_omnibox().await,
            KeyCode::Backspace => {
                app.omnibox.pop();
            }
            KeyCode::Char(c) if !c.is_control() => app.omnibox.push(c),
            _ => {}
        }
        return Ok(false);
    }

    match code {
        KeyCode::Esc => app.cancel_modes(),
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('?') => app.overlay = Overlay::Help,
        KeyCode::Char('t') => app.new_tab(),
        KeyCode::Char('x') | KeyCode::Char('w') => app.close_tab(),
        KeyCode::Char('[') | KeyCode::Char('H') => app.back(),
        KeyCode::Char(']') | KeyCode::Char('L') => app.forward(),
        KeyCode::Char('g') => {
            app.pending = Some('g');
            app.status = "g · g top  h home  i search  t/T tabs  o open".into();
        }
        KeyCode::Char('y') => {
            app.pending = Some('y');
            app.status = "y · y yank url".into();
        }
        _ => match app.t().screen {
            Screen::Home => {
                app.tm().focus = Focus::Omnibox;
                match code {
                    KeyCode::Enter => app.submit_omnibox().await,
                    KeyCode::Backspace => {
                        app.omnibox.pop();
                    }
                    KeyCode::Char(c) if !c.is_control() => app.omnibox.push(c),
                    _ => {}
                }
            }
            Screen::Browse => {
                let is_serp = app.t().doc().map(|d| d.is_serp()).unwrap_or(false);
                match code {
                    KeyCode::Char('f') => app.start_hints(false),
                    KeyCode::Char('F') => app.start_hints(true),
                    KeyCode::Char('m') => app.add_current_to_favorites(),
                    KeyCode::Char('s') => app.add_current_to_reading(),
                    KeyCode::Char('/') => {
                        if !app.focus_site_search() {
                            app.overlay = Overlay::Find;
                            app.find_buf.clear();
                            app.status = "find · type · n next · esc".into();
                        }
                    }
                    KeyCode::Char('o') | KeyCode::Char(':') => {
                        app.tm().focus = Focus::Omnibox;
                    }
                    KeyCode::Char('r') => app.reload().await,
                    KeyCode::Char('j') | KeyCode::Down => {
                        if is_serp {
                            let n = app.t().doc().map(|d| d.serp.hits.len()).unwrap_or(0);
                            if app.t().selected_result + 1 < n {
                                app.tm().selected_result += 1;
                            }
                        } else {
                            app.tm().scroll = app.tm().scroll.saturating_add(1);
                            app.clamp_scroll(view_h);
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if is_serp {
                            app.tm().selected_result = app.t().selected_result.saturating_sub(1);
                        } else {
                            app.tm().scroll = app.tm().scroll.saturating_sub(1);
                        }
                    }
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        app.tm().scroll = app.tm().scroll.saturating_add(view_h.saturating_sub(1));
                        app.clamp_scroll(view_h);
                    }
                    KeyCode::PageUp => {
                        app.tm().scroll = app.tm().scroll.saturating_sub(view_h.saturating_sub(1));
                    }
                    KeyCode::Home => app.tm().scroll = 0,
                    KeyCode::End | KeyCode::Char('G') => {
                        app.tm().scroll = app.t().layout.lines.len().saturating_sub(1) as u16;
                        app.clamp_scroll(view_h);
                    }
                    KeyCode::Tab | KeyCode::Char('n') => {
                        let len = app.t().layout.link_order.len();
                        if len > 0 {
                            app.tm().selected_link = (app.t().selected_link + 1) % len;
                            ensure_link_visible(app, view_h);
                            app.preview_link();
                        }
                    }
                    KeyCode::BackTab | KeyCode::Char('p') => {
                        let len = app.t().layout.link_order.len();
                        if len > 0 {
                            app.tm().selected_link = if app.t().selected_link == 0 {
                                len - 1
                            } else {
                                app.t().selected_link - 1
                            };
                            ensure_link_visible(app, view_h);
                            app.preview_link();
                        }
                    }
                    KeyCode::Enter if mods.contains(KeyModifiers::CONTROL)
                        || mods.contains(KeyModifiers::SHIFT) =>
                    {
                        app.open_selected_in_new_tab().await;
                    }
                    KeyCode::Enter => app.navigate_selected().await,
                    KeyCode::Char('T') => app.open_selected_in_new_tab().await,
                    KeyCode::Char(c) if c.is_ascii_digit() && is_serp => {
                        let n = c.to_digit(10).unwrap() as usize;
                        if n >= 1 {
                            if let Some(len) = app.t().doc().map(|d| d.serp.hits.len()) {
                                if n <= len {
                                    app.tm().selected_result = n - 1;
                                }
                            }
                        }
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        let n = c.to_digit(10).unwrap() as usize;
                        let len = app.t().layout.link_order.len();
                        if n >= 1 && n <= len {
                            app.tm().selected_link = n - 1;
                            ensure_link_visible(app, view_h);
                            app.preview_link();
                        }
                    }
                    _ => {}
                }
            }
        },
    }
    Ok(false)
}

fn ensure_link_visible(app: &mut App, view_h: u16) {
    let Some(r) = app.selected_ref() else {
        return;
    };
    for (i, line) in app.t().layout.lines.iter().enumerate() {
        let has = line
            .segments
            .iter()
            .any(|s| matches!(s, Segment::Link { r#ref, .. } if *r#ref == r));
        if has {
            let i = i as u16;
            if i < app.t().scroll {
                app.tm().scroll = i;
            } else if i >= app.t().scroll + view_h {
                app.tm().scroll = i.saturating_sub(view_h.saturating_sub(1));
            }
            break;
        }
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::new().bg(app.t().theme.bg)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);

    draw_tab_strip(frame, app, chunks[0]);
    draw_nav_bar(frame, app, chunks[1]);

    match app.t().screen {
        Screen::Home => draw_home(frame, app, chunks[2]),
        Screen::Browse => {
            if app.t().doc().map(|d| d.is_serp()).unwrap_or(false) {
                draw_serp(frame, app, chunks[2]);
            } else if app
                .t()
                .doc()
                .map(|d| d.wants_centered_search())
                .unwrap_or(false)
            {
                draw_centered_search(frame, app, chunks[2]);
            } else {
                let body = draw_site_search(frame, app, chunks[2]);
                let body = draw_site_nav(frame, app, body);
                draw_content(frame, app, body);
            }
        }
    }

    match app.overlay {
        Overlay::Help => draw_help(frame, app, chunks[2]),
        Overlay::History => draw_history(frame, app, chunks[2]),
        Overlay::Find | Overlay::Hints { .. } | Overlay::None => {}
    }

    draw_status(frame, app, chunks[3]);
}

fn draw_tab_strip(frame: &mut Frame, app: &mut App, area: Rect) {
    let mut x = area.x;
    for (i, tab) in app.tabs.iter().enumerate() {
        if x + 6 >= area.x + area.width {
            break;
        }
        let label = truncate(&tab.title(), 16);
        let text = format!(" {} ", label);
        let w = text.chars().count() as u16 + 2;
        let sel = i == app.tab;
        let rect = Rect::new(x, area.y, w.min(area.x + area.width - x), 1);
        let style = if sel {
            style_panel(app.t().theme.accent).add_modifier(Modifier::BOLD)
        } else {
            style_chrome_text(app.t().theme.text_dim)
        };
        frame.render_widget(Paragraph::new(text).style(style), rect);
        app.hits.push((rect, Hit::Tab(i)));
        let close = Rect::new(rect.x + rect.width.saturating_sub(2), rect.y, 1, 1);
        frame.render_widget(Paragraph::new("×").style(style), close);
        app.hits.push((close, Hit::CloseTab(i)));
        x = x.saturating_add(w);
    }
    let plus = Rect::new(x.saturating_add(1), area.y, 3, 1);
    if plus.x + plus.width <= area.x + area.width {
        frame.render_widget(
            Paragraph::new(" + ").style(style_chrome_text(app.t().theme.accent)),
            plus,
        );
        app.hits.push((plus, Hit::NewTab));
    }
}

fn draw_nav_bar(frame: &mut Frame, app: &mut App, area: Rect) {
    let th = app.t().theme;
    let back_on = app.t().session.can_back() || app.t().screen == Screen::Browse;
    let fwd_on = app.t().session.can_forward();
    let btns = [
        (Hit::Back, " ◀ ", back_on),
        (Hit::Forward, " ▶ ", fwd_on),
        (Hit::Reload, " ↻ ", app.t().session.current().is_some()),
        (Hit::HomeBtn, " ⌂ ", true),
    ];
    let mut x = area.x;
    for (hit, label, on) in btns {
        let w = label.chars().count() as u16;
        let rect = Rect::new(x, area.y, w, 1);
        let fg = if on { th.text } else { th.text_dim };
        frame.render_widget(Paragraph::new(label).style(style_chrome_text(fg)), rect);
        app.hits.push((rect, hit));
        x += w;
    }
    if app.t().screen == Screen::Home {
        return;
    }
    x += 1;
    let box_w = area.width.saturating_sub(x - area.x).saturating_sub(1);
    let omnibox = Rect::new(x, area.y, box_w, 1);
    let focused = app.t().focus == Focus::Omnibox;
    let shown = if focused {
        format!(" {}█", app.omnibox)
    } else if app.omnibox.is_empty() {
        " search or enter a url".into()
    } else {
        format!(" {}", truncate(&app.omnibox, box_w.saturating_sub(2) as usize))
    };
    let style = if focused {
        style_panel(th.link_active)
    } else {
        style_panel(th.text)
    };
    frame.render_widget(Paragraph::new(shown).style(style), omnibox);
    app.hits.push((omnibox, Hit::Omnibox));
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let th = app.t().theme;
    let prompt = if app.overlay == Overlay::Find {
        format!(" find: {}█", app.find_buf)
    } else {
        format!(" {}", app.status)
    };
    let help = "  j/k scroll  f hints  H/L back/fwd  gh home  o open  gi site  ? keys  q quit";
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(prompt, style_chrome_text(th.text_dim))),
            Line::from(Span::styled(help, style_chrome_text(th.accent_dim))),
        ]),
        area,
    );
}

fn draw_home(frame: &mut Frame, app: &mut App, area: Rect) {
    draw_centered_search(frame, app, area);
}

fn draw_site_search(frame: &mut Frame, app: &mut App, area: Rect) -> Rect {
    let form = match app.t().doc().and_then(|d| d.site_search.clone()) {
        Some(f) => f,
        None => return area,
    };
    if area.height < 3 {
        return area;
    }
    let th = app.t().theme;
    let focused = app.t().focus == Focus::SiteSearch;
    let row = Rect::new(area.x + 2, area.y, area.width.saturating_sub(4), 3);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", form.placeholder))
        .border_style(Style::new().fg(if focused { th.accent } else { th.border }).bg(panel_bg()))
        .style(Style::new().bg(panel_bg()));
    let inner = block.inner(row);
    frame.render_widget(block, row);
    let shown = if app.t().site_buf.is_empty() && !focused {
        format!("  {}  ·  / or F to type", form.placeholder)
    } else {
        format!("  {}{}", app.t().site_buf, if focused { "█" } else { "" })
    };
    frame.render_widget(
        Paragraph::new(shown).style(Style::new().fg(if focused { th.link_active } else { th.text }).bg(panel_bg())),
        inner,
    );
    app.hits.push((row, Hit::SiteSearch));
    Rect::new(area.x, area.y + 3, area.width, area.height.saturating_sub(3))
}

fn draw_site_nav(frame: &mut Frame, app: &mut App, area: Rect) -> Rect {
    let items = app
        .t()
        .doc()
        .map(|d| d.nav.clone())
        .unwrap_or_default();
    if items.is_empty() || area.height < 3 {
        return area;
    }
    let th = app.t().theme;
    let row = Rect::new(area.x, area.y, area.width, 1);
    let mut x = area.x + 1;
    frame.render_widget(
        Block::default().style(Style::new().bg(panel_bg()).fg(th.text_dim)),
        row,
    );
    for (i, item) in items.iter().enumerate() {
        let hint = app
            .hints
            .iter()
            .find(|(_, t)| *t == HintTarget::Nav(i))
            .map(|(h, _)| format!("[{h}]"))
            .unwrap_or_default();
        let label = format!(" {}{hint} ", truncate(&item.title, 16));
        let w = label.chars().count() as u16;
        if x + w >= area.x + area.width {
            break;
        }
        let rect = Rect::new(x, area.y, w, 1);
        frame.render_widget(
            Paragraph::new(label).style(style_panel(th.link)),
            rect,
        );
        app.hits.push((rect, Hit::SiteNav(i)));
        x = x.saturating_add(w + 1);
    }
    Rect::new(area.x, area.y + 1, area.width, area.height.saturating_sub(1))
}

fn center_text(frame: &mut Frame, area: Rect, y: u16, text: &str, style: Style) {
    let w = text.chars().count() as u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    frame.render_widget(Paragraph::new(text).style(style), Rect::new(x, y, w, 1));
}

fn draw_serp(frame: &mut Frame, app: &mut App, area: Rect) {
    let th = app.t().theme;
    let Some(doc) = app.t().doc() else {
        return;
    };
    let query = doc.serp.query.clone();
    let instant = doc.serp.instant.clone();
    let hits: Vec<SearchHit> = doc.serp.hits.clone();
    let selected = app.t().selected_result;

    let mut y = area.y + 1;
    let header = if query.is_empty() {
        "Search results".into()
    } else {
        format!("Results for “{query}”")
    };
    frame.render_widget(
        Paragraph::new(format!("  {header}")).style(style_panel(th.heading).add_modifier(Modifier::BOLD)),
        Rect::new(area.x + 2, y, area.width.saturating_sub(4), 1),
    );
    y += 2;

    if let Some(ans) = instant {
        let h = 5u16.min(area.y + area.height - y);
        if h >= 4 {
            let rect = Rect::new(area.x + 3, y, area.width.saturating_sub(6), h);
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" instant ")
                .border_style(Style::new().fg(th.accent).bg(panel_bg()))
                .style(Style::new().bg(panel_bg()));
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        truncate(&ans.title, inner.width as usize),
                        Style::new()
                            .fg(th.link)
                            .bg(panel_bg())
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        truncate(&ans.snippet, inner.width as usize * 2),
                        Style::new().fg(th.text).bg(panel_bg()),
                    )),
                ])
                .wrap(Wrap { trim: true }),
                inner,
            );
            app.hits.push((rect, Hit::Instant));
            y += h + 1;
        }
    }

    let card_h = 5u16;
    let start = selected.saturating_sub(2);
    for (i, hit) in hits.iter().enumerate().skip(start) {
        if y + card_h > area.y + area.height {
            break;
        }
        let rect = Rect::new(area.x + 3, y, area.width.saturating_sub(6), card_h);
        let hint = app
            .hints
            .iter()
            .find(|(_, t)| *t == HintTarget::Result(i))
            .map(|(h, _)| h.as_str());
        draw_result_card(frame, &th, i, hit, rect, i == selected, hint);
        app.hits.push((rect, Hit::Result(i)));
        y += card_h + 1;
    }
}

fn draw_result_card(
    frame: &mut Frame,
    th: &Theme,
    idx: usize,
    hit: &SearchHit,
    area: Rect,
    selected: bool,
    hint: Option<&str>,
) {
    let border = if selected { th.accent } else { th.border };
    let title = match hint {
        Some(h) => format!(" {} [{h}] ", idx + 1),
        None => format!(" {} ", idx + 1),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::new().fg(border).bg(panel_bg()))
        .style(Style::new().bg(panel_bg()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let title_style = if selected {
        Style::new()
            .fg(th.link_active)
            .bg(panel_bg())
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::new()
            .fg(th.link)
            .bg(panel_bg())
            .add_modifier(Modifier::BOLD)
    };
    frame.render_widget(
        Paragraph::new(truncate(&hit.title, inner.width as usize)).style(title_style),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    if inner.height > 1 {
        frame.render_widget(
            Paragraph::new(truncate(&hit.display_url, inner.width as usize))
                .style(Style::new().fg(th.success).bg(panel_bg())),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
    }
    if inner.height > 2 {
        frame.render_widget(
            Paragraph::new(truncate(&hit.snippet, inner.width as usize * 2))
                .style(Style::new().fg(th.text).bg(panel_bg()))
                .wrap(Wrap { trim: true }),
            Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 2),
        );
    }
}

fn draw_centered_search(frame: &mut Frame, app: &App, area: Rect) {
    let th = app.t().theme;
    let box_w = (area.width as usize * 62 / 100).clamp(40, 78) as u16;
    let box_h: u16 = 5;
    let top = area.y + area.height.saturating_sub(8) / 2;
    let left = area.x + area.width.saturating_sub(box_w) / 2;
    center_text(
        frame,
        area,
        top,
        "◆  DuckDuckGo",
        style_panel(th.accent).add_modifier(Modifier::BOLD),
    );
    let box_area = Rect::new(left, top + 2, box_w.min(area.width), box_h);
    // click-to-focus is handled by the caller when this is the home search.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(th.accent).bg(panel_bg()))
        .style(Style::new().bg(panel_bg()))
        .title(" ▎ search ");
    let inner = block.inner(box_area);
    frame.render_widget(block, box_area);
    let display = if app.omnibox.is_empty() {
        "  type a query, then enter".into()
    } else {
        format!("  {}█", app.omnibox)
    };
    frame.render_widget(
        Paragraph::new(display).style(Style::new().fg(th.text).bg(panel_bg())),
        Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1) / 2,
            inner.width,
            1,
        ),
    );
}

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect) {
    let th = app.t().theme;
    let selected = app.selected_ref();
    let view_h = area.height as usize;
    let start = app.t().scroll as usize;
    let end = (start + view_h).min(app.t().layout.lines.len());
    let q = if app.overlay == Overlay::Find {
        app.find_buf.to_ascii_lowercase()
    } else {
        String::new()
    };

    let mut link_hits: Vec<(Rect, Hit)> = Vec::new();
    let buf = frame.buffer_mut();
    let mut y = area.y;
    for line in &app.t().layout.lines[start..end] {
        if y >= area.y + area.height {
            break;
        }
        let mut x = area.x + 2;
        put_cell(buf, area.x, y, '▎', th.accent, Some(th.bg));
        for seg in &line.segments {
            if x >= area.x + area.width {
                break;
            }
            match seg {
                Segment::Text { text, style } => {
                    let st = match style {
                        LayStyle::Heading1 => th.heading,
                        LayStyle::Heading2 => th.heading,
                        LayStyle::Heading3 => th.heading,
                        LayStyle::Math => th.code,
                        LayStyle::Dim => th.text_dim,
                        LayStyle::Quote => th.quote,
                        LayStyle::Pre | LayStyle::Code => th.code,
                        LayStyle::Strong => th.heading,
                        LayStyle::Em => th.text,
                        LayStyle::Border => th.border,
                        LayStyle::Image => th.text_dim,
                        LayStyle::Normal => th.text,
                    };
                    x = put_str(buf, x, y, area.x + area.width, text, st, th.bg, &q);
                }
                Segment::Link { r#ref, text } => {
                    let active = Some(*r#ref) == selected;
                    let fg = if active { th.link_active } else { th.link };
                    let x0 = x;
                    x = put_str(buf, x, y, area.x + area.width, text, fg, th.bg, &q);
                    if x > x0 {
                        link_hits.push((
                            Rect::new(x0, y, x - x0, 1),
                            Hit::PageLink(r#ref.0),
                        ));
                    }
                    if let Overlay::Hints { typed, .. } = &app.overlay {
                        if let Some((hint, _)) =
                            app.hints.iter().find(|(_, t)| *t == HintTarget::Link(r#ref.0))
                        {
                            if hint.starts_with(typed.as_str()) {
                                let label = format!("[{hint}]");
                                x = put_str(
                                    buf,
                                    x,
                                    y,
                                    area.x + area.width,
                                    &label,
                                    indexed_warn(),
                                    chrome_bg(),
                                    "",
                                );
                            }
                        }
                    }
                }
            }
        }
        y += 1;
    }
    app.hits.extend(link_hits);
}

fn put_cell(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16, ch: char, fg: Color, bg: Option<Color>) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char(ch);
        let mut st = Style::new().fg(fg);
        if let Some(bg) = bg {
            st = st.bg(bg);
        }
        cell.set_style(st);
    }
}

fn put_str(
    buf: &mut ratatui::buffer::Buffer,
    mut x: u16,
    y: u16,
    max: u16,
    text: &str,
    fg: Color,
    bg: Color,
    find: &str,
) -> u16 {
    let lower = text.to_ascii_lowercase();
    for (i, ch) in text.chars().enumerate() {
        if x >= max {
            break;
        }
        let hit = !find.is_empty() && lower[i.min(lower.len())..].starts_with(find);
        let color = if hit { indexed_warn() } else { fg };
        let cell_bg = if hit { chrome_bg() } else { bg };
        put_cell(buf, x, y, ch, color, Some(cell_bg));
        x += 1;
    }
    x
}

fn indexed_warn() -> Color {
    crate::color::indexed(253, 224, 71)
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let th = app.t().theme;
    let w = area.width.min(72);
    let h = area.height.min(20);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let modal = Rect::new(x, y, w, h);
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" help ")
        .border_style(Style::new().fg(th.accent).bg(panel_bg()))
        .style(Style::new().bg(panel_bg()));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let lines = [
        "VIM",
        "  j k     scroll          gg G     top / bottom",
        "  C-d C-u half-page       space    page down",
        "  H L     back / forward  gh       start page",
        "  r       reload          yy       yank url",
        "  f / F   link hints / in new tab  (type asdf…)",
        "  tab n p next / prev link   enter open   T new tab",
        "  o :     open url/search    gi or /  site search",
        "  t x     new / close tab    gt gT    next / prev tab",
        "  m s     bookmark / reading list",
        "EMACS",
        "  C-n C-p line   C-v M-v page   C-s find   C-g cancel",
        "  C-t C-w tabs   C-l address    C-h history",
        "  esc cancel · ? this help · q quit",
    ];
    frame.render_widget(
        Paragraph::new(
            lines
                .iter()
                .map(|l| Line::from(Span::styled(*l, Style::new().fg(th.text).bg(panel_bg()))))
                .collect::<Vec<_>>(),
        ),
        inner,
    );
}

fn draw_history(frame: &mut Frame, app: &mut App, area: Rect) {
    let th = app.t().theme;
    let w = area.width.min(78);
    let h = area.height.min(22);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let modal = Rect::new(x, y, w, h);
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" history ")
        .border_style(Style::new().fg(th.accent).bg(panel_bg()))
        .style(Style::new().bg(panel_bg()));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let visits = app.visits.visits.clone();
    if visits.is_empty() {
        frame.render_widget(
            Paragraph::new("no visits yet").style(Style::new().fg(th.text_dim).bg(panel_bg())),
            inner,
        );
        return;
    }
    let mut lines = Vec::new();
    for (i, v) in visits.iter().enumerate().take(inner.height as usize) {
        let sel = i == app.hist_idx;
        let row = format!(" {}  ·  {}", truncate(&v.title, 32), truncate(&v.url, 36));
        let style = if sel {
            Style::new()
                .fg(th.link_active)
                .bg(panel_bg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(th.text).bg(panel_bg())
        };
        lines.push(Line::from(Span::styled(row, style)));
        let row_rect = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        app.hits.push((row_rect, Hit::HistoryItem(i)));
    }
    frame.render_widget(Paragraph::new(lines), inner);
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
