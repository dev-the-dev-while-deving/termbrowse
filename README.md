# termbrowse

**Custom interactive web for the terminal** — no headless Chrome, no screenshot paint.

```
URL → HTTPS fetch → HTML parse → blocks + forms + links → Grok-density TUI
                                                      ↘ agent JSON snapshot
```

A **session** (history, click links, type search), not a one-shot scraper dump and not a graphical browser.

## Stack (all custom)

| Layer | What it does |
|-------|----------------|
| `fetch` | HTTPS only |
| `parse` | HTML → headings, paragraphs, lists, links, search forms |
| `layout` | Terminal cell wrap |
| `session` | History, navigation, load |
| `theme` / `tui_session` | Grok-style accent rails + **centered search** |
| `snapshot` | Same document for agents |

**Not used:** Chromium, Playwright, pixel CRT, Kitty image protocol.

## Quick start

```bash
cargo run --release -- https://example.com
cargo run --release -- https://doc.rust-lang.org/book/
cargo run --release -- https://html.duckduckgo.com/html/

# Agent
cargo run --release -- snapshot https://example.com
```

### Keys

| Key | Action |
|-----|--------|
| *type* | On search homes: centered search box |
| `Enter` | Submit search / open link |
| `/` or `i` | Focus search |
| `Tab` | Search ↔ content / next link |
| `j` / `k` | Scroll |
| `[` / `]` | History |
| `o` or `:` | Open URL |
| `q` | Quit |

## Search & CAPTCHA

Google often blocks non-browser clients. We use **basic HTML** (`gbv=1`) only — never a real browser engine. If Google still CAPTCHAs your IP, the UI explains it and offers DuckDuckGo HTML:

```bash
./target/release/termbrowse https://html.duckduckgo.com/html/
```

## Position

| | Scrapers | **termbrowse** | Full browser |
|--|----------|----------------|--------------|
| Model | One-shot extract | **Live session** | GUI / JS engine |
| Engine | HTML parse | **Custom structure** | Chromium/WebKit |
| Speed | Fast | **Fast by default** | Heavy |

## License

MIT
