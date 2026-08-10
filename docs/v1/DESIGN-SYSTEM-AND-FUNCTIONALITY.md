# termbrowse v1 — Design System & Functionality Spec

**Status:** Draft for v1 freeze  
**Audience:** Product + engineering (single source of truth)  
**Stack:** Custom Rust pipeline + ratatui (no headless Chrome)  
**Repo:** termbrowse  

This document captures every design and product answer needed to make **v1 good** — positioning, rendering philosophy, UI system, features in/out, and open questions that still need a decision before calling v1 done.

---

## 1. Product definition

### 1.1 One-line position

> **termbrowse is an interactive web *session* for the terminal** — search, open, navigate, and read — with CLI-grade UX. Not a scraper dump, not Chrome in a box.

### 1.2 Who it’s for

| Primary | Secondary (later) |
|---------|-------------------|
| Developers and people who live in the terminal (SSH, tmux, Kitty, coding agents) | Agent platforms that need structured page access |
| Reading docs, blogs, SEARCH results, GitHub-ish HTML, Wikipedia, HN | Full SPA web apps |

### 1.3 Who it’s not for

- Designers checking pixel CSS  
- “Watch YouTube in the terminal”  
- Solving CAPTCHAs / pretending to be a full browser  

### 1.4 Success for v1 (must all be true)

1. User can launch **Start Page**, open a favorite, read a real article.  
2. User can **search (DuckDuckGo only)**, pick a result, land on a rendered page.  
3. Session feels interactive: **history, links, home, favorites, reading list**.  
4. Cold path is **fast** (structure fetch, no browser engine).  
5. Failures are **designed** (CAPTCHA, sparse JS shell), not random broken HTML.  
6. Agent can get the **same document** as JSON (`snapshot`).

### 1.5 Anti-goals for v1

| Do not build in v1 |
|--------------------|
| Headless Chromium / Playwright as default |
| Pixel/screenshot / Kitty-image page paint as primary UI |
| Full CSS engine (flex, grid, cascade) |
| JavaScript engine |
| Multi-search-engine picker |
| Accounts, sync, cloud bookmarks |
| Monetization / SaaS |

---

## 2. Core principles (non-negotiable)

1. **Structure first, pixels never (by default).**  
2. **Interaction over fidelity.** Click, back, type > looks like Chrome.  
3. **Fast means refuse work.** Every feature justifies its cost.  
4. **One model for human and agent.** TUI and `snapshot` share `Document`.  
5. **Render by role, not screenshot.** How a browser *treats* a node, not how it paints it.  
6. **Minimal chrome, intentional chrome.** Borders only when the role is bordered.  
7. **DuckDuckGo is the only search engine.** No Google/Bing as product paths.  
8. **Terminal-native aesthetics.** Grok-density: dark base, magenta accent, dense hierarchy.

---

## 3. Architecture (v1 system)

### 3.1 Pipeline

```
URL
  → fetch (HTTPS, browser-like headers)
  → normalize (unwrap redirects; rewrite captcha search hosts → DDG)
  → parse (HTML → roles → Document)
  → layout (roles → terminal lines/segments)
  → tui_session (ratatui screens + keys)
       ↘ snapshot (JSON of same Document)
```

### 3.2 Modules (ownership)

| Module | Responsibility |
|--------|----------------|
| `fetch` | HTTPS GET/POST; headers; timeouts |
| `urlutil` | ensure URL; unwrap DDG `uddg=` redirects |
| `parse` | HTML → blocks/spans/links; DDG form attach |
| `model` | Document, Block, Span, Link, SearchForm, forms |
| `layout` | wrap, boxes, tables, link labels `[eN]` |
| `session` | history stack; open/reload/follow |
| `home` | favorites + reading list JSON persist |
| `theme` | GrokNight color tokens |
| `tui_session` | screens: Home / Browse / Search / modals |
| `snapshot` | agent JSON |
| `search` | PrivSearch CLI path (optional; may lag TUI v1) |

### 3.3 Explicit non-layers

| Layer | v1 status |
|-------|-----------|
| CSS engine | Out (tiny border/class heuristics only) |
| JS engine | Out |
| Screenshot compositor | Out |
| Chrome escalate | Out |

