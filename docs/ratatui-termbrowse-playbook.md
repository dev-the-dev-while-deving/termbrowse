# Ratatui × termbrowse playbook

**Purpose:** Map Ratatui concepts and production patterns onto this repo’s TUI so you can extend termbrowse without re-learning the ecosystem from scratch.

**Stack (locked):** `ratatui 0.30.2` · `crossterm 0.29.0` · `tokio` (async I/O outside the draw path)  
**Primary TUI file:** [`src/tui_session.rs`](../src/tui_session.rs) (~1.3k lines)  
**Related:** [`src/theme.rs`](../src/theme.rs) · [`src/layout.rs`](../src/layout.rs) · [`src/model.rs`](../src/model.rs) · [`src/session.rs`](../src/session.rs)

Companion survey of other TUI frameworks: [`TUI-frameworks-research.md`](./TUI-frameworks-research.md).

---

## 1. Mental model (Ratatui)

Ratatui is **immediate-mode rendering**, not a retained widget tree:

| Layer | Who owns it | termbrowse |
|-------|-------------|------------|
| App state | You | `App` in `tui_session.rs` |
| Event / input loop | You | `event_loop` + `crossterm::event` |
| Frame composition | You | `draw` → `draw_home` / `draw_browse` / modals |
| Cell buffer + diff | Ratatui | `terminal.draw(\|f\| …)` double-buffers and only writes changed cells |
| Backend (raw mode, alt screen) | Crossterm via `ratatui::init` / `restore` | `run_home` / `run` |

**Rule:** every frame rebuilds the full UI from `App`. Never mutate widgets in place between frames — mutate `App`, then redraw.

```
┌─────────────┐   poll 80ms    ┌──────────────┐
│  crossterm  │ ─────────────► │  App (state) │
│  Key events │                └──────┬───────┘
└─────────────┘                       │
                                      ▼
                               terminal.draw
                                      │
                    ┌─────────────────┼─────────────────┐
                    ▼                 ▼                 ▼
              title (1 row)      body (Min)      status (2 rows)
                    │                 │
                    │          Home | Browse | modal
                    │                 │
                    │          layout.lines → Paragraph
                    ▼
              Ratatui Buffer → crossterm write (diffed)
```

---

## 2. Module map: product layers vs TUI

termbrowse intentionally separates **document truth** from **terminal paint**:

```
URL → fetch → parse → Document (model)
                      │
                      ├─► layout_document → Layout { lines, link_order }   (role → cells)
                      │
                      └─► snapshot (agents) — same Document, no TUI
                      
Session (history stack) ──► App ──► theme Styles ──► Frame widgets
```

| File | Job | Ratatui touch? |
|------|-----|----------------|
| `model.rs` | Roles (`Block`, `Span`, `Ref`, forms) | No |
| `layout.rs` | Role → `LayoutLine` / `Segment` + wrap + boxes | No (pure layout; TUI only consumes it) |
| `session.rs` | History, open/back/forward/reload | No (async) |
| `theme.rs` | Semantic colors → `ratatui::style::Style` | Yes |
| `tui_session.rs` | Screens, focus, keys, draw | Yes (all of it) |
| `home.rs` | Favorites / reading list persistence | Indirect |

This split is a strength: you can re-skin the TUI without re-parsing HTML, and agents share `Document` without Ratatui.

---

## 3. Architecture as implemented

### 3.1 State machine (screens + focus)

```text
Screen::Home  ←→  Screen::Browse
     │                    │
  home_section         Focus::{ Content, Search, OpenUrl }
  fav_idx / read_idx
  edit: Option<EditState>   ← modal overlays either screen
```

| Enum | Values | Role |
|------|--------|------|
| `Screen` | `Home`, `Browse` | Top-level mode |
| `HomeSection` | `Favorites`, `ReadingList` | Section focus on Start Page |
| `Focus` | `Search`, `Content`, `OpenUrl` | Input routing while browsing |
| `EditField` / `EditKind` | title/url · add/edit fav/reading | Modal form |

