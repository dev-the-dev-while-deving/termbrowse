# termbrowse

Terminal browser. Capture is **720p (1280×720)**. Display prefers **Kitty graphics** (real pixels).

| Mode | Flag | What it does |
|------|------|----------------|
| **full** (default) | — | Chrome @ 720p → Kitty/iTerm/Sixel graphics (real pixels). CRT halfblocks if no protocol. |
| **lite** | `--lite` | HTML-only document browser (no JS). |

## Resolution model

| Layer | Size / path |
|-------|-------------|
| Chrome viewport | **1280×720** (true 720p) |
| **In Kitty** | Graphics protocol — real pixels scaled to the pane |
| Fallback TTY | CRT half-blocks + pan |

### Run inside Kitty (recommended)

```bash
open -a kitty
# then:
cd /Users/devarsheejmude/Projects/trial
./target/release/termbrowse "https://www.youtube.com/results?search_query=cats"
```

Title bar should say `720p/Kitty graphics`. If it says `CRT halfblocks`, you’re not in Kitty.

## Quick start

```bash
# CRT green (default phosphor)
cargo run --release -- "https://www.youtube.com/results?search_query=cats"

# Other phosphors: color | green | amber | mono
cargo run --release -- --phosphor amber "https://example.com"

# Lite HTML-only
cargo run --release -- --lite https://example.com
```

### Keys (full / CRT mode)

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll real page (re-scan) |
| `s` | Rescan current frame line-by-line |
| `f` | Skip scan — show full frame now |
| `c` | Cycle phosphor (color → green → amber → mono) |
| `l` | Links panel |
| `Enter` | Open selected link |
| `r` | Reload |
| `q` | Quit |

Requires **Google Chrome**.

## License

MIT (or whatever you prefer later).