---

## 4. Rendering philosophy (translation layer)

### 4.1 What “translation” means

termbrowse does **not** convert HTML+CSS+JS into ASCII art of the page.

It does:

1. **Strip** noise (`script`, `style`, `svg`, `iframe`, …)  
2. **Classify** useful nodes into **roles**  
3. **Lay out** into monospaced lines (terminal columns, not CSS pixels)  
4. **Paint** with ratatui styles + keyboard behavior  

```
Browser:     tags → style → boxes → pixels
termbrowse:  tags → roles  → lines → cells
```

### 4.2 Role → terminal treatment (v1 map)

| Role | Source hints | Terminal treatment |
|------|--------------|--------------------|
| Heading | `h1`–`h6` | `#` / `##` / bold hierarchy |
| Paragraph | `p`, loose text | Wrapped body text |
| Strong / Em / Code | `strong`/`b`, `em`/`i`, `code` | Bold / italic / green mono |
| Link | `a[href]` | Accent + underline + ref `[eN]`; Enter opens |
| List item | `li` in `ul`/`ol` | `•` or `1.` |
| Quote | `blockquote` | Left bar `│` |
| Pre / code block | `pre` | **Unicode box** around mono text |
| Table | `table` | Unicode grid with borders |
| Frame / card | `fieldset`, `figure`, class/style border hints | **Bordered frame** + optional title |
| Image | `img` | `[ img: alt ]` placeholder (no pixels) |
| HR | `hr` | `────` |
| Nav / footer / aside | landmarks | Prefer skip (reader bias) |
| Script / style / svg / iframe | noise | Drop |

### 4.3 Borders rule (minimal but intentional)

- **Show borders** when the role is inherently bordered: `pre`, `table`, `fieldset`, framed cards.  
- **Do not** put a box on every `div`.  
- Class/style heuristics (`card`, `panel`, `border`, …) are allowed as **hints**, not full CSS.

### 4.4 What v1 rendering will *not* do

| Out of scope |
|--------------|
| Flex/grid/absolute positioning |
| Site fonts, shadows, animations |
| Running page JS |
| 1:1 visual clone of Chrome |
| Full image/video paint as primary UX |

### 4.5 Tier-2 upgrades (post-minimal, still no Chrome)

Documented so v1 doesn’t pretend to be done-forever:

1. **Readability** main-content extract (drop chrome aggressively)  
2. **Markdown IR** + denser prose renderer (glow-class hierarchy)  
3. **SERP result list widget** (title / url / snippet rows)  
4. Explicit screens: Home / Search / Results / Reader  
5. Loading / error / empty as designed states  

---

## 5. Design system (visual)

### 5.1 Theme: GrokNight-inspired

| Token | Role | Approx RGB |
|-------|------|------------|
| `bg` | Base canvas | `#0d0d10` |
| `bg_panel` | Panels, tiles, modals | `#18181e` |
| `accent` | Magenta brand / focus | `#e879f9` |
| `accent_dim` | Help text, inactive | muted purple |
| `text` | Primary | near-white |
| `text_dim` | Secondary / meta | gray |
| `heading` | Titles | bright |
| `link` | Links | light purple |
| `link_active` | Selected link | yellow |
| `code` | Code | green |
| `border` | Boxes / tiles | dark gray |
| `success` | Status badges | green |

**Rules:**

- Never hardcode colors outside `theme.rs`.  
- Accent rail `▎` on content rows (Grok-density).  
- Selected items: panel bg + bright foreground.  
- Help line always bottom; never compete with content.

### 5.2 Typography hierarchy (terminal)

| Level | Treatment |
|-------|-----------|
| App chrome title | accent wordmark + dim meta |
| Page H1 | bold, more space |
| H2/H3 | bold, slightly less weight |
| Body | normal wrap |
| Meta / URL | dim |
| Code | mono color |

### 5.3 Spacing

- Outer content pad: ~2 columns  
- Section gap: 1 blank line  
- Tile gap: 1 column  
- Modal: ~70% width, centered  
- Search box: ~60–62% width, vertically centered on search home  

### 5.4 Motion (v1)

- Optional: none required  
- Allowed: simple loading status string while fetch runs  
- Disallowed: CRT scanlines as primary product identity (legacy only if revived)

