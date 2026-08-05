//! Full-browser TUI.
//!
//! - **Kitty** (preferred): real-pixel 720p via the Kitty graphics protocol
//!   (also picks up iTerm2/Sixel when available).
//! - **CRT halfblocks**: fallback when no graphics protocol is available.

use crate::chrome::{FullBrowser, PageFrame, VIEW_H, VIEW_W};
use crate::term_image::{Phosphor, TermImage, TermImageWidget};
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use std::time::{Duration, Instant};

const SCAN_LINES_PER_TICK: u16 = 3;
const SCAN_TICK: Duration = Duration::from_millis(12);
const PAN_STEP: u16 = 8;
const PAN_STEP_BIG: u16 = 40;

/// How we paint the page into the terminal.
enum PaintMode {
    /// Real pixels (Kitty / iTerm2 / Sixel).
    Graphics {
        picker: Picker,
        /// Stateful image protocol for the current frame.
        protocol: Option<StatefulProtocol>,
        name: &'static str,
    },
    /// Unicode half-block CRT raster + pan.
    Crt {
        image: TermImage,
        pan_x: u16,
        pan_y: u16,
        scan_rows: u16,
        scanning: bool,
    },
}

pub struct FullApp {
    browser: FullBrowser,
    frame: PageFrame,
    paint: PaintMode,
    selected_link: usize,
    status: String,
    show_links: bool,
    link_scroll: u16,
    phosphor: Phosphor,
}

impl FullApp {
    pub fn open(url: &str, phosphor: Phosphor) -> Result<Self> {
        let browser = FullBrowser::launch()?;
        let frame = browser.open(url)?;
        let paint = init_paint_mode(phosphor)?;
        let mut app = Self {
            browser,
            frame,
            paint,
            selected_link: 0,
            status: String::new(),
            show_links: false,
            link_scroll: 0,
            phosphor,
        };
        app.rebuild_image()?;
        app.status = format!(
            "{} · 720p ({VIEW_W}×{VIEW_H}) · {}ms · {} links · j/k page · l links · q quit",
            app.mode_label(),
            app.frame.load_ms,
            app.frame.doc.links.len()
        );
        Ok(app)
    }