**Ratatui analogy:** this is the “model” half of Elm MVU, but hand-rolled. Bubble Tea would call these messages; here `match key.code` is the update function.

### 3.2 Event loop pattern

```rust
// tui_session.rs — event_loop (simplified)
loop {
    terminal.draw(|f| draw(f, app))?;
    if !event::poll(Duration::from_millis(80))? { continue; }
    let Event::Key(key) = event::read()? else { continue; };
    if key.kind != KeyEventKind::Press { continue; }
    // route: edit modal → home keys → browse focus → content keys
}
```

| Choice | Why it matters |
|--------|----------------|
| **80ms poll** | Lets the loop wake without keys (future: spinner / clock / async completion). Cheap idle CPU. |
| **KeyEventKind::Press** | Avoids double-fire on key release (esp. Windows / some macOS configs). |
| **Async only in handlers** | `app.go().await` / `navigate_selected().await` run **between** draws — UI freezes during fetch. Acceptable now; bottleneck for “feels instant” later. |
| **stderr for tracing** | `main` logs to stderr so stdout/alt-screen stay clean. Correct production practice. |

### 3.3 Shell layout (chrome)

Every frame uses a classic three-band constraint layout:

```rust
Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(1),  // title bar
        Constraint::Min(1),     // body
        Constraint::Length(2),  // status + help
    ])
```

| Band | Content |
|------|---------|
| Title | `termbrowse` accent + page title/url or Start Page counts |
| Body | Home grid / document content / centered search |
| Status | prompt (`>` line) + context-sensitive help |

This is textbook Ratatui chrome. Prefer **Length for chrome, Min for content** so resize never collapses the title/status first.

### 3.4 Document → cells → widgets

Browse path is **not** a Ratatui `List` of blocks. It is:

1. `layout::layout_document(doc, content_w)` → pre-wrapped `LayoutLine`s with styles and `link_order`
2. `draw_content` slices `lines[scroll..scroll+view_h]`
3. Maps each `Segment` → `Span` with `Theme` styles
4. Renders one `Paragraph` for the viewport
5. Prefixes every line with accent rail `▎`

```text
layout.lines ──scroll──► viewport lines ──Theme──► Vec<Line> ──► Paragraph
     ▲
link_order[selected_link] ──► highlight active Link span
```

**Why custom layout instead of Paragraph wrap?** Role-aware boxes (pre, table, frame), quote bars, link refs `[eN]`, and stable `link_order` for Tab navigation — all precomputed and shared with `snapshot` / CLI `text`.

### 3.5 Widgets already in use

| Widget | Where | Use |
|--------|-------|-----|
| `Paragraph` | Title, status, body lines, search boxes, modal fields | Primary drawing primitive |
| `Block` + `Borders` | Fav tiles, edit modal, search boxes | Chrome / focus ring |
| `Clear` | Modal + top search overlay | Punch a clean rect over content |
| `Span` / `Line` / `Style` | Everywhere | Dense multi-style lines |

**Not used yet (available in 0.30):** `List` / `ListState`, `Table` / `TableState`, `Scrollbar`, `Tabs`, `Gauge`, `Canvas`, `StatefulWidget`, mouse events, `ratatui::crossterm` helpers beyond init.

### 3.6 Theme as semantic tokens

[`theme.rs`](../src/theme.rs) is a small design system, not ad-hoc RGB in draw code:

| Slot | Role |
|------|------|
| `bg` / `bg_panel` | Page vs chrome / selected surfaces |
| `accent` / `accent_dim` | Brand rail, titles, secondary help |
| `text` / `text_dim` / `heading` | Hierarchy |
| `link` / `link_active` | Inactive vs focused links (yellow focus) |
| `code` / `quote` / `border` / `image` | Role styles |
| `success` / `warn` | Reserved product signals |

Helpers (`title_bar`, `link(active)`, `heading(level)`, …) keep draw code readable. This matches Lip Gloss / Textual CSS practice: **tokens first, components second**.

---

## 4. Interaction map (current keybindings)

