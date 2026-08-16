# termbrowse — curl install + `browse update`

**Status:** Locked for implementation  
**Date:** 2026-08-17  
**Branch:** `from-scratch`  
**Follows:** `docs/superpowers/specs/2026-08-14-structure-identity-engine.md`

## Intent

A stranger on macOS or Linux can install a prebuilt binary with one curl, then type `browse` and use the product. When we cut a new GitHub Release, existing installs see a Start Page notice and upgrade with `browse update`.

No Rust toolchain required. No sudo. No telemetry from the TUI.

## Locked decisions

| Decision | Choice |
|----------|--------|
| Install mechanism | Prebuilt binary via `curl …/install.sh \| sh` |
| Public command | `browse` (crate name stays `termbrowse`) |
| Platforms | macOS aarch64 + x86_64; Linux aarch64 + x86_64 (musl) |
| Host | GitHub Releases on `dev-the-dev-while-deving/termbrowse` |
| Install path | `$HOME/.local/bin/browse` (override: `BROWSE_INSTALL_DIR`) |
| Update UX | `browse update` + Start Page notice; nothing downloads until they update |
| Update check | GitHub API, cached 24h, never blocks first paint, fail silent |
| Checksum | SHA-256 of each tarball; refuse on mismatch |
| Linux libc | musl, statically linked |
| Windows / brew / notarization / sudo `/usr/local` / auto-download | Out |

Parked for a later spec (not this work): marketing site, Vercel, Supabase download counts, installer ping.

## Public contract

```bash
curl -fsSL https://raw.githubusercontent.com/dev-the-dev-while-deving/termbrowse/from-scratch/install.sh | sh
browse
browse update
```

`browse` with no args opens the Start Page (same as today).

If `~/.local/bin` is not on `PATH`, the installer still succeeds and prints:

```text
Add this to your shell profile, then open a new terminal:
  export PATH="$HOME/.local/bin:$PATH"
```

Re-running `install.sh` is equivalent to installing the latest release over the same path.

## Architecture

```text
git tag vX.Y.Z
    → GitHub Actions builds 4 targets
    → Release assets: browse-X.Y.Z-<target>.tar.gz + SHA256SUMS

install.sh  ──┐
browse update ┴─ GET /repos/…/releases/latest
                 → pick asset for this OS/arch
                 → download tarball + SHA256SUMS
                 → verify SHA-256
                 → extract `browse`
                 → atomic replace dest
```

The TUI never downloads a binary. It only reads a local cache and may refresh that cache in the background.

## Components

| Piece | Responsibility |
|-------|----------------|
| `install.sh` | Detect target, fetch latest release, verify, write dest, PATH hint |
| `.github/workflows/release.yml` | On `v*.*.*` tags, build four binaries, publish tarballs + `SHA256SUMS` |
| `Cargo.toml` `[[bin]] name = "browse"` | Artifact and `cargo run` produce `browse` |
| `src/update.rs` | Target map, semver compare, GitHub parse, checksum, atomic replace, cache, `run_update` |
| `src/main.rs` | Clap name `browse`; subcommand `update` |
| Start Page | One status line when cached latest > current |
| `update-check.json` | Next to `home.json` via `HomeData::config_dir()` |

## Release assets

Version in filenames is the Cargo version (`0.1.0`), not the tag (`v0.1.0`).

| Target | Artifact |
|--------|----------|
| `aarch64-apple-darwin` | `browse-0.1.0-aarch64-apple-darwin.tar.gz` |
| `x86_64-apple-darwin` | `browse-0.1.0-x86_64-apple-darwin.tar.gz` |
| `x86_64-unknown-linux-musl` | `browse-0.1.0-x86_64-unknown-linux-musl.tar.gz` |
| `aarch64-unknown-linux-musl` | `browse-0.1.0-aarch64-unknown-linux-musl.tar.gz` |

Each tarball contains a single file named `browse` (no directory prefix).

`SHA256SUMS` is `sha256sum` / `shasum -a 256` format, one line per tarball:

```text
<64 hex>  browse-0.1.0-aarch64-apple-darwin.tar.gz
```

## Target detection

| `uname -s` | `uname -m` | Target |
|------------|------------|--------|
| Darwin | arm64 | `aarch64-apple-darwin` |
| Darwin | x86_64 | `x86_64-apple-darwin` |
| Linux | x86_64 | `x86_64-unknown-linux-musl` |
| Linux | aarch64 or arm64 | `aarch64-unknown-linux-musl` |

Anything else is an error: unsupported platform. Do not attempt a fallback build.

## Data flow

### Fresh install

1. `install.sh` maps `uname` → target.
2. `GET https://api.github.com/repos/dev-the-dev-while-deving/termbrowse/releases/latest` with `User-Agent: browse-installer`.
3. Read `tag_name` and asset `browser_download_url`s.
4. Download `browse-<ver>-<target>.tar.gz` and `SHA256SUMS`.
5. Verify hex digest. Mismatch → exit 1, leave dest untouched.
6. Extract `browse` to `$DEST.new`, `chmod 755`, `mv` over `$DEST`.
7. Print version and PATH hint if needed.

### `browse update`

