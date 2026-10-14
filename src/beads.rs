//! Read-only access to Beads through its public CLI.

use std::io;
use std::process::{Command, Output};

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Bead {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: u64,
    pub data: Value,
}

const LIST_ARGS: &[&str] = &[
    "--readonly",
    "--no-color",
    "list",
    "--all",
    "--limit",
    "0",
    "--json",
];

pub fn list_beads() -> io::Result<Vec<Bead>> {
    parse_list(&run_bd(LIST_ARGS)?)
}

pub fn show_bead(id: &str) -> io::Result<Value> {
    validate_id(id)?;
    let id_arg = format!("--id={id}");
    let args = show_args(&id_arg);
    parse_show(&run_bd(&args)?, id)
}

fn show_args(id_arg: &str) -> [&str; 8] {
    [
        "--readonly",
        "--no-color",
        "show",
        id_arg,
        "--json",
        "--long",
        "--include-comments",
        "--include-dependents",
    ]
}

fn validate_id(id: &str) -> io::Result<()> {
    if id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Beads issue ID must not be empty",
        ));
    }
    Ok(())
}

fn run_bd(args: &[&str]) -> io::Result<Vec<u8>> {
    run_bd_with(args, |args| Command::new("bd").args(args).output())
}

fn run_bd_with(
    args: &[&str],
    execute: impl FnOnce(&[&str]) -> io::Result<Output>,
) -> io::Result<Vec<u8>> {
    let output = execute(args)
        .map_err(|error| io::Error::new(error.kind(), format!("Could not run bd: {error}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!(
            "bd failed ({}): {}",
            output.status,
            stderr.trim()
        )));
    }
    Ok(output.stdout)
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn parse_records(bytes: &[u8]) -> io::Result<Vec<Value>> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| invalid_data(format!("Invalid bd JSON: {error}")))?;
    match value {
        Value::Array(records) => Ok(records),
        _ => Err(invalid_data("Expected a JSON array from bd")),
    }
}

fn required_string(record: &Value, field: &str) -> io::Result<String> {
    record
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid_data(format!("Beads record has missing or invalid {field}")))
}

fn parse_list(bytes: &[u8]) -> io::Result<Vec<Bead>> {
    parse_records(bytes)?
        .into_iter()
        .map(|data| {
            let id = required_string(&data, "id")?;
            let title = required_string(&data, "title")?;
            let status = required_string(&data, "status")?;
            let priority = match data.get("priority") {
                None | Some(Value::Null) => 2,
                Some(value) => value
                    .as_u64()
                    .ok_or_else(|| invalid_data("Beads record has invalid priority"))?,
            };
            Ok(Bead {
                id,
                title,
                status,
                priority,
                data,
            })
        })
        .collect()
}