---

## 6. Screens & navigation model

### 6.1 Screens

| Screen | Purpose |
|--------|---------|
| **Start Page (Home)** | Safari-like favorites + reading list |
| **Browse** | Rendered page content + optional search strip |
| **Search home (centered)** | DuckDuckGo-oriented prompt in the middle |
| **Edit modal** | Add/edit favorite or reading item |

### 6.2 Session

- History stack with back/forward  
- Back with empty history → Start Page  
- `Esc` / `H` → Start Page from browse  
- Same session for human and agent-facing document model  

### 6.3 Default launch

```text
termbrowse          → Start Page
termbrowse <url>    → open URL in Browse
termbrowse home     → Start Page
termbrowse search   → PrivSearch CLI (optional path)
```

---

## 7. Functionality — Start Page (Safari-like)

### 7.1 Sections

1. **Favorites** — tile grid (letter glyph + title)  
2. **Reading List** — vertical list (title + URL)  

### 7.2 Persistence

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/termbrowse/home.json` |
| Linux | `~/.config/termbrowse/home.json` |
| Fallback | `./termbrowse-home.json` |

Schema sketch:

```json
{
  "favorites": [{ "title": "…", "url": "…" }],
  "reading_list": [{ "title": "…", "url": "…", "saved_at": 0 }]
}
```

### 7.3 Default favorites (CAPTCHA-safe)

- Search → DuckDuckGo HTML  
- Rust Book, MDN, Hacker News, Wikipedia, Example  

No Google as a default favorite.

### 7.4 Start Page keys

| Key | Action |
|-----|--------|
| arrows / hjkl | Move selection |
| Tab | Favorites ↔ Reading List |
| Enter | Open selected |
| a | Add |
| e | Edit |
| d / Delete | Delete |
| / | Open DuckDuckGo search |
| o / : | Quick add/open URL flow |
| q | Quit |

### 7.5 From Browse → library

| Key | Action |
|-----|--------|
| f | Add current page to Favorites |
| s | Save current page to Reading List |
| H / Esc | Start Page |

### 7.6 Edit modal

- Fields: Title, URL  
- Tab switches field  
- Enter saves; Esc cancels  
- URL normalized with `https://` if missing  

---

## 8. Functionality — Search

### 8.1 Only engine

**DuckDuckGo HTML** — `https://html.duckduckgo.com/html/`

| In | Out |
|----|-----|
| DDG HTML only | Google, Bing, multi-engine picker |
| Query `q=` | Headless Chrome for SERPs |

### 8.2 Behaviors

- Product `search_url` / submit always builds DDG URL.  
- Opening `google.com` / Google search URLs rewrites to DDG.  
- Result links unwrap `uddg=` (and similar) to **real destination URLs**.  
- Clicking a result loads that URL through the normal structure pipeline.

### 8.3 Search UI

| Context | UI |
|---------|-----|
| DDG home / empty query | Centered Grok-style search box |
| Results page | Content list + optional compact search strip on `/` |
| CAPTCHA wall | Designed copy + DDG form still available |

### 8.4 Search keys

| Key | Action |
|-----|--------|
| / or i | Focus DuckDuckGo search (open DDG if needed) |
| type | Query |
| Enter | Submit |
| Tab | Leave search → content / next link |

### 8.5 CAPTCHA reality (v1 answer)

- CAPTCHA is **site policy**, not a termbrowse bug.  
- We do **not** solve CAPTCHAs.  
- We **avoid** paths that cause them (no Chrome; no Google product path).  
- Designed CAPTCHA screen explains next steps (DDG, Home favorites, docs).  

---

## 9. Functionality — Browse / render any page

### 9.1 Open any URL

- `termbrowse <url>` or Start Page / Reading List / Favorites / link Enter  
- Relative links resolve against current page base  
- Redirect wrappers unwrapped before load  

### 9.2 Page interaction

| Key | Action |
|-----|--------|
| j/k · arrows | Scroll |
| Tab / n · p | Next / prev link |
| Enter | Open selected link |
| [ ] | History back / forward |
| r | Reload |
| o / : | Open URL |
| f | Favorite |
| s | Reading list |
| H / Esc | Home |
| q | Quit |