### Start Page (`Screen::Home`)

| Keys | Action |
|------|--------|
| arrows / `hjkl` | Move in favorites grid or reading list |
| `Tab` | Favorites ↔ Reading List |
| `Enter` | Open selected URL |
| `a` / `e` / `d` | Add / edit / delete |
| `/` | Open DuckDuckGo HTML |
| `o` / `:` | Add via URL-focused modal |
| `q` / Esc / Ctrl+C | Quit |

### Browse (`Screen::Browse`)

| Keys | Action |
|------|--------|
| `j/k`, arrows, PgUp/Dn, space, `g`/`G` | Scroll document |
| `Tab` / `n`, `BackTab` / `p` | Next / prev link |
| digits `1`–`9` | Jump to link by index (if in range) |
| `Enter` | Follow selected link |
| `/` or `i` | Focus page search form |
| `o` / `:` | Open URL prompt |
| `r` | Reload |
| `[` / `]` | Back / forward history |
| `f` / `s` | Add to Favorites / Reading List |
| `H` / Esc | Home |
| `q` / Ctrl+C | Quit |

**Convention fit:** dual arrows+vim, `q`/Ctrl+C quit, Tab for focus — matches the Charm/Bubbles keyboard-first baseline.

---

## 5. What termbrowse already does well

1. **Clear layering** — Document / layout / session / theme / TUI are separate.
2. **Semantic theme** — One `Theme` type; draw code almost never invents colors.
3. **Constraint chrome** — Title / body / status never fight the content area.
4. **Custom layout engine** — Role-aware structure shared with agents (`snapshot`).
5. **Focus / screen enums** — Input routing is explicit, not boolean soup (mostly).
6. **Overlays done right** — `Clear` + centered `Block` for modal and search.
7. **Logging to stderr** — TUI owns the terminal; tracing doesn’t corrupt the buffer.
8. **Link visibility** — `ensure_link_visible` keeps Tab navigation usable while scrolled.
9. **`ratatui::init` / `restore`** — Modern 0.29+ lifecycle instead of hand-rolled raw mode.

---

## 6. Gaps and concrete upgrades

Ordered by impact for *this* product. Not a wishlist of every Ratatui feature.

### P0 — correctness / feel

| Gap | Today | Upgrade |
|-----|-------|---------|
| **Blocking network in event loop** | `go` / `navigate` / `reload` `.await` freeze the TUI | Load in a `tokio` task; set `status = "loading…"`; draw spinner or pulse status on 80ms ticks; apply result when ready. Pattern: `enum LoadState { Idle, Loading { url }, Ready(Result<…>) }` on `App`. |
| **Resize mid-frame** | Relayout only when width changes at end of key handler | Also call `relayout` when `Event::Resize` is received (currently non-Key events are ignored). |
| **Home favorites overflow** | Grid draws until `ty + tile_h` clips; no scroll | Track `home_scroll` or use a Ratatui `List` / virtual grid with scroll. |
| **`truncate` vs display width** | Char count, not Unicode width | Reuse `layout.rs` width helpers (`UnicodeWidthStr`) for title bars and tiles. |

### P1 — Ratatui widgets you can adopt without rewriting layout

| Widget | Fit | Notes |
|--------|-----|-------|
| **`Scrollbar`** | Browse body when `layout.lines.len() > view_h` | Pair with existing `scroll` / `view_h`. Low risk, high polish. |
| **`List` + `ListState`** | Reading List | Replaces hand-indexed rows; built-in highlight + scroll. |
| **`Tabs`** | Favorites / Reading List section | Visual mirror of `home_section` instead of `◆` title only. |
| **`Table` + `TableState`** | Optional: dense reading list with columns title / host / date | Only if product needs multi-column; tiles stay custom. |

Keep **document body** on custom `layout` + `Paragraph` — Ratatui `Paragraph` wrap would fight role boxes and link refs.

### P2 — architecture cleanups inside `tui_session.rs`