    fn mode_label(&self) -> &'static str {
        match &self.paint {
            PaintMode::Graphics { name, .. } => name,
            PaintMode::Crt { .. } => "CRT halfblocks",
        }
    }

    fn is_graphics(&self) -> bool {
        matches!(self.paint, PaintMode::Graphics { .. })
    }

    fn rebuild_image(&mut self) -> Result<()> {
        match &mut self.paint {
            PaintMode::Graphics { picker, protocol, .. } => {
                let dyn_img =
                    image::load_from_memory(&self.frame.png).context("decode screenshot png")?;
                // Stateful protocol resizes to the widget area at render time (pixel-perfect fit).
                *protocol = Some(picker.new_resize_protocol(dyn_img));
            }
            PaintMode::Crt {
                image,
                pan_x,
                pan_y,
                scan_rows,
                scanning,
            } => {
                *image = TermImage::from_png_720p(&self.frame.png, self.phosphor)
                    .context("build CRT raster")?;
                *pan_x = 0;
                *pan_y = 0;
                *scan_rows = 0;
                *scanning = true;
            }
        }
        Ok(())
    }

    fn apply_frame(&mut self, frame: PageFrame) -> Result<()> {
        self.frame = frame;
        self.selected_link = 0;
        self.link_scroll = 0;
        self.rebuild_image()?;
        self.status = format!(
            "{} · {} · {} links · {}ms",
            self.mode_label(),
            self.frame.doc.title,
            self.frame.doc.links.len(),
            self.frame.load_ms
        );
        Ok(())
    }

    fn scroll_page(&mut self, dy: i32) -> Result<()> {
        self.status = "page scroll…".into();
        let frame = self.browser.scroll(dy)?;
        self.frame = frame;
        self.rebuild_image()?;
        self.status = format!(
            "{} · {} · page {dy}px",
            self.mode_label(),
            self.frame.doc.title
        );
        Ok(())
    }

    fn navigate_selected(&mut self) -> Result<()> {
        let Some(link) = self.frame.doc.links.get(self.selected_link) else {
            self.status = "no link selected".into();
            return Ok(());
        };
        let href = link.href.clone();
        self.status = format!("opening {href} …");
        let frame = self.browser.open_href(&href)?;
        self.apply_frame(frame)?;
        Ok(())
    }

    fn reload(&mut self) -> Result<()> {
        self.status = "reloading…".into();
        let frame = self.browser.reload()?;
        self.apply_frame(frame)?;
        Ok(())
    }

    fn cycle_phosphor(&mut self) -> Result<()> {
        if self.is_graphics() {
            self.status = "phosphor is CRT-only (graphics mode uses real color pixels)".into();
            return Ok(());
        }
        self.phosphor = match self.phosphor {
            Phosphor::Color => Phosphor::Green,
            Phosphor::Green => Phosphor::Amber,
            Phosphor::Amber => Phosphor::Mono,
            Phosphor::Mono => Phosphor::Color,
        };
        self.rebuild_image()?;
        self.status = format!("phosphor → {:?}", self.phosphor);
        Ok(())
    }

    fn pan_crt(&mut self, dx: i32, dy: i32, view_w: u16, view_h: u16) {
        let PaintMode::Crt {
            image,
            pan_x,
            pan_y,
            ..
        } = &mut self.paint
        else {
            return;
        };
        let nx = (*pan_x as i32 + dx).max(0) as u16;
        let ny = (*pan_y as i32 + dy).max(0) as u16;
        let max_x = image.cols.saturating_sub(view_w);
        let max_y = image.rows.saturating_sub(view_h);
        *pan_x = nx.min(max_x);
        *pan_y = ny.min(max_y);
    }

    fn tick_scan(&mut self, view_h: u16) -> bool {
        let PaintMode::Crt {
            scan_rows,
            scanning,
            ..
        } = &mut self.paint
        else {
            return false;
        };
        if !*scanning {
            return false;
        }
        *scan_rows = scan_rows.saturating_add(SCAN_LINES_PER_TICK);
        if *scan_rows >= view_h.max(1) {
            *scan_rows = u16::MAX;
            *scanning = false;
            return false;
        }
        true
    }
}

fn init_paint_mode(phosphor: Phosphor) -> Result<PaintMode> {
    // Prefer querying the live terminal (works inside Kitty after alt-screen is up).
    let mut picker = match Picker::from_query_stdio() {
        Ok(p) => p,
        Err(_) => {
            // No query possible — still force Kitty if env says so.
            let mut p = Picker::halfblocks();
            if looking_at_kitty() {
                p.set_protocol_type(ProtocolType::Kitty);
            }
            p
        }
    };

    if looking_at_kitty() {
        picker.set_protocol_type(ProtocolType::Kitty);
    }

    let proto = picker.protocol_type();
    let name = match proto {
        ProtocolType::Kitty => "Kitty graphics",
        ProtocolType::Iterm2 => "iTerm2 graphics",
        ProtocolType::Sixel => "Sixel graphics",
        ProtocolType::Halfblocks => "halfblocks",
    };

    if matches!(
        proto,
        ProtocolType::Kitty | ProtocolType::Iterm2 | ProtocolType::Sixel
    ) {
        Ok(PaintMode::Graphics {
            picker,
            protocol: None,
            name,
        })
    } else {
        let _ = phosphor; // applied when first frame is built
        Ok(PaintMode::Crt {
            image: TermImage {
                cols: 0,
                rows: 0,
                src_w: 0,
                src_h: 0,
                cells: vec![],
            },
            pan_x: 0,
            pan_y: 0,
            scan_rows: 0,
            scanning: true,
        })
    }
}

fn looking_at_kitty() -> bool {
    if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        return true;
    }
    let term = std::env::var("TERM").unwrap_or_default();
    term.contains("kitty")
}