### 9.3 Sparse JS shells

If HTML has almost no content (SPA shell):

- Show a **designed sparse-page state**  
- Do **not** launch a browser engine  
- Suggest working destinations (DDG, docs)  

### 9.4 Agent surface

```bash
termbrowse snapshot <url>
```

- Same `Document` as TUI  
- Includes blocks, links, forms, timing  
- stderr may log source/timing  

---

## 10. Data model (shared)

### 10.1 Document

- `url`, `title`  
- `blocks[]` — role-based content  
- `links[]` — `{ ref, href, text }`  
- `forms[]` — always includes DDG search form for product search  
- `timing_ms`  

### 10.2 Blocks (v1)

Heading, Paragraph, ListItem, Pre, Quote, Hr, Spacer, Image, Table, Frame, Caption  

### 10.3 Spans

Text, Strong, Em, Code, Link  

### 10.4 Stability rule

Changing roles must update: parse → layout → theme → snapshot → tests together.

---

## 11. Network & reliability

### 11.1 Fetch

- HTTPS via reqwest + rustls  
- Browser-like User-Agent and Accept headers  
- Redirects limited  
- Timeout ~30s  

### 11.2 Failure modes (must be designed)

| Case | UX |
|------|----|
| DNS / network error | Status error, stay on previous screen if possible |
| HTTP 4xx/5xx | Error status with code |
| CAPTCHA HTML | CAPTCHA screen |
| Empty SPA | Sparse page screen |
| Slow load | Status “loading…” |

---

## 12. Tech stack decisions

| Choice | Decision | Why |
|--------|----------|-----|
| Language | Rust | Performance, single binary |
| TUI | **ratatui** + crossterm | Advanced CLI baseline; Grok-like density possible |
| HTML parse | scraper / html5ever | Fast structure extract |
| Browser engine | **None** | Product law; CAPTCHA + complexity |
| Search | DDG HTML only | Terminal-realistic |
| Persist | Local JSON | No backend for v1 |

### 12.1 Optional crates for “advanced CLI” feel (v1.x candidates)

| Crate | Use |
|-------|-----|
| tui-textarea / tui-input | Better search/edit inputs |
| throbber-widgets-tui | Loading states |
| tuirealm | Screen/message architecture if complexity grows |

Do **not** switch to Textual/Charm for v1 (rewrite cost > benefit).

---

## 13. v1 feature freeze checklist

### Must ship

- [x] Structure pipeline (fetch → parse → layout → TUI)  
- [x] Role-based minimal rendering + borders where bordered  
- [x] DuckDuckGo-only search  
- [x] Link open + redirect unwrap  
- [x] Session history  
- [x] Start Page: favorites + reading list, editable, persisted  
- [x] CAPTCHA / sparse designed states  
- [x] snapshot for agents  
- [ ] Readability-quality main content (tier-2; decide if v1 gate)  
- [ ] SERP as first-class list widget (tier-2; decide if v1 gate)  
- [ ] Polished loading indicator  

### Explicitly later

- Forms beyond search (POST logins)  
- Cookies / auth sessions  
- Multiple engines  
- Image protocol previews  
- Sync / multi-device  
- PrivSearch ranking product (see `docs/PRIVSEARCH.md`) as separate track  

---

## 14. Open questions (need answers before calling v1 “done”)

Answer these with **yes/no or a single choice**. Defaults in parentheses are recommendations.

### Product

| # | Question | Options | Recommended |
|---|----------|---------|-------------|
| P1 | Is **Readability extract** a v1 gate or v1.1? | gate / later | **later (v1.1)** |
| P2 | Is **SERP list UI** a v1 gate or v1.1? | gate / later | **later (v1.1)** if search works as content scroll |
| P3 | Default launch always Start Page? | always / only if no URL | **always when no URL** (done) |
| P4 | Should `/` from any page always jump to DDG? | yes / only if form | **yes** (done) |
| P5 | Favorites seed list final? | keep / edit | **keep CAPTCHA-safe list** |
| P6 | Is PrivSearch CLI part of termbrowse v1 marketing? | in / out | **out of core story** (parked doc) |

### Rendering

