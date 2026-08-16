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
