# Curl install + browse update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a curl-installed `browse` binary and `browse update` so macOS/Linux users can install and upgrade without Rust.

**Architecture:** GitHub Releases host four musl/darwin tarballs plus `SHA256SUMS`. `install.sh` and `src/update.rs` share that contract (detect target → latest release → verify SHA-256 → atomic replace). The TUI only reads a 24h local cache and shows a Start Page notice. No telemetry, no site, no Supabase.

**Tech Stack:** Rust 2021, clap, reqwest, sha2, POSIX `sh`, `tar`, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-17-curl-install-browse-update-design.md`

**Workspace:** Work in place on `from-scratch`. Do not create a git worktree — most of `src/` is still untracked (only `src/main.rs` and `src/update.rs` are in git). Commit only files this task touches (plus `Cargo.lock` if you change crates). Follow TDD: failing test, then minimal code. Tests: `cargo test --bin termbrowse update::tests` (no `src/lib.rs`; after Task 5 the bin name becomes `browse`).

## Progress (2026-08-17)

**Plan tasks 1–8 implemented.** Site / Vercel / Supabase stay parked (second spec). Reviews after Task 4 skipped per user.

| Task | Status | Commit |
|------|--------|--------|
| 1 Target / version / asset / banner | done | `e779d12` |
| 2 SHA-256 / atomic replace / tar extract | done | `ca8ee74` |
| 2b Refuse symlink `browse` member | done | `57b4a72` |
| 3 24h update-check cache | done | `01cd563` |
| 4 GitHub parse + `run_update_with` | done | `4f35dd3` |
| 5 CLI `browse` + `browse update` | done | `6f1cc7a` |
| 6 Start Page notice | done | `52072eb` |
| 7 `install.sh` + shell test | done | `60a9bfc` |
| 8 Release workflow + README | done | `7013870` |

HEAD: `7013870` on `from-scratch` (11 commits ahead of origin; **not pushed**). Curl install for other machines needs `git push` and a `v0.1.0` tag so Actions can publish binaries.

Verified: `cargo test --bin browse update::tests` — 22 passed.

---

## File map

| File | Responsibility |
|------|----------------|
| `src/update.rs` | Target, versions, checksums, tar extract, atomic replace, cache, GitHub parse, `run_update` |
| `src/main.rs` | `mod update`; clap name `browse`; `Commands::Update` |
| `src/tui.rs` | Load/spawn update check; home status notice |
| `Cargo.toml` | `[[bin]] name = "browse"`; `sha2` |
| `install.sh` | POSIX installer |
| `tests/install_test.sh` | Mock-curl install smoke test |
| `.github/workflows/release.yml` | Tag-triggered four-target release |
| `README.md` | curl / `browse` / `browse update` |

---

### Task 1: Target map, versions, asset names, banner

**Files:**
- Create: `src/update.rs`
- Modify: `src/main.rs` (add `mod update;` only)

- [ ] **Step 1: Write the failing tests**

Create `src/update.rs` with **only** the test module (no production fns yet):

```rust
//! Self-update helpers. GitHub Releases only. No telemetry.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_supported_targets() {
        assert_eq!(map_target("Darwin", "arm64").unwrap(), "aarch64-apple-darwin");
        assert_eq!(map_target("Darwin", "x86_64").unwrap(), "x86_64-apple-darwin");
        assert_eq!(map_target("Linux", "x86_64").unwrap(), "x86_64-unknown-linux-musl");
        assert_eq!(map_target("Linux", "aarch64").unwrap(), "aarch64-unknown-linux-musl");
        assert_eq!(map_target("Linux", "arm64").unwrap(), "aarch64-unknown-linux-musl");
    }

    #[test]
    fn rejects_unknown_target() {
        let err = map_target("Windows", "x86_64").unwrap_err().to_string();
        assert!(err.contains("unsupported platform"), "{err}");
        assert!(err.contains("Windows"), "{err}");
        assert!(err.contains("x86_64"), "{err}");
    }

    #[test]
    fn version_compare() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn asset_name_strips_v() {
        assert_eq!(
            asset_filename("v0.1.0", "aarch64-apple-darwin"),
            "browse-0.1.0-aarch64-apple-darwin.tar.gz"
        );
    }

    #[test]
    fn banner_text() {
        assert_eq!(banner("0.2.0"), "v0.2.0 available — browse update");
        assert_eq!(banner("v0.2.0"), "v0.2.0 available — browse update");
    }
}
```

Add `mod update;` near the other `mod` lines in `src/main.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib update::tests -- --nocapture`

Expected: compile error, `map_target` / `is_newer` / `asset_filename` / `banner` not found.

- [ ] **Step 3: Write minimal implementation**

Put this **above** the test module in `src/update.rs`:

```rust
//! Self-update helpers. GitHub Releases only. No telemetry.

use anyhow::{bail, Result};

pub fn map_target(os: &str, arch: &str) -> Result<String> {
    let t = match (os, arch) {
        ("Darwin", "arm64") => "aarch64-apple-darwin",
        ("Darwin", "x86_64") => "x86_64-apple-darwin",
        ("Linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("Linux", "aarch64") | ("Linux", "arm64") => "aarch64-unknown-linux-musl",
        _ => bail!("unsupported platform: {os} {arch}"),
    };
    Ok(t.to_string())
}

pub fn strip_v(s: &str) -> &str {
    s.trim().trim_start_matches(['v', 'V'])
}

pub fn parse_version(s: &str) -> Result<(u64, u64, u64)> {
    let s = strip_v(s);
    let mut parts = s.split('.');
    let major = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let minor = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let patch = parts
        .next()
        .unwrap_or("0")
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0);
    Ok((major, minor, patch))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    parse_version(latest).unwrap_or((0, 0, 0)) > parse_version(current).unwrap_or((0, 0, 0))
}