1. Ignore the 24h cache (always talk to GitHub).
2. Same fetch/verify/replace as install, dest = `std::env::current_exe()` (follow the real path; replace that file).
3. Outcomes:
   - already latest → stdout `browse is up to date (0.1.0)` exit 0
   - replaced → stdout `updated 0.1.0 → 0.2.0` exit 0
   - dest not writable → stderr explain; suggest re-running `install.sh`; exit 1
   - checksum / network / unknown target → stderr, dest untouched, exit 1

The running process keeps the old inode; the next `browse` is the new binary.

### Start Page notice

1. On TUI start, load `update-check.json` from `HomeData::config_dir()` (local file only — must stay well under the 50ms Start Page budget).
2. If `latest` is newer than `CARGO_PKG_VERSION`, set status to `vX.Y.Z available — browse update`.
3. If cache missing or `checked_at` older than 24 hours, spawn a background task (do not await before first paint). The existing 80ms event-loop poll may pick up the notice in the same session.
4. Background task: GitHub latest, compare, write cache `{ checked_at, latest }`. On failure: write `checked_at` now, keep previous `latest` if any, no status error.
5. User-Agent: `browse/<version>`.

## Cache file

Path: `HomeData::config_dir().join("update-check.json")`  
(macOS: `~/Library/Application Support/termbrowse/`; Linux: `~/.config/termbrowse/` or `$XDG_CONFIG_HOME/termbrowse/`)

```json
{
  "checked_at": 1786665600,
  "latest": "0.2.0"
}
```

`latest` may be `null` after a failed first check. Banner only if `latest` is a string and `is_newer(latest, current)`.

TTL: 24 hours (`86400` seconds).

## Version compare

Strip a leading `v`/`V`. Parse `major.minor.patch` (non-digits after patch ignored). Compare as three integers. `0.2.0` > `0.1.9` > `0.1.0`. Equal → not newer.

## Atomic replace

1. Write bytes to `dest` with extension `.new` in the same directory.
2. `chmod 0o755`.
3. `rename` over `dest` (same filesystem — required for atomicity).
4. On any failure after creating `.new`, delete `.new` and leave `dest` as it was.

Tarball extract: write the `.tar.gz` to a temp file, run `tar -x -z -f <archive> -C <tempdir> browse`, read `<tempdir>/browse`, then atomic-replace dest. Tests create fixtures with system `tar`.

## Error handling

| Situation | Behavior |
|-----------|----------|
| Unknown OS/arch | Exit 1: `unsupported platform: <sys> <machine>` |
| No GitHub release / 404 | Exit 1: `no release found; tag a version (vX.Y.Z) first` |
| Network failure (install / `browse update`) | Exit 1, dest untouched |
| Network failure (Start Page check) | Silent; update `checked_at`; keep old `latest` |
| Checksum mismatch | Exit 1: `checksum mismatch, aborting`; dest untouched |
| GitHub rate limit | Exit 1 with status code; Start Page silent |
| Dest not writable | Exit 1; tell user to re-run installer or fix permissions |
| `tar` missing / bad archive | Exit 1; dest untouched |
| `~/.local/bin` missing | Create it (`mkdir -p`) |
| `~/.local/bin` not on PATH | Success + printed export line |

## Privacy

- The TUI does **not** phone home about pages, queries, or identity.
- The only extra network (beyond fetching pages the user asked for) is GitHub: `releases/latest` and asset download for install/update, plus the 24h latest check.
- No IP, machine id, or install ping is stored by us in this spec.
- Document the GitHub check in README (one sentence).

## Testing

Rust unit tests in `src/update.rs` (same style as `urlutil.rs` / `parse.rs`):

- Target map: four supported pairs; one unknown pair errors.
- `is_newer`: greater / equal / less; leading `v`.
- Asset filename for a version + target.
- Parse a fixture `releases/latest` JSON; pick the matching asset URL.
- Parse `SHA256SUMS`; verify matching bytes; reject mismatch.
- Atomic replace: dest gets new bytes and is executable.
- Extract a real `.tar.gz` fixture (one member named `browse`) and install it to a temp dest.
- Cache fresh vs stale at 24h boundary.
- Banner string exactly `v0.2.0 available — browse update`.
- `run_update` already-latest vs updated, with a fake fetcher (no live GitHub in unit tests).

`tests/install_test.sh` (invoked by a `#[test]` that runs `sh tests/install_test.sh` if `/bin/sh` exists):

- Mock `curl` on `PATH`.
- `HOME` is a temp dir.
- After `install.sh`, `$HOME/.local/bin/browse` exists, is executable, and matches the fixture payload.

Release workflow is config; no unit test. Verify by reading the matrix (four targets, tag trigger, asset names, `SHA256SUMS`).

## Files

| Path | Action |
|------|--------|
| `src/update.rs` | Create |
| `src/main.rs` | Add `update` subcommand; clap name `browse`; `mod update` |
| `src/tui.rs` | Background check + home status notice |
| `Cargo.toml` | `[[bin]] name = "browse"`; add `sha2` |
| `install.sh` | Create (POSIX sh) |
| `tests/install_test.sh` | Create |
| `.github/workflows/release.yml` | Create |
| `README.md` | Install / update / PATH / GitHub-check sentence |

## Out of this spec

Marketing site, Vercel, Supabase, download counters, Windows, Homebrew, notarization, editing the user’s shell rc, auto-download, `browse` as a rustup-style prefix, cargo-dist.
