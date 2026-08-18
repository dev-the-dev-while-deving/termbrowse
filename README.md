# termbrowse (from-scratch)

A **terminal web session** — search, open, read, save — without a browser engine.

HTML becomes a structured document, then a **256-color** ratatui layout that reflows with the terminal. Each site can steal a link / heading / accent color onto a dark canvas. No page JavaScript. No Chrome.

```
URL → fetch → roles + identity → layout(width) → terminal
```

## Install

macOS (Apple Silicon or Intel) and Linux (x86_64 or ARM):

```bash
curl -fsSL https://raw.githubusercontent.com/dev-the-dev-while-deving/termbrowse/from-scratch/install.sh | sh
```

That puts `browse` in `~/.local/bin`. If your shell cannot find it:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

```bash
browse                  # Start Page
browse https://example.com
browse update           # latest GitHub Release
```

The binary is unsigned. `browse` checks GitHub at most once per day for a newer version and never sends page or search data.

## Run from source

```bash
cargo run --release
cargo run --release -- https://example.com
cargo run --release -- snapshot https://example.com
cargo run --release -- text https://example.com
```

## Keys (keyboard-first)

Vim-style. Mouse is optional. Press `?` in the app for the full card.

| | |
|---|---|
| Scroll | `j` `k` · `C-d` `C-u` · `gg` `G` |
| Links | `f` hints · `F` hint in new tab · `tab`/`n` next · `enter` open |
| History | `H` back · `L` forward · `gh` home · `r` reload · `yy` copy url |
| Tabs | `t` new · `x` close · `gt` / `gT` next / prev |
| Search | `o` address · `gi` or `/` this site · `C-s` find in page |
| Emacs | `C-n` `C-p` · `C-v` `M-v` · `C-g` cancel |
| Meta | `?` help · `esc` cancel · `q` quit |

## What this is not

Not Chrome. Not a pixel clone. Not a CAPTCHA solver. Docs, articles, and public HTML are the point.

## Strategy

See `docs/v1/` and `docs/superpowers/specs/2026-08-14-structure-identity-engine.md`.
