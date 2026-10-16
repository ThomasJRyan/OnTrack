//! Launch-context checks and read-only workspace discovery, completed before raw mode.
use std::{
    ffi::OsStr,
    io::{self, IsTerminal, Write},
    process::{Command, Output},
};

use crossterm::style::Stylize;

pub fn ensure_herdr_workspace() -> io::Result<()> {
    require_herdr_workspace(std::env::var_os("HERDR_ENV").as_deref())
}

fn require_herdr_workspace(marker: Option<&OsStr>) -> io::Result<()> {
    if marker == Some(OsStr::new("1")) {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "OnTrack must be launched inside a Herdr workspace. Open a Herdr workspace and run ontrack from its shell.",
    ))
}

pub fn ensure_workspace() -> io::Result<bool> {
    let directory = std::env::current_dir()?;
    let result = setup(
        io::stdout().is_terminal(),
        &mut io::stdout().lock(),
        |args| {
            Command::new("bd")
                .args(args)
                .current_dir(&directory)
                .output()
        },
    );
    result.map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Beads startup in {}: {error}. Check bd and workspace permissions, then retry",
                directory.display()
            ),
        )
    })
}

fn setup(
    highlight: bool,
    output: &mut impl Write,
    mut execute: impl FnMut(&[&str]) -> io::Result<Output>,
) -> io::Result<bool> {
    let discovery = execute(&["--no-color", "where", "--json"])?;
    if workspace_exists(&discovery)? {
        return Ok(true);
    }
    let command = "bd init";
    if highlight {
        writeln!(
            output,
            "No Beads workspace found. Run {} in your project, then launch OnTrack again.",
            command.bold()
        )?;
    } else {
        writeln!(
            output,
            "No Beads workspace found. Run {command} in your project, then launch OnTrack again."
        )?;
    }
    output.flush()?;
    Ok(false)
}

fn workspace_exists(output: &Output) -> io::Result<bool> {
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok();
    if output.status.success() {
        // Discovery belongs to bd, including ancestors, redirects and worktrees.
        if value
            .as_ref()
            .and_then(|value| value["path"].as_str())
            .is_some_and(|path| !path.trim().is_empty())
        {
            return Ok(true);
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bd where returned invalid workspace JSON; expected a nonempty path",
        ));
    }
    if value.as_ref().and_then(|value| value["error"].as_str()) == Some("no_beads_directory") {
        return Ok(false);
    }
    Err(io::Error::other(format!(
        "Could not discover Beads workspace: {} {}",
        String::from_utf8_lossy(&output.stderr).trim(),
        String::from_utf8_lossy(&output.stdout).trim()
    )))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn herdr_workspace_requires_the_managed_pane_marker() {
        assert!(require_herdr_workspace(Some(OsStr::new("1"))).is_ok());

        for marker in [
            None,
            Some(OsStr::new("")),
            Some(OsStr::new("0")),
            Some(OsStr::new("true")),
        ] {
            let error = require_herdr_workspace(marker).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(error.to_string().contains("Open a Herdr workspace"));
        }
    }

    fn response(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn missing_workspace_prints_guidance_and_exits_without_other_commands() {
        for highlight in [false, true] {
            let mut calls = 0;
            let mut transcript = Vec::new();
            assert!(
                !setup(highlight, &mut transcript, |args| {
                    calls += 1;
                    assert_eq!(args, &["--no-color", "where", "--json"]);
                    Ok(response(1, r#"{"error":"no_beads_directory"}"#, ""))
                })
                .unwrap()
            );
            assert_eq!(calls, 1);
            let transcript = String::from_utf8(transcript).unwrap();
            assert!(transcript.contains("No Beads workspace found. Run "));
            assert!(transcript.contains("bd init"));
            assert!(transcript.ends_with("then launch OnTrack again.\n"));
            if highlight {
                assert!(transcript.contains("\x1b[1mbd init\x1b[0m"));
            } else {
                assert!(!transcript.contains('\x1b'));
            }
        }
    }

    #[test]
    fn existing_inherited_or_redirected_workspace_has_no_writes_or_prompts() {
        for path in [
            "/project/.beads",
            "/ancestor/.beads",
            "/linked-worktree/.beads",
        ] {
            let mut calls = 0;
            let mut transcript = Vec::new();
            assert!(
                setup(false, &mut transcript, |args| {
                    calls += 1;
                    assert_eq!(args, &["--no-color", "where", "--json"]);
                    Ok(response(
                        0,
                        &serde_json::json!({"path": path}).to_string(),
                        "",
                    ))
                })
                .unwrap()
            );
            assert_eq!(calls, 1);
            assert!(transcript.is_empty());
        }
    }

    #[test]
    fn malformed_successful_discovery_is_an_error() {
        for stdout in ["", "bad json", "{}", r#"{"path":" "}"#] {
            let mut transcript = Vec::new();
            assert_eq!(
                setup(false, &mut transcript, |_| Ok(response(0, stdout, "")))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
            assert!(transcript.is_empty());
        }
    }

    #[test]
    fn discovery_errors_do_not_print_initialization_guidance() {
        for (stdout, stderr) in [
            ("", "permission denied"),
            ("broken json", "bad redirect"),
            (r#"{"error":"database_error"}"#, "Dolt unavailable"),
        ] {
            let mut calls = 0;
            let mut transcript = Vec::new();
            let error = setup(true, &mut transcript, |_| {
                calls += 1;
                Ok(response(1, stdout, stderr))
            })
            .unwrap_err();
            assert_eq!(calls, 1);
            assert!(error.to_string().contains(stderr));
            assert!(transcript.is_empty());
        }
    }

    #[test]
    fn unavailable_cli_is_an_error() {
        let mut transcript = Vec::new();
        let error = setup(false, &mut transcript, |_| {
            Err(io::Error::new(io::ErrorKind::NotFound, "bd unavailable"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(transcript.is_empty());
    }
}
