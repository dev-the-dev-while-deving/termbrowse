//! Self-update helpers. GitHub Releases only. No telemetry.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
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
    let browse = dir.join("browse");
    let meta = match fs::symlink_metadata(&browse) {
        Ok(m) => m,
        Err(_) => {
            let _ = fs::remove_dir_all(&dir);
            bail!("tar extract failed");
        }
    };
    let ft = meta.file_type();
    if ft.is_symlink() || !ft.is_file() {
        let _ = fs::remove_dir_all(&dir);
        bail!("tar extract failed");
    }
    let bytes = fs::read(&browse);
    let _ = fs::remove_dir_all(&dir);
    bytes.context("read extracted browse")
}

pub fn install_tarball(tarball: &[u8], dest: &Path) -> Result<()> {
    let bin = extract_browse_from_tarball(tarball)?;
    atomic_replace(dest, &bin)
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn detect_target_matches_host() {
        let t = detect_target().expect("this CI/dev host must be a supported target");
        assert!(
            t.ends_with("apple-darwin") || t.ends_with("linux-musl"),
            "{t}"
        );
    }

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

    #[test]
    fn extract_refuses_symlink_browse() {
        let dir = std::env::temp_dir().join(format!(
            "browse-tar-symlink-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&dir);
        std::os::unix::fs::symlink("/etc/passwd", dir.join("browse")).unwrap();
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
        let err = extract_browse_from_tarball(&tarball).unwrap_err().to_string();
        assert!(
            err.contains("tar extract failed") || err.contains("symlink") || err.contains("not a regular file"),
            "{err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

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
        fn get_text(&self, url: &str) -> anyhow::Result<String> {
            self.text
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no text {url}"))
        }
        fn get_bytes(&self, url: &str) -> anyhow::Result<Vec<u8>> {
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
}