pub fn run(url: &str, phosphor: Phosphor) -> Result<()> {
    // Enter alt-screen first so capability query talks to the real TTY (Kitty).
    let mut terminal = ratatui::init();

    let result = (|| -> Result<()> {
        let mut app = FullApp::open(url, phosphor)?;
        event_loop(&mut terminal, &mut app)
    })();

    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut FullApp) -> Result<()> {
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| draw(f, app))?;

        let size = terminal.size()?;
        let body_h = size.height.saturating_sub(3);
        let body_w = if app.show_links {
            size.width.saturating_sub(34)
        } else {
            size.width
        };

        let scanning = matches!(
            &app.paint,
            PaintMode::Crt {
                scanning: true,
                ..
            }
        );
        let timeout = if scanning {
            SCAN_TICK
        } else {
            Duration::from_millis(50)
        };

        if event::poll(timeout)? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            let big = key.modifiers.contains(KeyModifiers::SHIFT);
            let step = if big { PAN_STEP_BIG } else { PAN_STEP } as i32;

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char('c') => {
                    let _ = app.cycle_phosphor();
                }
                KeyCode::Char('s') => {
                    if let PaintMode::Crt {
                        scan_rows,
                        scanning,
                        ..
                    } = &mut app.paint
                    {
                        *scan_rows = 0;
                        *scanning = true;
                        app.status = "CRT re-scan…".into();
                    } else {
                        app.status = "rescan is CRT-only; graphics paints full frame".into();
                    }
                }
                KeyCode::Char('f') => {
                    if let PaintMode::Crt {
                        scan_rows,
                        scanning,
                        ..
                    } = &mut app.paint
                    {
                        *scan_rows = u16::MAX;
                        *scanning = false;
                    }
                }

                // Pan only matters for CRT; in Kitty the full 720p frame is fit to the pane.
                KeyCode::Left | KeyCode::Char('a') => app.pan_crt(-step, 0, body_w, body_h),
                KeyCode::Right | KeyCode::Char('d') => app.pan_crt(step, 0, body_w, body_h),
                KeyCode::Up | KeyCode::Char('w') => {
                    if app.show_links {
                        app.selected_link = app.selected_link.saturating_sub(1);
                        ensure_link_list_visible(app, body_h);
                    } else {
                        app.pan_crt(0, -step, body_w, body_h);
                    }
                }
                KeyCode::Down | KeyCode::Char('x') => {
                    if app.show_links {
                        if app.selected_link + 1 < app.frame.doc.links.len() {
                            app.selected_link += 1;
                            ensure_link_list_visible(app, body_h);
                        }
                    } else {
                        app.pan_crt(0, step, body_w, body_h);
                    }
                }

                KeyCode::Char('j') => {
                    if app.show_links {
                        if app.selected_link + 1 < app.frame.doc.links.len() {
                            app.selected_link += 1;
                            ensure_link_list_visible(app, body_h);
                        }
                    } else if let Err(e) = app.scroll_page(240) {
                        app.status = format!("scroll error: {e:#}");
                    }
                }
                KeyCode::Char('k') => {
                    if app.show_links {
                        app.selected_link = app.selected_link.saturating_sub(1);
                        ensure_link_list_visible(app, body_h);
                    } else if let Err(e) = app.scroll_page(-240) {
                        app.status = format!("scroll error: {e:#}");
                    }
                }
                KeyCode::PageDown | KeyCode::Char(' ') => {
                    if let Err(e) = app.scroll_page(480) {
                        app.status = format!("scroll error: {e:#}");
                    }
                }
                KeyCode::PageUp => {
                    if let Err(e) = app.scroll_page(-480) {
                        app.status = format!("scroll error: {e:#}");
                    }
                }
                KeyCode::Tab | KeyCode::Char('l') => {
                    app.show_links = !app.show_links;
                }
                KeyCode::Char('n') => {
                    if !app.frame.doc.links.is_empty() {
                        app.selected_link = (app.selected_link + 1) % app.frame.doc.links.len();
                        app.show_links = true;
                        ensure_link_list_visible(app, body_h);
                    }
                }
                KeyCode::Char('p') => {
                    if !app.frame.doc.links.is_empty() {
                        if app.selected_link == 0 {
                            app.selected_link = app.frame.doc.links.len() - 1;
                        } else {
                            app.selected_link -= 1;
                        }
                        app.show_links = true;
                        ensure_link_list_visible(app, body_h);
                    }
                }
                KeyCode::Enter => {
                    if let Err(e) = app.navigate_selected() {
                        app.status = format!("nav error: {e:#}");
                    }
                }
                KeyCode::Char('r') => {
                    if let Err(e) = app.reload() {
                        app.status = format!("reload error: {e:#}");
                    }
                }
                KeyCode::Home | KeyCode::Char('g') => app.pan_crt(
                    -(app
                        .paint
                        .pan_xy()
                        .map(|(x, _)| x as i32)
                        .unwrap_or(0)),
                    -(app
                        .paint
                        .pan_xy()
                        .map(|(_, y)| y as i32)
                        .unwrap_or(0)),
                    body_w,
                    body_h,
                ),
                _ => {}
            }
        } else if scanning && last_tick.elapsed() >= SCAN_TICK {
            app.tick_scan(body_h);
            last_tick = Instant::now();
        }
    }
    Ok(())
}