pub fn asset_filename(version: &str, target: &str) -> String {
    format!("browse-{}-{}.tar.gz", strip_v(version), target)
}

pub fn banner(latest: &str) -> String {
    format!("v{} available — browse update", strip_v(latest))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib update::tests`

Expected: all 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/update.rs src/main.rs
git commit -m "$(cat <<'EOF'
feat: add update target, version, and asset helpers

EOF
)"
```

---

### Task 2: SHA-256 verify, atomic replace, tarball extract

**Files:**
- Modify: `Cargo.toml` (add `sha2 = "0.10"`)
- Modify: `src/update.rs`

- [ ] **Step 1: Write the failing tests**

Append inside `src/update.rs` `mod tests`:

```rust
    #[test]
    fn sha256_roundtrip() {
        let hex = sha256_hex(b"hello");
        assert_eq!(
            hex,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        verify_sha256(b"hello", &hex).unwrap();
        assert!(verify_sha256(b"hello", "deadbeef").is_err());
    }

    #[test]
    fn parses_sha256sums() {
        let text = "abc123  browse-0.1.0-aarch64-apple-darwin.tar.gz\n\
                    def456  SHA256SUMS\n";
        let map = parse_sha256sums(text);
        assert_eq!(
            map.get("browse-0.1.0-aarch64-apple-darwin.tar.gz").map(String::as_str),
            Some("abc123")
        );
    }

    #[test]
    fn atomic_replace_writes_executable() {
        let dir = std::env::temp_dir().join(format!("browse-replace-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let dest = dir.join("browse");
        atomic_replace(&dest, b"#!/bin/sh\necho hi\n").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"#!/bin/sh\necho hi\n");
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode();
        assert_ne!(mode & 0o111, 0, "expected executable, mode={mode:o}");
        atomic_replace(&dest, b"new").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extracts_browse_member_from_tarball() {
        let dir = std::env::temp_dir().join(format!("browse-tar-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("browse"), b"payload-bytes").unwrap();
        let tar_path = dir.join("b.tar.gz");
        let status = std::process::Command::new("tar")
            .args(["-c", "-z", "-f"])
            .arg(&tar_path)
            .arg("-C")
            .arg(&dir)
            .arg("browse")
            .status()
            .unwrap();
        assert!(status.success());
        let tarball = std::fs::read(&tar_path).unwrap();
        let dest = dir.join("out").join("browse");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        install_tarball(&tarball, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"payload-bytes");
        let _ = std::fs::remove_dir_all(&dir);
    }
```

Add `use std::os::unix::fs::PermissionsExt;` inside the test module (or at crate level — crate level is required for `atomic_replace` anyway).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib update::tests`

Expected: compile error (`sha256_hex`, `verify_sha256`, `parse_sha256sums`, `atomic_replace`, `install_tarball` not found).

- [ ] **Step 3: Write minimal implementation**

Add to `Cargo.toml` under `[dependencies]`:

```toml
sha2 = "0.10"
```

Add to `src/update.rs` (top uses + fns):

```rust
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn parse_sha256sums(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(hex) = parts.next() else { continue };
        let Some(name) = parts.next() else { continue };
        let name = name.trim_start_matches('*');
        map.insert(name.to_string(), hex.to_ascii_lowercase());
    }
    map
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<()> {
    let got = sha256_hex(bytes);
    let exp = expected_hex.trim().to_ascii_lowercase();
    if got != exp {
        bail!("checksum mismatch, aborting");
    }
    Ok(())
}

pub fn atomic_replace(dest: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    let tmp = dest.with_extension("new");
    let result = (|| {
        fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
        let mut perms = fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp, perms)?;
        fs::rename(&tmp, dest).with_context(|| format!("replace {}", dest.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn extract_browse_from_tarball(tarball: &[u8]) -> Result<Vec<u8>> {
    let dir = std::env::temp_dir().join(format!(
        "browse-extract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir)?;
    let archive = dir.join("in.tar.gz");
    fs::write(&archive, tarball)?;
    let status = Command::new("tar")
        .args(["-x", "-z", "-f"])
        .arg(&archive)
        .arg("-C")
        .arg(&dir)
        .arg("browse")
        .status()
        .context("run tar")?;
    if !status.success() {
        let _ = fs::remove_dir_all(&dir);
        bail!("tar extract failed");
    }
    let bytes = fs::read(dir.join("browse"));
    let _ = fs::remove_dir_all(&dir);
    bytes.context("read extracted browse")
}

pub fn install_tarball(tarball: &[u8], dest: &Path) -> Result<()> {
    let bin = extract_browse_from_tarball(tarball)?;
    atomic_replace(dest, &bin)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib update::tests`

Expected: all tests PASS (previous 5 + 4 new).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/update.rs
git commit -m "$(cat <<'EOF'
feat: verify checksums and atomically replace browse binary

EOF
)"
```

---

### Task 3: Update-check cache

**Files:**
- Modify: `src/update.rs`

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `src/update.rs`:

```rust
    #[test]
    fn cache_freshness_24h() {
        let c = CheckCache {
            checked_at: 1_000,
            latest: Some("0.2.0".into()),
        };
        assert!(cache_is_fresh(&c, 1_000 + 86_399));
        assert!(!cache_is_fresh(&c, 1_000 + 86_400));
        assert!(!cache_is_fresh(&c, 1_000 + 86_401));
    }

    #[test]
    fn notice_only_when_newer() {
        let c = CheckCache {
            checked_at: 1,
            latest: Some("0.2.0".into()),
        };
        assert_eq!(
            notice_if_newer(&c, "0.1.0").as_deref(),
            Some("v0.2.0 available — browse update")
        );
        assert_eq!(notice_if_newer(&c, "0.2.0"), None);
        let empty = CheckCache {
            checked_at: 1,
            latest: None,
        };
        assert_eq!(notice_if_newer(&empty, "0.1.0"), None);
    }

    #[test]
    fn cache_roundtrip_file() {
        let dir = std::env::temp_dir().join(format!("browse-cache-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("update-check.json");
        let c = CheckCache {
            checked_at: 42,
            latest: Some("0.3.0".into()),
        };
        save_cache_at(&path, &c).unwrap();
        let loaded = load_cache_at(&path);
        assert_eq!(loaded.checked_at, 42);
        assert_eq!(loaded.latest.as_deref(), Some("0.3.0"));
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib update::tests::cache_freshness_24h update::tests::notice_only_when_newer update::tests::cache_roundtrip_file`

Expected: compile error, `CheckCache` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `src/update.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::home::HomeData;

pub const CHECK_TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckCache {
    pub checked_at: u64,
    #[serde(default)]
    pub latest: Option<String>,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn cache_path() -> PathBuf {
    HomeData::config_dir().join("update-check.json")
}

pub fn load_cache_at(path: &Path) -> CheckCache {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn load_cache() -> CheckCache {
    load_cache_at(&cache_path())
}

pub fn save_cache_at(path: &Path, cache: &CheckCache) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(cache)?)?;
    Ok(())
}

pub fn save_cache(cache: &CheckCache) -> Result<()> {
    save_cache_at(&cache_path(), cache)
}

pub fn cache_is_fresh(cache: &CheckCache, now: u64) -> bool {
    cache.checked_at > 0 && now.saturating_sub(cache.checked_at) < CHECK_TTL_SECS
}

pub fn notice_if_newer(cache: &CheckCache, current: &str) -> Option<String> {
    let latest = cache.latest.as_deref()?;
    if is_newer(latest, current) {
        Some(banner(latest))
    } else {
        None
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib update::tests`

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/update.rs
git commit -m "$(cat <<'EOF'
feat: persist 24h GitHub update-check cache

EOF
)"
```

---

### Task 4: GitHub release parse + `run_update_with`

**Files:**
- Modify: `src/update.rs`

- [ ] **Step 1: Write the failing tests**

Append to `mod tests`:

```rust
    fn sample_release_json() -> String {
        r#"{
          "tag_name": "v0.2.0",
          "assets": [
            {
              "name": "browse-0.2.0-aarch64-apple-darwin.tar.gz",
              "browser_download_url": "https://example.test/browse-0.2.0-aarch64-apple-darwin.tar.gz"
            },
            {
              "name": "SHA256SUMS",
              "browser_download_url": "https://example.test/SHA256SUMS"
            }
          ]
        }"#
        .into()
    }

    #[test]
    fn parses_latest_release() {
        let r = parse_release_json(&sample_release_json()).unwrap();
        assert_eq!(r.tag, "v0.2.0");
        assert_eq!(r.version, "0.2.0");
        let a = pick_asset(&r, "aarch64-apple-darwin").unwrap();
        assert_eq!(
            a.url,
            "https://example.test/browse-0.2.0-aarch64-apple-darwin.tar.gz"
        );
        assert_eq!(
            pick_checksums_url(&r).unwrap(),
            "https://example.test/SHA256SUMS"
        );
    }

    struct MapFetcher {
        text: std::collections::HashMap<String, String>,
        bytes: std::collections::HashMap<String, Vec<u8>>,
    }

    impl Fetcher for MapFetcher {
        fn get_text(&self, url: &str) -> Result<String> {
            self.text
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no text {url}"))
        }
        fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
            self.bytes
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no bytes {url}"))
        }
    }

    fn pack_browse(payload: &[u8]) -> Vec<u8> {
        let dir = std::env::temp_dir().join(format!(
            "browse-pack-{}-{}",
            std::process::id(),
            payload.len()
        ));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("browse"), payload).unwrap();
        let tar_path = dir.join("b.tar.gz");
        assert!(std::process::Command::new("tar")
            .args(["-c", "-z", "-f"])
            .arg(&tar_path)
            .arg("-C")
            .arg(&dir)
            .arg("browse")
            .status()
            .unwrap()
            .success());
        std::fs::read(&tar_path).unwrap()
    }

    #[test]
    fn update_already_latest() {
        let tarball = pack_browse(b"bin");
        let mut text = std::collections::HashMap::new();
        text.insert(latest_api_url(), sample_release_json());
        let fetcher = MapFetcher {
            text,
            bytes: std::collections::HashMap::new(),
        };
        let dir = std::env::temp_dir().join(format!("browse-up-same-{}", std::process::id()));
        let dest = dir.join("browse");
        let out = run_update_with(&fetcher, "0.2.0", &dest, "aarch64-apple-darwin").unwrap();
        assert!(matches!(out, UpdateOutcome::AlreadyLatest { .. }));
        assert!(!dest.exists());
        let _ = tarball;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn update_replaces_when_newer() {
        let tarball = pack_browse(b"new-binary");
        let hex = sha256_hex(&tarball);
        let name = asset_filename("0.2.0", "aarch64-apple-darwin");
        let sums = format!("{hex}  {name}\n");
        let mut text = std::collections::HashMap::new();
        text.insert(latest_api_url(), sample_release_json());
        text.insert("https://example.test/SHA256SUMS".into(), sums);
        let mut bytes = std::collections::HashMap::new();
        bytes.insert(
            "https://example.test/browse-0.2.0-aarch64-apple-darwin.tar.gz".into(),
            tarball,
        );
        let fetcher = MapFetcher { text, bytes };
        let dir = std::env::temp_dir().join(format!("browse-up-new-{}", std::process::id()));
        let dest = dir.join("browse");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&dest, b"old").unwrap();
        let out = run_update_with(&fetcher, "0.1.0", &dest, "aarch64-apple-darwin").unwrap();
        match out {
            UpdateOutcome::Updated { from, to } => {
                assert_eq!(from, "0.1.0");
                assert_eq!(to, "0.2.0");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(std::fs::read(&dest).unwrap(), b"new-binary");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn update_refuses_bad_checksum() {
        let tarball = pack_browse(b"new-binary");
        let name = asset_filename("0.2.0", "aarch64-apple-darwin");
        let sums = format!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  {name}\n");
        let mut text = std::collections::HashMap::new();
        text.insert(latest_api_url(), sample_release_json());
        text.insert("https://example.test/SHA256SUMS".into(), sums);
        let mut bytes = std::collections::HashMap::new();
        bytes.insert(
            "https://example.test/browse-0.2.0-aarch64-apple-darwin.tar.gz".into(),
            tarball,
        );
        let fetcher = MapFetcher { text, bytes };
        let dir = std::env::temp_dir().join(format!("browse-up-bad-{}", std::process::id()));
        let dest = dir.join("browse");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&dest, b"old").unwrap();
        let err = run_update_with(&fetcher, "0.1.0", &dest, "aarch64-apple-darwin")
            .unwrap_err()
            .to_string();
        assert!(err.contains("checksum mismatch"), "{err}");
        assert_eq!(std::fs::read(&dest).unwrap(), b"old");
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib update::tests::parses_latest_release`

Expected: compile error, `parse_release_json` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `src/update.rs`:

```rust
pub const GITHUB_REPO: &str = "dev-the-dev-while-deving/termbrowse";

pub fn latest_api_url() -> String {
    format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest")
}

#[derive(Debug, Clone)]
pub struct Release {
    pub tag: String,
    pub version: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    AlreadyLatest { version: String },
    Updated { from: String, to: String },
}

pub trait Fetcher {
    fn get_text(&self, url: &str) -> Result<String>;
    fn get_bytes(&self, url: &str) -> Result<Vec<u8>>;
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

pub fn parse_release_json(json: &str) -> Result<Release> {
    let raw: GhRelease = serde_json::from_str(json).context("parse release json")?;
    let version = strip_v(&raw.tag_name).to_string();
    Ok(Release {
        tag: raw.tag_name,
        version,
        assets: raw
            .assets
            .into_iter()
            .map(|a| Asset {
                name: a.name,
                url: a.browser_download_url,
            })
            .collect(),
    })
}

pub fn pick_asset<'a>(release: &'a Release, target: &str) -> Result<&'a Asset> {
    let want = asset_filename(&release.version, target);
    release
        .assets
        .iter()
        .find(|a| a.name == want)
        .with_context(|| format!("no asset {want}"))
}

pub fn pick_checksums_url(release: &Release) -> Result<&str> {
    release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS")
        .map(|a| a.url.as_str())
        .context("no SHA256SUMS asset")
}

pub fn run_update_with(
    fetcher: &dyn Fetcher,
    current: &str,
    dest: &Path,
    target: &str,
) -> Result<UpdateOutcome> {
    let json = fetcher.get_text(&latest_api_url())?;
    let release = parse_release_json(&json)?;
    if !is_newer(&release.version, current) {
        return Ok(UpdateOutcome::AlreadyLatest {
            version: strip_v(current).to_string(),
        });
    }
    let asset = pick_asset(&release, target)?;
    let sums_url = pick_checksums_url(&release)?;
    let sums = fetcher.get_text(sums_url)?;
    let map = parse_sha256sums(&sums);
    let expected = map
        .get(&asset.name)
        .with_context(|| format!("no checksum for {}", asset.name))?;
    let tarball = fetcher.get_bytes(&asset.url)?;
    verify_sha256(&tarball, expected)?;
    install_tarball(&tarball, dest)?;
    Ok(UpdateOutcome::Updated {
        from: strip_v(current).to_string(),
        to: release.version,
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib update::tests`

Expected: all PASS, including already-latest / replace / bad checksum.

- [ ] **Step 5: Commit**

```bash
git add src/update.rs
git commit -m "$(cat <<'EOF'
feat: apply GitHub release updates through a fakeable fetcher

EOF
)"
```

---

### Task 5: CLI `browse` + `browse update`

**Files:**
- Modify: `Cargo.toml` (add `[[bin]]`)
- Modify: `src/main.rs`
- Modify: `src/update.rs` (reqwest `ReqwestFetcher` + `run_update`)

- [ ] **Step 1: Write the failing test**

Add to `src/update.rs` tests:

```rust
    #[test]
    fn detect_target_matches_host() {
        let t = detect_target().expect("this CI/dev host must be a supported target");
        assert!(
            t.ends_with("apple-darwin") || t.ends_with("linux-musl"),
            "{t}"
        );
    }
```

This should fail to compile (`detect_target` missing).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib update::tests::detect_target_matches_host`

Expected: compile error, `detect_target` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `src/update.rs`:

```rust
pub fn detect_target() -> Result<String> {
    let os = Command::new("uname").arg("-s").output().context("uname -s")?;
    let arch = Command::new("uname").arg("-m").output().context("uname -m")?;
    if !os.status.success() || !arch.status.success() {
        bail!("uname failed");
    }
    map_target(
        std::str::from_utf8(&os.stdout)?.trim(),
        std::str::from_utf8(&arch.stdout)?.trim(),
    )
}

pub async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client.get(url).send().await.with_context(|| url.to_string())?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("no release found; tag a version (vX.Y.Z) first");
    }
    if !resp.status().is_success() {
        bail!("download failed: {} {url}", resp.status());
    }
    Ok(resp.text().await?)
}

pub async fn fetch_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let resp = client.get(url).send().await.with_context(|| url.to_string())?;
    if !resp.status().is_success() {
        bail!("download failed: {} {url}", resp.status());
    }
    Ok(resp.bytes().await?.to_vec())
}

pub struct AsyncClientFetcher {
    pub latest_json: String,
    pub sums: String,
    pub tarball: Vec<u8>,
}

pub async fn run_update(current: &str, dest: &Path) -> Result<UpdateOutcome> {
    let ua = format!("browse/{}", strip_v(current));
    let client = reqwest::Client::builder()
        .user_agent(&ua)
        .build()
        .context("http client")?;
    let json = fetch_text(&client, &latest_api_url()).await?;
    let release = parse_release_json(&json)?;
    if !is_newer(&release.version, current) {
        return Ok(UpdateOutcome::AlreadyLatest {
            version: strip_v(current).to_string(),
        });
    }
    let asset = pick_asset(&release, &detect_target()?)?;
    let sums_url = pick_checksums_url(&release)?;
    let sums = fetch_text(&client, sums_url).await?;
    let expected = parse_sha256sums(&sums)
        .get(&asset.name)
        .cloned()
        .with_context(|| format!("no checksum for {}", asset.name))?;
    let tarball = fetch_bytes(&client, &asset.url).await?;
    verify_sha256(&tarball, &expected)?;
    install_tarball(&tarball, dest)?;
    Ok(UpdateOutcome::Updated {
        from: strip_v(current).to_string(),
        to: release.version,
    })
}
```

In `Cargo.toml` after `[dependencies]`:

```toml
[[bin]]
name = "browse"
path = "src/main.rs"
```

In `src/main.rs`:

- Change clap `name = "termbrowse"` to `name = "browse"`.
- Add to `Commands`:

```rust
    /// Replace this binary with the latest GitHub Release
    Update,
```

- In `match cli.command`:

```rust
        Some(Commands::Update) => {
            let dest = std::env::current_exe().context("current exe")?;
            let dest = dest.canonicalize().unwrap_or(dest);
            match update::run_update(env!("CARGO_PKG_VERSION"), &dest).await? {
                update::UpdateOutcome::AlreadyLatest { version } => {
                    println!("browse is up to date ({version})");
                }
                update::UpdateOutcome::Updated { from, to } => {
                    println!("updated {from} → {to}");
                }
            }
        }
```

Keep `Home`, `Open`, `Snapshot`, `Text` unchanged.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```
cargo test --lib update::tests
cargo build
```

Expected: tests PASS. `cargo build` produces `target/debug/browse` (not `termbrowse`).

Run: `./target/debug/browse --help`

Expected: usage name `browse`, subcommand `update` listed.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/main.rs src/update.rs
git commit -m "$(cat <<'EOF'
feat: add browse update and rename the binary to browse

EOF
)"
```

---

### Task 6: Start Page notice (non-blocking)

**Files:**
- Modify: `src/update.rs` (`refresh_latest_cache`)
- Modify: `src/tui.rs`

- [ ] **Step 1: Write the failing test**

Add to `src/update.rs` tests:

```rust
    #[test]
    fn failed_check_keeps_previous_latest() {
        let prev = CheckCache {
            checked_at: 10,
            latest: Some("0.2.0".into()),
        };
        let next = cache_after_check(prev, 99, None);
        assert_eq!(next.checked_at, 99);
        assert_eq!(next.latest.as_deref(), Some("0.2.0"));
        let next_ok = cache_after_check(
            CheckCache {
                checked_at: 10,
                latest: Some("0.1.0".into()),
            },
            99,
            Some("0.3.0".into()),
        );
        assert_eq!(next_ok.latest.as_deref(), Some("0.3.0"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib update::tests::failed_check_keeps_previous_latest`

Expected: compile error, `cache_after_check` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `src/update.rs`:

```rust
pub fn cache_after_check(prev: CheckCache, now: u64, latest: Option<String>) -> CheckCache {
    CheckCache {
        checked_at: now,
        latest: latest.or(prev.latest),
    }
}

pub async fn refresh_latest_cache() -> Result<CheckCache> {
    let prev = load_cache();
    let ua = format!("browse/{}", env!("CARGO_PKG_VERSION"));
    let client = reqwest::Client::builder().user_agent(&ua).build()?;
    let latest = match fetch_text(&client, &latest_api_url()).await {
        Ok(json) => parse_release_json(&json).ok().map(|r| r.version),
        Err(_) => None,
    };
    let next = cache_after_check(prev, now_secs(), latest);
    let _ = save_cache(&next);
    Ok(next)
}
```

In `src/tui.rs`:

1. Add `use std::sync::{Arc, Mutex};` and `use crate::update;`
2. Add field on `App`: `update_notice: Arc<Mutex<Option<String>>>`
3. In `App::new`, after constructing fields, set:

```rust
update_notice: {
    let cache = update::load_cache();
    Arc::new(Mutex::new(update::notice_if_newer(
        &cache,
        env!("CARGO_PKG_VERSION"),
    )))
},
```

Every other `App { ... }` construction site must include this field (only `App::new` builds it).

4. After `App::new()` body (end of `new`, before `with_home_status`), spawn if stale:

```rust
    // inside new(), after Self { ... } is built — easier as a method:
```

Implement `fn kick_update_check(notice: Arc<Mutex<Option<String>>>)` in `tui.rs`:

```rust
fn kick_update_check(notice: Arc<Mutex<Option<String>>>) {
    let cache = update::load_cache();
    if update::cache_is_fresh(&cache, update::now_secs()) {
        return;
    }
    tokio::spawn(async move {
        if let Ok(next) = update::refresh_latest_cache().await {
            if let Some(msg) = update::notice_if_newer(&next, env!("CARGO_PKG_VERSION")) {
                if let Ok(mut g) = notice.lock() {
                    *g = Some(msg);
                }
            }
        }
    });
}
```

Call `kick_update_check(Arc::clone(&app.update_notice));` from `run_app` immediately after `let mut terminal = ratatui::init();` (first paint is the following `event_loop` draw — do **not** `await` the GitHub call).

5. Change `refresh_home_status`:

```rust
    fn refresh_home_status(&mut self) {
        if let Ok(g) = self.update_notice.lock() {
            if let Some(msg) = g.as_ref() {
                self.status = msg.clone();
                return;
            }
        }
        self.status = "Search DuckDuckGo · type a query · enter".into();
    }
```

6. In `event_loop`, when `!event::poll(...)` (the 80ms idle branch), if `app.t().screen == Screen::Home`, call `app.refresh_home_status();` so a same-session check can appear.

Do not add a GitHub call on the first-paint path. Do not log network errors to the status bar.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib update::tests`

Expected: PASS.

Run: `cargo build`

Expected: success.

- [ ] **Step 5: Commit**

```bash
git add src/update.rs src/tui.rs
git commit -m "$(cat <<'EOF'
feat: show a Start Page notice when a newer browse exists

EOF
)"
```

---

### Task 7: `install.sh` + shell test

**Files:**
- Create: `install.sh`
- Create: `tests/install_test.sh`
- Modify: `src/update.rs` (add `#[test] fn install_script_smokes` that execs the shell test)

- [ ] **Step 1: Write the failing test**

Create `tests/install_test.sh`:

```sh
#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

payload="fixture-browse-ok"
mkdir -p "$tmp/payload"
printf '%s' "$payload" > "$tmp/payload/browse"
tar -c -z -f "$tmp/browse-0.2.0-aarch64-apple-darwin.tar.gz" -C "$tmp/payload" browse
tar -c -z -f "$tmp/browse-0.2.0-x86_64-apple-darwin.tar.gz" -C "$tmp/payload" browse
tar -c -z -f "$tmp/browse-0.2.0-x86_64-unknown-linux-musl.tar.gz" -C "$tmp/payload" browse
tar -c -z -f "$tmp/browse-0.2.0-aarch64-unknown-linux-musl.tar.gz" -C "$tmp/payload" browse

: > "$tmp/SHA256SUMS"
for f in \
  browse-0.2.0-aarch64-apple-darwin.tar.gz \
  browse-0.2.0-x86_64-apple-darwin.tar.gz \
  browse-0.2.0-x86_64-unknown-linux-musl.tar.gz \
  browse-0.2.0-aarch64-unknown-linux-musl.tar.gz
do
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$tmp" && sha256sum "$f") >> "$tmp/SHA256SUMS"
  else
    (cd "$tmp" && shasum -a 256 "$f") >> "$tmp/SHA256SUMS"
  fi
done

cat > "$tmp/release.json" <<'JSON'
{
  "tag_name": "v0.2.0",
  "assets": [
    {"name":"browse-0.2.0-aarch64-apple-darwin.tar.gz","browser_download_url":"https://example.test/browse-0.2.0-aarch64-apple-darwin.tar.gz"},
    {"name":"browse-0.2.0-x86_64-apple-darwin.tar.gz","browser_download_url":"https://example.test/browse-0.2.0-x86_64-apple-darwin.tar.gz"},
    {"name":"browse-0.2.0-x86_64-unknown-linux-musl.tar.gz","browser_download_url":"https://example.test/browse-0.2.0-x86_64-unknown-linux-musl.tar.gz"},
    {"name":"browse-0.2.0-aarch64-unknown-linux-musl.tar.gz","browser_download_url":"https://example.test/browse-0.2.0-aarch64-unknown-linux-musl.tar.gz"},
    {"name":"SHA256SUMS","browser_download_url":"https://example.test/SHA256SUMS"}
  ]
}
JSON

mkdir -p "$tmp/bin"
cat > "$tmp/bin/curl" <<EOF
#!/bin/sh
set -eu
url=""
while [ \$# -gt 0 ]; do
  case "\$1" in
    http*|https*) url="\$1" ;;
  esac
  shift
done
case "\$url" in
  *releases/latest) cat "$tmp/release.json" ;;
  *SHA256SUMS) cat "$tmp/SHA256SUMS" ;;
  *browse-0.2.0-*.tar.gz)
    name=\$(printf '%s' "\$url" | sed 's|.*/||')
    cat "$tmp/\$name"
    ;;
  *) echo "unexpected curl \$url" >&2; exit 1 ;;
esac
EOF
chmod +x "$tmp/bin/curl"

export PATH="$tmp/bin:$PATH"
export HOME="$tmp/home"
export BROWSE_INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$HOME"

sh "$root/install.sh"

test -x "$BROWSE_INSTALL_DIR/browse"
got=$(cat "$BROWSE_INSTALL_DIR/browse")
test "$got" = "$payload"
```

Add to `src/update.rs` tests:

```rust
    #[test]
    fn install_script_smokes() {
        let status = std::process::Command::new("sh")
            .arg("tests/install_test.sh")
            .status()
            .expect("run sh");
        assert!(status.success(), "install_test.sh failed");
    }
```

Do **not** create `install.sh` yet.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib update::tests::install_script_smokes -- --nocapture`

Expected: FAIL because `install.sh` is missing (`sh: .../install.sh: No such file`).

- [ ] **Step 3: Write `install.sh`**

Create POSIX `install.sh` at repo root (executable):

```sh
#!/bin/sh
set -eu

REPO="${BROWSE_REPO:-dev-the-dev-while-deving/termbrowse}"
INSTALL_DIR="${BROWSE_INSTALL_DIR:-$HOME/.local/bin}"
UA="${BROWSE_UA:-browse-installer}"
API_URL="${BROWSE_API_URL:-https://api.github.com/repos/${REPO}/releases/latest}"

detect_target() {
  sys=$(uname -s)
  mach=$(uname -m)
  case "${sys}:${mach}" in
    Darwin:arm64) echo aarch64-apple-darwin ;;
    Darwin:x86_64) echo x86_64-apple-darwin ;;
    Linux:x86_64) echo x86_64-unknown-linux-musl ;;
    Linux:aarch64|Linux:arm64) echo aarch64-unknown-linux-musl ;;
    *)
      echo "unsupported platform: ${sys} ${mach}" >&2
      exit 1
      ;;
  esac
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

need_cmd curl
need_cmd tar
need_cmd uname
need_cmd mktemp

target=$(detect_target)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

json=$(curl -fsSL -A "$UA" "$API_URL") || {
  echo "no release found; tag a version (vX.Y.Z) first" >&2
  exit 1
}

tag=$(printf '%s' "$json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
if [ -z "$tag" ]; then
  echo "no release found; tag a version (vX.Y.Z) first" >&2
  exit 1
fi
version=$(printf '%s' "$tag" | sed 's/^[vV]//')
asset="browse-${version}-${target}.tar.gz"

asset_url=$(printf '%s' "$json" | tr ',' '\n' | sed -n "s/.*\"browser_download_url\"[[:space:]]*:[[:space:]]*\"\\([^\"]*${asset}\\)\".*/\\1/p" | head -n 1)
sums_url=$(printf '%s' "$json" | tr ',' '\n' | sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*SHA256SUMS\)".*/\1/p' | head -n 1)

if [ -z "$asset_url" ] || [ -z "$sums_url" ]; then
  echo "no release found; tag a version (vX.Y.Z) first" >&2
  exit 1
fi

curl -fsSL -A "$UA" "$asset_url" -o "$work/$asset"
curl -fsSL -A "$UA" "$sums_url" -o "$work/SHA256SUMS"

expected=$(awk -v n="$asset" '$2 == n || $2 == "*"n { print $1; exit }' "$work/SHA256SUMS")
if [ -z "$expected" ]; then
  echo "checksum mismatch, aborting" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  got=$(sha256sum "$work/$asset" | awk '{print $1}')
else
  got=$(shasum -a 256 "$work/$asset" | awk '{print $1}')
fi

if [ "$got" != "$expected" ]; then
  echo "checksum mismatch, aborting" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
tar -x -z -f "$work/$asset" -C "$work" browse
chmod 755 "$work/browse"
mv "$work/browse" "$INSTALL_DIR/browse"

echo "installed browse ${version} -> ${INSTALL_DIR}/browse"

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo ""
    echo "Add this to your shell profile, then open a new terminal:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac
```

`chmod +x install.sh`

JSON parsing with `sed` is fragile; the test fixture is compact one-line-ish objects. If `sed` fails on pretty-printed JSON in the test, flatten `release.json` to one line in the test (already mostly fine) **or** extract URLs with:

```sh
printf '%s' "$json" | tr '"' '\n' | grep -F "$asset" | grep -E '^https?://' | head -n 1
```

Prefer the `tr '"'` approach if the first extract is unreliable. The test must pass.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib update::tests::install_script_smokes -- --nocapture`

Expected: PASS. `$HOME/.local/bin/browse` in the temp home contains `fixture-browse-ok`.

Also run: `cargo test --lib update::tests`

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add install.sh tests/install_test.sh src/update.rs
git commit -m "$(cat <<'EOF'
feat: add curl installer for prebuilt browse

EOF
)"
```

---

### Task 8: Release workflow + README

**Files:**
- Create: `.github/workflows/release.yml`
- Modify: `README.md`

- [ ] **Step 1: Write the failing test**

Add to `src/update.rs` tests:

```rust
    #[test]
    fn release_workflow_covers_four_targets() {
        let yml = std::fs::read_to_string(".github/workflows/release.yml")
            .expect("release.yml");
        for t in [
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-musl",
        ] {
            assert!(yml.contains(t), "missing target {t}");
        }
        assert!(yml.contains("SHA256SUMS"), "missing SHA256SUMS");
        assert!(yml.contains("v*.*.*") || yml.contains("v*.*.*"), "tag trigger");
        assert!(yml.contains("browse-"), "asset name prefix");
    }

    #[test]
    fn readme_has_curl_and_browse() {
        let md = std::fs::read_to_string("README.md").unwrap();
        assert!(md.contains("install.sh"), "missing install.sh");
        assert!(md.contains("curl -fsSL"), "missing curl one-liner");
        assert!(md.contains("browse update"), "missing browse update");
        assert!(md.contains("GitHub"), "mention GitHub check");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib update::tests::release_workflow_covers_four_targets update::tests::readme_has_curl_and_browse`

Expected: FAIL — `release.yml` missing and/or README lacks the strings.

- [ ] **Step 3: Write workflow + README**

Create `.github/workflows/release.yml`:

```yaml
name: release

on:
  push:
    tags:
      - "v*.*.*"

permissions:
  contents: write

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: macos-14
            target: aarch64-apple-darwin
          - os: macos-14
            target: x86_64-apple-darwin
          - os: ubuntu-22.04
            target: x86_64-unknown-linux-musl
          - os: ubuntu-22.04
            target: aarch64-unknown-linux-musl
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: musl linker
        if: endsWith(matrix.target, 'linux-musl')
        uses: taiki-e/setup-cross-toolchain-action@v1
        with:
          target: ${{ matrix.target }}
      - name: Build
        run: cargo build --release --target ${{ matrix.target }} --locked
      - name: Pack
        shell: bash
        run: |
          set -euo pipefail
          tag="${GITHUB_REF_NAME}"
          ver="${tag#v}"
          target="${{ matrix.target }}"
          bin="target/${target}/release/browse"
          test -f "$bin"
          stage="$(mktemp -d)"
          cp "$bin" "${stage}/browse"
          chmod 755 "${stage}/browse"
          asset="browse-${ver}-${target}.tar.gz"
          tar -c -z -f "$asset" -C "$stage" browse
          if command -v sha256sum >/dev/null; then
            sha256sum "$asset" > "${asset}.sha256"
          else
            shasum -a 256 "$asset" > "${asset}.sha256"
          fi
          mkdir -p dist
          mv "$asset" "${asset}.sha256" dist/
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.target }}
          path: dist/*

  publish:
    needs: build
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/download-artifact@v4
        with:
          path: artifacts
      - name: SHA256SUMS and release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          set -euo pipefail
          mkdir -p dist
          find artifacts -type f -name 'browse-*.tar.gz' -exec cp {} dist/ \;
          : > dist/SHA256SUMS
          for f in dist/browse-*.tar.gz; do
            base=$(basename "$f")
            sum=$(find artifacts -type f -name "${base}.sha256" | head -n 1)
            if [ -n "$sum" ]; then
              # normalize to "hex  filename"
              awk -v n="$base" '{print $1 "  " n}' "$sum" >> dist/SHA256SUMS
            else
              sha256sum "$f" | awk -v n="$base" '{print $1 "  " n}' >> dist/SHA256SUMS
            fi
          done
          gh release create "${GITHUB_REF_NAME}" \
            --title "${GITHUB_REF_NAME}" \
            --generate-notes \
            dist/browse-*.tar.gz dist/SHA256SUMS
```

Replace the README **Run** section with:

```markdown
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
```

Keep the existing Keys / What this is not / Strategy sections.

`--locked` requires `Cargo.lock` in git. If `Cargo.lock` is still untracked, `git add Cargo.lock` in this commit.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib update::tests`

Expected: all PASS, including workflow + README tests.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml README.md Cargo.lock
git commit -m "$(cat <<'EOF'
feat: publish GitHub Release binaries and document curl install

EOF
)"
```

---

## Plan self-review

| Spec requirement | Task |
|------------------|------|
| curl \| sh prebuilt `browse` | 7 |
| `browse` command name | 5 |
| Four targets + musl | 1, 8 |
| GitHub Releases + SHA256SUMS | 4, 8 |
| `~/.local/bin` + PATH hint | 7 |
| `browse update` | 4, 5 |
| Start Page 24h notice, non-blocking | 3, 6 |
| Atomic replace + checksum refuse | 2, 4 |
| Silent failed check, keep latest | 6 |
| Privacy: no TUI telemetry | all (no ping added) |
| README | 8 |
| Site / Supabase | none (parked) |
