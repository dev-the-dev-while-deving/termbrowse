# TUI frameworks: designs, implementations, and practices

**Source:** deep-research workflow  
**Status:** Partial (see coverage gaps at the end)  
**Date:** 2026-08-10  
**Related local project:** termbrowse (Ratatui + crossterm)  
**Deep dive mapped to this repo:** [`ratatui-termbrowse-playbook.md`](./ratatui-termbrowse-playbook.md)

---

Terminal UI work today is led by a small set of mature frameworks across languages—**Textual** (Python), **Bubble Tea** (Go), **Ratatui** (Rust), **Ink** (JS/TS), and **FTXUI** (C++)—each with a clear design model, layout system, widgets, and production tooling. You can pick by language and architecture (retained widget trees vs Elm-style MVU vs immediate-mode rendering), then reuse shared patterns for layout, CSS-like styling, keyboard-first UX, and theming. Below is a single consolidated map of those frameworks, designs, and implementation practices.

## Major frameworks by language

### Textual (Python)

**Textual** is a Python rapid-application framework for sophisticated terminal UIs that can also run in a browser, with a built-in widget set (buttons, trees, data tables, inputs, text areas, and more) and a flexible layout system; it is MIT-licensed, cross-platform, and usable over SSH. [[S1]](https://textual.textualize.io/)

- Docs: https://textual.textualize.io/
- Layout: outside-in docking, `fr` fractional units, scroll containers
- Styling: Textual CSS (`.tcss`), design variables, live reload (`textual run --dev`)
- Testing: headless `App.run_test()` + Pilot API; SVG snapshots

### Bubble Tea (Go)

**Bubble Tea** is a Go framework for inline, full-window, or mixed terminal apps, with production features such as a high-performance cell-based renderer, color downsampling, declarative views, high-fidelity keyboard/mouse handling, and native clipboard support. [[S2]](https://github.com/charmbracelet/bubbletea)

- Repo: https://github.com/charmbracelet/bubbletea
- Architecture: The Elm Architecture (Model / Update / View)
- Companions: **Bubbles** (components) + **Lip Gloss** (styling)
- Ecosystem: 18,000+ dependent apps; used by Azure aztfy, CockroachDB, NVIDIA, MinIO mc, AWS tooling, Ubuntu, eks-node-viewer [[S6]](https://github.com/charmbracelet/bubbletea) [[S24]](https://github.com/charmbracelet/bubbletea)

### Ratatui (Rust)

**Ratatui** is a lightweight Rust crate for complex TUIs (forked from tui-rs in 2023). It uses immediate-mode rendering with intermediate buffers, constraint-based layouts, pure Rust with no C dependencies, and Crossterm as the default backend on Linux, macOS, and Windows. [[S3]](https://docs.rs/ratatui/latest/ratatui/)

- Docs: https://docs.rs/ratatui/latest/ratatui/ · Concepts: https://ratatui.rs/
- Architecture: immediate-mode; app owns render + input loops
- Widgets: Block, BarChart, Calendar, Canvas, Chart, Gauge, List, Paragraph, Scrollbar, Sparkline, Table, Tabs, etc.
- Ecosystem: ~22.1k GitHub stars, ~42.4M crates.io downloads, 5,200+ reverse-dependent crates; users include Netflix, OpenAI, AWS, Vercel [[S6]](https://github.com/charmbracelet/bubbletea)

### Ink (JS/TS)

**Ink** is a JavaScript/TypeScript library that renders React to interactive CLIs, using Yoga for Flexbox so CSS-like layout properties work in the terminal, and supporting all standard React features; it powers tools including Claude Code, Gemini CLI, Gatsby, Prisma, Shopify CLI, and Cloudflare Wrangler. [[S4]](https://github.com/vadimdemedes/ink)

- Repo: https://github.com/vadimdemedes/ink
- Layout: Yoga Flexbox + CSS-like props on React components

### FTXUI (C++)

**FTXUI** is a simple cross-platform C++ library (documented at v7.0.3) with a functional, React-inspired style, no external dependencies, keyboard and mouse navigation, UTF-8/fullwidth text, animations, and canvas drawing. It targets Linux, macOS, Windows, and WebAssembly, and ships as three modules (screen, dom, component) via CMake, Bazel, vcpkg, Conan, and distro packages. [[S5]](https://arthursonzogni.github.io/FTXUI/)

- Docs: https://arthursonzogni.github.io/FTXUI/

---

## Architecture models

| Model | Who uses it | Core idea |
|-------|-------------|-----------|
| **Elm Architecture (MVU)** | Bubble Tea | Model + `Init` / `Update` (messages → model + cmds) / `View` (UI from model) [[S7]](https://github.com/charmbracelet/bubbletea) [[S13]](https://github.com/charmbracelet/bubbletea) |
| **Immediate-mode** | Ratatui | Recreate UI each frame from app state; double-buffer + cell-level diff; app owns event loop [[S14]](https://ratatui.rs/concepts/rendering/) [[S15]](https://ratatui.rs/concepts/rendering/under-the-hood/) [[S16]](https://ratatui.rs/concepts/rendering/) |
| **Retained widget tree** | Textual | `compose()` yields widgets; events via `on_*` / `@on`; TCSS for layout/look; `run()` enters app mode [[S17]](https://textual.textualize.io/guide/app/) |

### Local reference: termbrowse

This workspace’s **termbrowse** TUI follows the common Ratatui + crossterm pattern: an `App` holds screen/focus/buffers; a loop calls `terminal.draw`, then polls/reads key events with a short (~80ms) timeout, on ratatui 0.30.2 and crossterm 0.29.0. See `src/tui_session.rs`, `src/theme.rs`, `src/layout.rs`. [[S18]]

---

## Layout design

### Constraint-based (Ratatui)

Splits the terminal horizontally or vertically with constraints—**Length, Percentage, Ratio, Min, Max, Fill**—and nests layouts for complex UIs; widgets fill the resulting sections. [[S8]](https://ratatui.rs/concepts/layout/)

### Outside-in / fractional (Textual)

Sketch the terminal, dock fixed chrome (header/footer), use `fr` fractional units for remaining space, and put scrollable content in containers such as `HorizontalScroll` and `VerticalScroll`. [[S9]](https://textual.textualize.io/how-to/design-a-layout/)

### Flexbox (Ink)

Yoga Flexbox with CSS-like properties on React components. [[S4]](https://github.com/vadimdemedes/ink)

---

## Styling, color, and theming

TUI styling commonly uses CSS-like block rules—borders, padding, margins, alignment, width/height—plus ANSI 16, 256-color, and truecolor, with automatic color downsampling and light/dark adaptive colors. [[S10]](https://github.com/charmbracelet/lipgloss)

| Stack | Styling approach |
|-------|------------------|
| **Go** | **Lip Gloss** — declarative styles, multi-profile colors, downsampling, `LightDark` / `HasDarkBackground`, Bubble Tea `BackgroundColorMsg` [[S10]](https://github.com/charmbracelet/lipgloss) [[S22]](https://github.com/charmbracelet/lipgloss) |
| **Python** | **Textual CSS** (`.tcss` via `CSS_PATH`), design variables (`$panel`, `$text`), live reload, state selectors (`:hover`, `:focus`, `:disabled`) [[S19]](https://textual.textualize.io/guide/CSS/) [[S12]](https://textual.textualize.io/guide/CSS/) |
| **Rust (termbrowse)** | Semantic theme slots: `bg`, `bg_panel`, `accent`, `text_dim`, `success`, `warn` in `src/theme.rs` [[S12]] |

**UX practice:** state-driven styling + semantic tokens (panels, text, success/error) so look stays separate from logic.

---

## Interaction conventions

Keyboard-first is the baseline:

- Dual **arrow / vim** keys for movement
- **Enter / space** for selection
- **`q` or Ctrl+C** to quit
- Reusable help, lists, viewports, inputs
- Bubbles keybindings often document pairs like `↑/k` [[S11]](https://github.com/charmbracelet/bubbles)

Mouse exists in some toolkits (viewport wheel, BubbleZone, Textual `:hover`) but is secondary to keyboard-first design.

---

## Widgets and companion libraries

### Bubble Tea stack

Paired with **Bubbles**: spinner, textinput, textarea, table, progress, paginator, viewport, list, filepicker, timer, stopwatch, help, key-binding helpers — plus **Lip Gloss** for styling/layout. [[S21]](https://github.com/charmbracelet/bubbles)

### Ratatui built-ins

Block, BarChart, Calendar, Canvas, Chart, Gauge, List, Paragraph, Scrollbar, Sparkline, Table, Tabs, and more — plus `Widget` / `StatefulWidget` (and optional `WidgetRef`) so apps compose custom components and keep scroll/selection state outside pure render structs. [[S23]](https://ratatui.rs/concepts/widgets/)

---

## Testing, debugging, and production ops

| Framework | Practice |
|-----------|----------|
| **Textual** | Headless `App.run_test()` + Pilot API (press/click/pause, size); visual regressions via pytest-textual-snapshot (SVG, `--snapshot-update`) [[S20]](https://textual.textualize.io/guide/testing/) |
| **Bubble Tea** | Headless Delve (stdin/stdout owned by TUI); file logging via `tea.LogToFile` [[S24]](https://github.com/charmbracelet/bubbletea) |
| **General** | Log and debug **off-stdout** when the TUI owns the terminal |

---

## How to choose

| If you want… | Pick |
|--------------|------|
| Python, retained widgets + CSS, browser option, strong testing | **Textual** |
| Go, Elm Architecture, rich companion UI/styling stack | **Bubble Tea + Bubbles + Lip Gloss** |
| Rust, immediate-mode, full control of loops, rich built-in widgets | **Ratatui (+ crossterm)** |
| JS/TS and React/Flexbox mental model | **Ink** |
| C++ / WebAssembly, no deps, React-inspired functional UI | **FTXUI** |

### Shared design playbook

1. Model state explicitly  
2. Redraw or recompose from that state  
3. Use constraint or Flex/`fr` layouts  
4. Prefer semantic colors and adaptive theming  
5. Keep navigation keyboard-first with standard quit/select keys  
6. Log and debug off-stdout when the TUI owns the terminal  

---

## Quick link map

| Resource | URL |
|----------|-----|
| Textual | https://textual.textualize.io/ |
| Textual layout how-to | https://textual.textualize.io/how-to/design-a-layout/ |
| Textual CSS | https://textual.textualize.io/guide/CSS/ |
| Textual testing | https://textual.textualize.io/guide/testing/ |
| Bubble Tea | https://github.com/charmbracelet/bubbletea |
| Bubbles | https://github.com/charmbracelet/bubbles |
| Lip Gloss | https://github.com/charmbracelet/lipgloss |
| Ratatui docs.rs | https://docs.rs/ratatui/latest/ratatui/ |
| Ratatui layout | https://ratatui.rs/concepts/layout/ |
| Ratatui rendering | https://ratatui.rs/concepts/rendering/ |
| Ratatui widgets | https://ratatui.rs/concepts/widgets/ |
| Ink | https://github.com/vadimdemedes/ink |
| FTXUI | https://arthursonzogni.github.io/FTXUI/ |

---

## Sources

- [S1] Textual — Welcome / What is Textual? — https://textual.textualize.io/
- [S2] charmbracelet/bubbletea — https://github.com/charmbracelet/bubbletea
- [S3] Crate ratatui — https://docs.rs/ratatui/latest/ratatui/
- [S4] vadimdemedes/ink — https://github.com/vadimdemedes/ink
- [S5] FTXUI: Introduction — https://arthursonzogni.github.io/FTXUI/
- [S6] Bubble Tea README / Ratatui homepage metrics — https://github.com/charmbracelet/bubbletea
- [S7] [S13] Bubble Tea README (Elm Architecture) — https://github.com/charmbracelet/bubbletea
- [S8] Layout \| Ratatui — https://ratatui.rs/concepts/layout/
- [S9] Design a Layout - Textual — https://textual.textualize.io/how-to/design-a-layout/
- [S10] charmbracelet/lipgloss README — https://github.com/charmbracelet/lipgloss
- [S11] Bubble Tea tutorial; charmbracelet/bubbles README — https://github.com/charmbracelet/bubbles
- [S12] Textual CSS guide; local theme.rs — https://textual.textualize.io/guide/CSS/
- [S14] [S16] Ratatui — Rendering — https://ratatui.rs/concepts/rendering/
- [S15] Ratatui — Rendering under the hood — https://ratatui.rs/concepts/rendering/under-the-hood/
- [S17] Textual — App Basics — https://textual.textualize.io/guide/app/
- [S18] termbrowse `tui_session.rs` / Cargo.toml — local workspace
- [S19] Textual CSS — https://textual.textualize.io/guide/CSS/
- [S20] Testing - Textual — https://textual.textualize.io/guide/testing/
- [S21] charmbracelet/bubbles — https://github.com/charmbracelet/bubbles
- [S22] charmbracelet/lipgloss — https://github.com/charmbracelet/lipgloss
- [S23] Introduction to Widgets \| Ratatui — https://ratatui.rs/concepts/widgets/
- [S24] charmbracelet/bubbletea production notes — https://github.com/charmbracelet/bubbletea

---

## Coverage and uncertainty

This research is **partial**. Known gaps:

- **Not exhaustive:** other libraries (ncurses/curses, Python urwid/prompt_toolkit, Go tview/rivo, Node blessed, .NET Consolonia, OpenTUI) were not inspected via primary sources and are omitted rather than summarized from secondary lists.
- **Metrics:** star counts, download totals, and dependent-app numbers are self-reported or snapshot metrics; methodology for “18,000 applications” was not independently verified.
- **Ranking:** “major” is based on widely documented multi-ecosystem libraries with official primary docs, not a single authoritative survey.
- **Maturity:** API stability, release cadence, and long-term maintenance beyond official site claims were not assessed.
- **Platforms:** Windows quirks, WASM completeness for FTXUI, etc. may vary beyond high-level platform lists.
- **No single design system:** conventions are inferred from major frameworks, not a formal UX standard body.
- **Accessibility:** guidance (don’t rely only on color, prefer user terminal palette) appears mainly in secondary discussion; primary docs emphasize adaptive/downsampled colors more than WCAG-style rules.
- **Architecture variety:** Elm MVU, immediate-mode, and retained widget/CSS coexist; “typical” depends on language/framework.
- **Testing:** Ink’s full testing docs only partially captured; termbrowse has a hand-rolled theme but no shared community testing harness documented here.
- **Production practices:** CI across terminals, accessibility, packaging are fragmented across ecosystems.

### Possible follow-ups

1. Deep-dive the omitted stacks (prompt_toolkit, tview, blessed, OpenTUI, Consolonia).
2. Ratatui-specific playbook mapped onto termbrowse (`layout.rs`, `theme.rs`, widgets, testing).
3. Side-by-side starter templates (hello-world + list + form) per framework.
4. Accessibility and color-contrast checklist for TUIs.