impl PaintMode {
    fn pan_xy(&self) -> Option<(u16, u16)> {
        match self {
            PaintMode::Crt { pan_x, pan_y, .. } => Some((*pan_x, *pan_y)),
            _ => None,
        }
    }
}

fn ensure_link_list_visible(app: &mut FullApp, view_h: u16) {
    let i = app.selected_link as u16;
    if i < app.link_scroll {
        app.link_scroll = i;
    } else if i >= app.link_scroll + view_h.saturating_sub(1) {
        app.link_scroll = i.saturating_sub(view_h.saturating_sub(2));
    }
}

fn draw(frame: &mut Frame, app: &mut FullApp) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);

    let title = format!(
        " termbrowse 720p/{}  │  {}  │  {} ",
        app.mode_label(),
        app.frame.doc.title,
        app.frame.doc.url
    );
    frame.render_widget(
        Paragraph::new(title).style(Style::new().bg(Color::Black).fg(Color::Green)),
        chunks[0],
    );

    let body = chunks[1];
    let image_area = if app.show_links {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(20), Constraint::Length(34)])
            .split(body);
        draw_links(frame, app, cols[1]);
        cols[0]
    } else {
        body
    };

    match &mut app.paint {
        PaintMode::Graphics { protocol, .. } => {
            if let Some(proto) = protocol.as_mut() {
                // Scale 720p into the pane using real terminal pixels (Kitty/etc).
                let widget = StatefulImage::new().resize(Resize::Scale(Some(
                    image::imageops::FilterType::Triangle,
                )));
                frame.render_stateful_widget(widget, image_area, proto);
            }
        }
        PaintMode::Crt {
            image,
            pan_x,
            pan_y,
            scan_rows,
            scanning,
        } => {
            frame.render_widget(
                TermImageWidget {
                    image,
                    pan_x: *pan_x,
                    pan_y: *pan_y,
                    scan_rows: *scan_rows,
                    show_beam: *scanning,
                },
                image_area,
            );
        }
    }

    let link_info = app
        .frame
        .doc
        .links
        .get(app.selected_link)
        .map(|l| format!(" [{}] {} → {}", l.r#ref, l.text, l.href))
        .unwrap_or_default();
    let help = if app.is_graphics() {
        " 720p real pixels · j/k page · l links · Enter open · r reload · q quit"
    } else {
        " CRT fallback · arrows pan · j/k page · l links · c phosphor · q"
    };
    let status = format!("{}{}\n{}", app.status, link_info, help);
    frame.render_widget(
        Paragraph::new(status).style(Style::new().bg(Color::Black).fg(Color::DarkGray)),
        chunks[2],
    );
}

fn draw_links(frame: &mut Frame, app: &FullApp, area: Rect) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .title(" links ")
        .border_style(Style::new().fg(Color::Green));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    let start = app.link_scroll as usize;
    let h = inner.height as usize;
    for (idx, link) in app
        .frame
        .doc
        .links
        .iter()
        .enumerate()
        .skip(start)
        .take(h)
    {
        let selected = idx == app.selected_link;
        let label = format!(
            "{:>3} {}",
            link.r#ref,
            truncate(&link.text, (inner.width as usize).saturating_sub(4))
        );
        let style = if selected {
            Style::new()
                .bg(Color::Rgb(0, 40, 0))
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::Green)
        };
        lines.push(Line::from(Span::styled(label, style)));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
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