| Issue | Suggestion |
|-------|------------|
| Single ~1.3k file | Split: `ui/app.rs` (state), `ui/events.rs`, `ui/draw/{home,browse,modal}.rs`, keep `theme` |
| Key handling nested matches | `fn handle_key(app, key) -> Action` + `async fn apply(action)` — clearer test surface |
| Open-URL from home reuses `EditState` with fake title `"New"` | Dedicated `Focus::OpenUrl` on Home, or `EditKind::OpenUrl` |
| `success` / `warn` theme slots unused | Status line: green on save, yellow on error / CAPTCHA |
| No mouse | Optional: `EnableMouseCapture`; click favorites tile / link under cursor (nice-to-have) |

### P3 — testing (ecosystem pattern)

Ratatui apps are testable without a real TTY:

1. **Unit:** pure functions — `layout_document`, `ensure_link_visible`, key→state transitions with a fake `App`.
2. **Buffer assert:** `ratatui::Terminal::new(TestBackend::new(w, h))`, `draw`, assert `backend.buffer()` cells / styles.
3. **No Pilot-equivalent in-tree** — don’t wait on Textual-style Pilot; TestBackend is enough for chrome + selection highlight.

Suggested first tests:

- layout: link_order stable, wrap width, table box width
- theme: `link(true)` vs `link(false)` styles differ
- keys: from Home, `Tab` flips section; from Browse, `n` advances `selected_link` with clamp

### P4 — optional ecosystem crates

| Crate | When |
|-------|------|
| `tui-textarea` / `tui-input` | Multi-line or richer search/edit fields |
| `throbber-widgets-tui` or custom spinner | Loading states |
| `color-eyre` + panic hook that `restore()`s terminal | Crash safety (always restore raw mode) |
| `signal-hook` / crossterm resize | Already partially covered by crossterm events |

Stay lean: termbrowse’s philosophy is custom structure browsing — add crates only for pain you feel twice.

---

## 7. Patterns to copy when adding UI

### 7.1 New screen

1. Add `Screen::…` or a nested focus enum.
2. Branch **input** early in `event_loop` (modal-like gates first).
3. Branch **draw** in `draw` / body match.
4. Update status + help strings together (never only one).
5. Persist nothing until commit (edit modal pattern).

### 7.2 New chrome region

Prefer nesting constraints over absolute `Rect` math:

```rust
let rows = Layout::vertical([
    Constraint::Length(1),
    Constraint::Min(0),
    Constraint::Length(1),
]).split(body);
// then Layout::horizontal for side panels if needed
```

Manual `Rect::new` is fine for **overlays** (modal, centered search) — already the local style.

### 7.3 New role in the document pipeline

1. Extend `model::Block` / `Span`
2. Teach `parse` to emit it
3. Teach `layout` to emit `Segment`s + style
4. Map style in `draw_content` via `Theme`
5. Include in `snapshot` if agents need it  

**Do not** special-case HTML tags only inside `draw_*`.

### 7.4 Loading without freezing (target pattern)

```text
Key Enter
  → app.load_state = Loading { url }
  → spawn: fetch+parse → channel/oneshot
poll tick (80ms)
  → if Loading: animate status / spinner
  → if channel ready: session.push, after_nav(), Idle
draw always runs
```

Keep **all** network in `session` / `fetch`; TUI only owns `LoadState`.

---

## 8. Design tokens cheat sheet

| Token | RGB (GrokNight) | Use |
|-------|-----------------|-----|
| bg | `#0d0d10` | Page background |
| bg_panel | `#18181e` | Chrome, selected tiles, modals |
| accent | `#e879f9` | Brand, rail, active borders |
| accent_dim | `#78468c` | Help line |
| text | `#e8e6e3` | Body |
| text_dim | `#787673` | Meta, empty states |
| link | `#c084fc` | Links |
| link_active | `#fde047` | Focused link / field |
| code | `#86efac` | Code / pre |
| border | `#32323a` | Boxes, inactive tiles |
| success / warn | green / amber | Status (wire up) |

Truecolor (`Color::Rgb`) is fine on modern terminals; if you ever need 256-color fallback, map accents in one place inside `Theme`.

