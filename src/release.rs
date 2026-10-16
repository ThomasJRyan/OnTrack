use std::process::Command;

use semver::Version;

/// The official release feed for this repository. GitHub's latest endpoint
/// selects published stable releases (and ignores drafts and prereleases).
pub const RELEASE_API_URL: &str =
    "https://api.github.com/repos/ThomasJRyan/OnTrack/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    Available { version: Version, url: String },
    Current { version: Version },
    NoPublishedRelease,
}

pub fn check() -> Result<CheckResult, String> {
    check_with(fetch, env!("CARGO_PKG_VERSION"))
}

fn fetch(url: &str) -> Result<Option<String>, String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--connect-timeout",
            "3",
            "--max-time",
            "8",
            "--max-filesize",
            "262144",
            "--header",
            "Accept: application/vnd.github+json",
            "--user-agent",
            concat!("ontrack/", env!("CARGO_PKG_VERSION")),
            url,
        ])
        .output()
        .map_err(|error| format!("Could not start curl: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return if output.status.code() == Some(22) && detail.contains("404") {
            Ok(None)
        } else {
            Err(if detail.is_empty() {
                format!("GitHub request failed ({})", output.status)
            } else {
                format!("GitHub request failed: {detail}")
            })
        };
    }
    String::from_utf8(output.stdout)
        .map(Some)
        .map_err(|_| "GitHub returned non-UTF-8 data".into())
}

pub fn check_with(
    fetch: impl FnOnce(&str) -> Result<Option<String>, String>,
    current_version: &str,
) -> Result<CheckResult, String> {
    let current = Version::parse(current_version)
        .map_err(|error| format!("Invalid running version {current_version}: {error}"))?;
    let body = match fetch(RELEASE_API_URL)? {
        Some(body) => body,
        None => return Ok(CheckResult::NoPublishedRelease),
    };
    let release: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("Invalid GitHub release response: {error}"))?;
    let tag = release["tag_name"]
        .as_str()
        .ok_or("GitHub release has no tag_name")?;
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|error| format!("Invalid release version {tag}: {error}"))?;
    if version <= current {
        return Ok(CheckResult::Current { version: current });
    }
    let url = release["html_url"]
        .as_str()
        .filter(|url| url.starts_with("https://github.com/ThomasJRyan/OnTrack/releases/"))
        .ok_or("GitHub release has no valid release link")?;
    Ok(CheckResult::Available {
        version,
        url: url.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(tag: &str) -> String {
        format!(
            r#"{{"tag_name":"{tag}","html_url":"https://github.com/ThomasJRyan/OnTrack/releases/tag/{tag}"}}"#
        )
    }

    #[test]
    fn checks_official_source_and_semantic_order() {
        let result = check_with(
            |url| {
                assert_eq!(url, RELEASE_API_URL);
                Ok(Some(response("v1.10.0")))
            },
            "1.9.0",
        )
        .unwrap();
        assert_eq!(
            result,
            CheckResult::Available {
                version: Version::parse("1.10.0").unwrap(),
                url: "https://github.com/ThomasJRyan/OnTrack/releases/tag/v1.10.0".into(),
            }
        );
        assert!(matches!(
            check_with(|_| Ok(Some(response("v1.9.0"))), "1.9.0"),
            Ok(CheckResult::Current { .. })
        ));
        assert!(matches!(
            check_with(|_| Ok(Some(response("v1.8.0"))), "1.9.0"),
            Ok(CheckResult::Current { .. })
        ));
        assert!(matches!(
            check_with(|_| Ok(Some(response("v1.0.0"))), "1.0.0-rc.1"),
            Ok(CheckResult::Available { .. })
        ));
    }

    #[test]
    fn handles_missing_release_and_bad_responses() {
        assert_eq!(
            check_with(|_| Ok(None), "0.1.0"),
            Ok(CheckResult::NoPublishedRelease)
        );
        assert!(
            check_with(|_| Err("offline".into()), "0.1.0")
                .unwrap_err()
                .contains("offline")
        );
        assert!(check_with(|_| Ok(Some("{broken".into())), "0.1.0").is_err());
        assert!(check_with(|_| Ok(Some(response("nightly"))), "0.1.0").is_err());
        assert!(
            check_with(
                |_| Ok(Some(
                    r#"{"tag_name":"v2.0.0","html_url":"https://evil.example"}"#.into()
                )),
                "0.1.0"
            )
            .is_err()
        );
    }
}
