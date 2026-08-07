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
| `parse` | HTML → **roles** (heading, link, pre, table, frame, image, …) |
| `layout` | Role → cells; **borders only when the role is bordered** |
| `session` | History, navigation, load |
| `theme` / `tui_session` | Grok-density + centered search |
| `snapshot` | Same document for agents |

**Not used:** Chromium, Playwright, pixel CRT, Kitty image protocol.

### Minimal role rendering

Not a 1:1 browser paint — same *kinds* of elements, terminal treatment:

| Role | Terminal |
|------|----------|
| Heading | `#` / bold |
| Paragraph | wrapped text |
| Strong / em / code | bold / italic / green |
| Link | accent + `[eN]` |
| List | `•` or `1.` |
| Quote | `│` bar |
| Pre / code block | **box border** |
| Table | unicode grid |
| Fieldset / card / border | **frame box** |
| Image | `[ img: alt ]` |
| HR | `────` |

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

## Contributing

Contributions are welcome!

1. Fork & clone the repository.
2. Build and check locally:
   ```bash
   cargo check
   cargo run --release -- https://example.com
   ```
3. Create a pull request or push your commits.

## License

MIT