---

## 9. Async + TUI: rules of thumb

| Do | Don’t |
|----|-------|
| `tokio::main`, async load functions | Hold a `Mutex` across `terminal.draw` |
| Channel results into the event loop | Call `block_on` inside `draw` |
| Short poll timeout for animation | Spin 100% CPU with zero timeout forever |
| Restore terminal on every error path | `return Err` before `ratatui::restore()` (see `run` open failure — restore is correct there) |

Current `run` restores on open failure before returning `Err` — keep that pattern for panic hooks too.

---

## 10. Comparison: termbrowse vs other TUI architectures

| Concern | Textual | Bubble Tea | **termbrowse (Ratatui)** |
|---------|--------|------------|---------------------------|
| UI tree | Retained widgets + CSS | View fn → string/lines | Immediate draw from `App` |
| Layout | TCSS / fr units | Lip Gloss | Constraints + custom `layout.rs` |
| State updates | Messages / events | `Update(msg)` | `match KeyCode` |
| Document content | N/A | N/A | First-class `Document` model |
| Testing | Pilot + snapshots | harder (stdout owned) | TestBackend + pure layout tests |

You’re closest to **“hand-rolled Elm on Ratatui”**: model in `App`, update in key match, view in `draw`.

---

## 11. Suggested roadmap (product-shaped)

| Phase | Deliverable |
|-------|-------------|
| **A. Polish** | Scrollbar; Resize events; Unicode-width truncate; status colors for ok/error |
| **B. Responsiveness** | Non-blocking loads + loading indicator; cancel in-flight on new navigation |
| **C. Structure** | Split `tui_session.rs`; `Action` enum; unit + TestBackend tests |
| **D. Home UX** | Scrollable favorites; optional Tabs; Open URL without fake EditKind |
| **E. Optional** | Mouse click-to-open; help overlay (`?`); theme switch (light / high-contrast) |

PrivSearch TUI can reuse: same chrome layout, same theme, a `Screen::SearchResults` with `List`/`ListState`, and session open on Enter — without touching the document pipeline.

---

## 12. File / symbol index

| Symbol | File | Role |
|--------|------|------|
| `App` | `tui_session.rs` | All UI state |
| `event_loop` | `tui_session.rs` | Input + draw cycle |
| `draw` / `draw_home` / `draw_browse` / `draw_content` | `tui_session.rs` | Immediate-mode view |
| `Theme` / `Theme::groknight` | `theme.rs` | Design tokens |
| `layout_document` | `layout.rs` | Document → lines |
| `Session` | `session.rs` | Navigation history |
| `Document` / `Ref` / `Block` | `model.rs` | Structure browser model |
| `run` / `run_home` | `tui_session.rs` | Terminal lifecycle |
| CLI entry | `main.rs` | Routes to TUI or headless snapshot/text/search |

---

## 13. References (Ratatui)

- Concepts — rendering: https://ratatui.rs/concepts/rendering/
- Layout constraints: https://ratatui.rs/concepts/layout/
- Widgets: https://ratatui.rs/concepts/widgets/
- docs.rs crate: https://docs.rs/ratatui/latest/ratatui/
- Crossterm events: https://docs.rs/crossterm/latest/crossterm/event/

---

## 14. One-page checklist for PRs that touch the TUI

- [ ] State change lives on `App` (or `Session` / `HomeData`), not in a temporary draw-only variable
- [ ] New keys documented in status **and** help line for that screen
- [ ] Resize / width change still relayouts browse content
- [ ] Theme tokens used; no raw `Color::Rgb` in draw paths
- [ ] Network stays out of `draw`
- [ ] `ratatui::restore()` on all exit paths
- [ ] Layout/role changes update `layout.rs` + snapshot consumers if needed
- [ ] Manual smoke: home grid, open URL, scroll long page, tab links, search form, modal add/edit

---

*Generated as a Ratatui-focused deep dive against termbrowse 0.2.0 sources. Update this doc when `tui_session` is split or async loading lands.*