fn parse_show(bytes: &[u8], id: &str) -> io::Result<Value> {
    let mut found = None;
    for record in parse_records(bytes)? {
        if required_string(&record, "id")? == id {
            if found.is_some() {
                return Err(invalid_data(format!("bd returned duplicate issue {id}")));
            }
            found = Some(record);
        }
    }
    found.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("bd did not return requested issue {id}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn list_args_are_read_only_uncapped_and_include_closed() {
        assert_eq!(
            LIST_ARGS,
            [
                "--readonly",
                "--no-color",
                "list",
                "--all",
                "--limit",
                "0",
                "--json"
            ]
        );
    }

    #[test]
    fn show_args_request_full_details_and_keep_id_in_one_argument() {
        assert_eq!(
            show_args("--id=ontrack-ow6"),
            [
                "--readonly",
                "--no-color",
                "show",
                "--id=ontrack-ow6",
                "--json",
                "--long",
                "--include-comments",
                "--include-dependents",
            ]
        );
    }

    #[test]
    fn list_preserves_full_records_and_defaults_priority() {
        let records = json!([
            {"id": "a", "title": "First", "status": "closed", "priority": 0,
             "future_metadata": {"revision": 3}},
            {"id": "b", "title": "Second", "status": "open"},
            {"id": "c", "title": "Third", "status": "in_progress", "priority": null}
        ]);
        let beads = parse_list(&serde_json::to_vec(&records).unwrap()).unwrap();
        assert_eq!(beads.len(), 3);
        assert_eq!(beads[0].id, "a");
        assert_eq!(beads[0].title, "First");
        assert_eq!(beads[0].status, "closed");
        assert_eq!(beads[0].priority, 0);
        assert_eq!(beads[0].data, records[0]);
        assert_eq!(beads[1].priority, 2);
        assert_eq!(beads[2].priority, 2);
        assert!(parse_list(b"[]").unwrap().is_empty());
    }

    #[test]
    fn list_rejects_malformed_json_and_records_without_panicking() {
        for bytes in [
            "",
            "not json",
            "{}",
            "null",
            "[null]",
            "[{}]",
            r#"[{"id":"","title":"x","status":"open"}]"#,
            r#"[{"id":"a","title":3,"status":"open"}]"#,
            r#"[{"id":"a","title":"x","status":""}]"#,
            r#"[{"id":"a","title":"x","status":"open","priority":-1}]"#,
            r#"[{"id":"a","title":"x","status":"open","priority":"high"}]"#,
        ] {
            assert_eq!(
                parse_list(bytes.as_bytes()).unwrap_err().kind(),
                io::ErrorKind::InvalidData,
                "input: {bytes}"
            );
        }
    }

    #[test]
    fn show_selects_requested_record_and_preserves_unknown_fields() {
        let records = json!([
            {"id": "other"},
            {"id": "wanted", "revision": 4, "dependency_count": 2,
             "metadata": {"future": true}, "comments": [{"text": "hello"}],
             "dependents": [{"id": "other"}]}
        ]);
        let result = parse_show(&serde_json::to_vec(&records).unwrap(), "wanted").unwrap();
        assert_eq!(result, records[1]);
    }

    #[test]
    fn show_rejects_missing_duplicate_or_malformed_records() {
        for input in ["[]", r#"[{"id":"other"}]"#] {
            assert_eq!(
                parse_show(input.as_bytes(), "wanted").unwrap_err().kind(),
                io::ErrorKind::NotFound
            );
        }
        for input in [
            "",
            "{}",
            "[null]",
            "[{}]",
            r#"[{"id":3}]"#,
            r#"[{"id":"wanted"},{"id":"wanted"}]"#,
        ] {
            assert_eq!(
                parse_show(input.as_bytes(), "wanted").unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn show_rejects_empty_id_before_running_a_process() {
        for id in ["", " \n "] {
            assert_eq!(
                show_bead(id).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    fn missing_executable_keeps_not_found_kind_and_adds_context() {
        let error = run_bd_with(LIST_ARGS, |args| {
            assert_eq!(args, LIST_ARGS);
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "missing executable",
            ))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("Could not run bd"));
        assert!(error.to_string().contains("missing executable"));
    }

    #[cfg(unix)]
    #[test]
    fn command_failure_includes_stderr_and_success_returns_stdout() {
        use std::os::unix::process::ExitStatusExt;

        let error = run_bd_with(LIST_ARGS, |_| {
            Ok(Output {
                status: std::process::ExitStatus::from_raw(1 << 8),
                stdout: b"[]".to_vec(),
                stderr: b"database unavailable\n".to_vec(),
            })
        })
        .unwrap_err();
        assert!(error.to_string().contains("exit status: 1"));
        assert!(error.to_string().contains("database unavailable"));

        let bytes = run_bd_with(LIST_ARGS, |_| {
            Ok(Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"[]".to_vec(),
                stderr: Vec::new(),
            })
        })
        .unwrap();
        assert_eq!(bytes, b"[]");
    }
}
