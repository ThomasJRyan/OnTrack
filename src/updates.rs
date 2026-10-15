use std::process::Command;

const LATEST_RELEASE: &str = "https://api.github.com/repos/ThomasJRyan/OnTrack/releases/latest";

/// This is best effort: an offline session, missing curl, or no published
/// release should leave the normal browser usable without a warning.
pub fn available_version() -> Option<String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "5",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "User-Agent: ontrack",
            LATEST_RELEASE,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    newer_release(&output.stdout, env!("CARGO_PKG_VERSION"))
}

fn newer_release(body: &[u8], current: &str) -> Option<String> {
    let release: serde_json::Value = serde_json::from_slice(body).ok()?;
    let tag = release.get("tag_name")?.as_str()?;
    let version = tag.strip_prefix('v').unwrap_or(tag);
    let latest = semver::Version::parse(version).ok()?;
    let current = semver::Version::parse(current).ok()?;
    (latest > current).then(|| version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::newer_release;

    #[test]
    fn reports_only_newer_valid_release_tags() {
        assert_eq!(
            newer_release(br#"{"tag_name":"v0.2.0"}"#, "0.1.0"),
            Some("0.2.0".into())
        );
        assert_eq!(
            newer_release(br#"{"tag_name":"1.0.0"}"#, "1.0.0-beta.1"),
            Some("1.0.0".into())
        );
        for body in [
            br#"{"tag_name":"v0.1.0"}"#.as_slice(),
            br#"{"tag_name":"v0.0.9"}"#,
            br#"{"tag_name":"latest"}"#,
            br#"{"tag_name":null}"#,
            br#"{}"#,
            br#"not JSON"#,
        ] {
            assert_eq!(newer_release(body, "0.1.0"), None);
        }
        assert_eq!(newer_release(br#"{"tag_name":"v0.2.0"}"#, "bad"), None);
    }
}