| # | Question | Options | Recommended |
|---|----------|---------|-------------|
| R1 | How aggressive is nav/footer stripping? | soft / hard | **hard in article mode** (needs readability) |
| R2 | Max table size before collapse? | e.g. 20×10 | **20 rows × 8 cols, then truncate** |
| R3 | Images: alt only, or optional Kitty later? | alt only / optional | **alt only in v1** |
| R4 | CSS `display:none`? | ignore / best-effort | **best-effort simple** if easy; else ignore |

### Interaction

| # | Question | Options | Recommended |
|---|----------|---------|-------------|
| I1 | Mouse support in v1? | yes / no | **no** |
| I2 | Multi-line search input? | single / multi | **single** |
| I3 | Number keys 1–9 jump to links? | yes / no | **yes** (done) |
| I4 | Confirm before delete favorite? | yes / no | **no** (fast CLI) |

### Reliability

| # | Question | Options | Recommended |
|---|----------|---------|-------------|
| N1 | Retry fetch on failure? | 0 / 1 / 2 | **1 retry** |
| N2 | Timeout seconds? | 15 / 30 / 60 | **30** |
| N3 | Custom User-Agent string? | browser-like / termbrowse | **browser-like** |

### Packaging

| # | Question | Options | Recommended |
|---|----------|---------|-------------|
| K1 | Install story for v1? | cargo only / brew later | **cargo install / binary** |
| K2 | Config path freeze? | as implemented | **yes** |

---

## 15. Acceptance scenarios (v1)

Write these as manual QA:

1. **Cold start** → Start Page shows favorites tiles + empty/filled reading list.  
2. **Add/edit/delete favorite** → persists after restart.  
3. **Open Rust Book favorite** → readable headings + links; Enter follows a link.  
4. **Search** → type query on DDG → results → Enter on a result → destination page renders.  
5. **DDG redirect unwrap** → result link is real site, not duckduckgo.com/l/?uddg=…  
6. **History** → back returns previous page; back from first page returns Home.  
7. **Reading list** → on a page press `s`; Home shows item; open works.  
8. **Google URL** → rewritten or blocked with designed CAPTCHA/DDG path, not a crash.  
9. **snapshot** → JSON contains title, blocks, links for example.com.  
10. **Sparse SPA** → designed message, not panic.

---

## 16. Glossary

| Term | Meaning |
|------|---------|
| **Role** | How we classify a node (heading, link, frame…) |
| **Document** | Canonical structured page model |
| **Session** | History + current page |
| **Structure path** | fetch+parse without a browser engine |
| **SERP** | Search engine results page |
| **IR** | Intermediate representation (future markdown-ish layer) |
| **Grok density** | Dark, magenta accent, tight hierarchy UI language |

---

## 17. Decision log (locked so far)

| Decision | Choice | Date / context |
|----------|--------|----------------|
| Engine | No headless Chrome | CAPTCHA + product law |
| Search | DuckDuckGo HTML only | Terminal reality |
| UI toolkit | ratatui | Advanced CLI baseline |
| Rendering | Role-based minimal | Not pixel clone |
| Home | Safari-like favorites + reading list | v1 |
| Persist | Local JSON | v1 |
| Agents | Same Document snapshot | v1 |

---

## 18. How to use this doc

1. **Before building a feature:** check §2 principles and §13 freeze.  
2. **Before changing look:** update §5 tokens and the role map in §4.  
3. **Before calling v1 done:** answer §14 open questions and pass §15 scenarios.  
4. **PrivSearch** (ranked search product) lives in `docs/PRIVSEARCH.md` — separate track unless P6 says otherwise.

---

## 19. Recommended v1.0 cut line

**Ship when:**

- Start Page + library CRUD + browse + DDG search + link follow + history + snapshot work end-to-end on a laptop.  
- CAPTCHA/sparse states are designed.  
- Role map in §4 is implemented and tested for core tags.  
- §14 answers filled (even if “defer”).  

**Defer to v1.1:**

- Readability extract  
- SERP list widget  
- Advanced input widgets / mouse  
- `display:none` CSS lite  

---

*End of v1 design system & functionality answers. Update this file when a decision changes — don’t fork silent tribal knowledge.*
