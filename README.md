# termbrowse

**Interactive web sessions for the terminal** — not a scraper dump, not Chrome-in-a-box.

Structure-first browsing for developers and people who live in the shell. Performance over pixels. Same document model for humans and agents.

```
URL → structure (HTML → blocks + link refs)
        ↓ if thin JS shell
      escalate (Chrome extract → same blocks)
        ↓
      session TUI  ·  agent JSON snapshot
```

## Position

| | Scrapers | termbrowse | Full browser |
|--|----------|------------|--------------|
| Model | One-shot extract | **Live session** | GUI / pixels |
| Interaction | None | links, history, reload | Everything |
| Speed | Fast | **Fast by default** | Heavy |
| Output | Data | **UI + data** | Screen |

**Promise:** Browse the web where you already work — navigate for real, stay fast, zero guilt about not looking like Chrome.

## Quick start

```bash
cargo run --release -- https://example.com
cargo run --release -- https://doc.rust-lang.org/book/

# Never use Chrome (structure only)
cargo run --release -- --structure-only https://example.com

# Agent snapshot (stderr: source=Structure|Escalated)
cargo run --release -- snapshot https://example.com

# Legacy pixel paint (Kitty/CRT) — optional, not the product default
cargo run --release -- --pixels https://example.com
```

### Keys

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll |
| `Tab` / `n` | Next link |
| `Enter` | Follow link |
| `[` / `]` | History back / forward |
| `o` or `:` | Open URL |
| `r` | Reload |
| `q` | Quit |

Title bar shows **structure** (fast path) or **escalated** (Chrome extract).

## Architecture

| Module | Role |
|--------|------|
| `session` | History + load path (structure → escalate) |
| `parse` / `layout` | HTML → blocks → terminal cells |
| `theme` / `tui_session` | Grok-density UI (accent rails, magenta) |
| `chrome` | Optional extract only — not the face |
| `snapshot` | Agent JSON of the same Document |

## Compatibility (v0)

**In:** docs, blogs, marketing pages, many static/dev sites.  
**Escalate:** empty JS shells → structured text + links (still not pixel YouTube).  
**Out:** pixel-perfect GUI web as the default experience.

## License

MIT
