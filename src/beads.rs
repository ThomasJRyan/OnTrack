//! Read-only browsing and explicit create/update/delete operations through the Beads CLI.

use std::io;
use std::process::{Command, Output};

use serde_json::Value;

pub const BEAD_TYPES: &[&str] = &[
    "task",
    "bug",
    "feature",
    "epic",
    "chore",
    "decision",
    "spike",
    "story",
    "milestone",
];

#[derive(Debug, Clone)]
pub struct CreateBead {
    pub title: String,
    pub description: String,
    pub issue_type: String,
    pub priority: u8,
}

#[derive(Debug, Clone)]
pub struct UpdateBead {
    pub id: String,
    pub fields: CreateBead,
}

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

pub fn create_bead(input: &CreateBead) -> io::Result<String> {
    let args = create_args(input)?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    parse_created_id(&run_bd(&args)?)
}

pub fn create_child(input: &CreateBead, parent_id: &str) -> io::Result<String> {
    let args = create_child_args(input, parent_id)?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    parse_created_id(&run_bd(&args)?)
}

fn create_child_args(input: &CreateBead, parent_id: &str) -> io::Result<Vec<String>> {
    validate_id(parent_id)?;
    let mut args = create_args(input)?;
    args.push(format!("--parent={parent_id}"));
    Ok(args)
}

/// Beads cannot add a blocking edge from a parent to its descendant: that
/// would cascade the blocked status back to the child. Check the hierarchy
/// itself before handing a parent to an agent.
pub fn open_descendants(parent_id: &str) -> io::Result<Vec<String>> {
    validate_id(parent_id)?;
    Ok(open_descendants_in(&list_beads()?, parent_id))
}

fn open_descendants_in(beads: &[Bead], parent_id: &str) -> Vec<String> {
    let mut descendants = Vec::new();
    let mut frontier = vec![parent_id];
    let mut visited = std::collections::HashSet::new();
    while let Some(parent) = frontier.pop() {
        if !visited.insert(parent) {
            continue;
        }
        for bead in beads {
            if bead.data["parent"].as_str() == Some(parent) {
                frontier.push(&bead.id);
                if bead.status != "closed" {
                    descendants.push(bead.id.clone());
                }
            }
        }
    }
    descendants
}

pub fn update_bead(input: &UpdateBead) -> io::Result<()> {
    let args = update_args(input)?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run_bd(&args)?;
    Ok(())
}

/// Permanently delete only this issue, never cascading to dependent issues.
pub fn delete_bead(id: &str) -> io::Result<()> {
    run_bd(&delete_args(id)?)?;
    Ok(())
}

fn create_args(input: &CreateBead) -> io::Result<Vec<String>> {
    let title = validated_title(input)?;
    Ok(vec![
        "--no-color".to_owned(),
        "create".to_owned(),
        format!("--title={title}"),
        format!("--description={}", input.description),
        format!("--type={}", input.issue_type),
        format!("--priority={}", input.priority),
        "--json".to_owned(),
    ])
}

fn update_args(input: &UpdateBead) -> io::Result<Vec<String>> {
    validate_id(&input.id)?;
    let title = validated_title(&input.fields)?;
    Ok(vec![
        "--no-color".to_owned(),
        "update".to_owned(),
        format!("--title={title}"),
        format!("--description={}", input.fields.description),
        format!("--type={}", input.fields.issue_type),
        format!("--priority={}", input.fields.priority),
        "--json".to_owned(),
        "--".to_owned(),
        input.id.clone(),
    ])
}

fn validated_title(input: &CreateBead) -> io::Result<&str> {
    let title = input.title.trim();
    if title.is_empty()
        || input
            .title
            .contains(['\n', '\r', '\0', '\u{85}', '\u{2028}', '\u{2029}'])
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Beads title must be nonempty and single-line",
        ));
    }
    if input.priority > 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Beads priority must be between 0 and 4",
        ));
    }
    if !BEAD_TYPES.contains(&input.issue_type.as_str()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsupported Beads issue type",
        ));
    }
    Ok(title)
}

fn delete_args(id: &str) -> io::Result<[&str; 6]> {
    validate_id(id)?;
    Ok(["--no-color", "delete", "--force", "--json", "--", id])
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
    if id.is_empty()
        || id.starts_with('-')
        || id.chars().any(|ch| ch.is_whitespace() || ch.is_control())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Beads issue ID must be nonempty, not start with '-', and contain no whitespace or control characters",
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

fn parse_created_id(bytes: &[u8]) -> io::Result<String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| invalid_data(format!("Invalid bd JSON: {error}")))?;
    if !value.is_object() {
        return Err(invalid_data("Expected a JSON object from bd create"));
    }
    let id = required_string(&value, "id")?;
    validate_id(&id).map_err(|error| invalid_data(format!("Invalid created issue ID: {error}")))?;
    Ok(id)
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
    fn child_creation_keeps_parent_and_user_text_in_separate_arguments() {
        let args = create_child_args(
            &CreateBead {
                title: "$(touch /tmp/unwanted)".into(),
                description: "--priority=0\nsecond line".into(),
                issue_type: "task".into(),
                priority: 2,
            },
            "ontrack-parent",
        )
        .unwrap();
        assert!(args.contains(&"--parent=ontrack-parent".to_owned()));
        assert!(args.contains(&"--title=$(touch /tmp/unwanted)".to_owned()));
        assert!(args.contains(&"--description=--priority=0\nsecond line".to_owned()));
        assert!(create_child_args(&create_input(), "--help").is_err());
    }

    #[test]
    fn open_descendants_follow_beads_hierarchy_through_closed_intermediates() {
        let rows = json!([
            {"id":"parent","title":"Parent","status":"open"},
            {"id":"child","title":"Child","status":"closed","parent":"parent"},
            {"id":"grandchild","title":"Grandchild","status":"open","parent":"child"},
            {"id":"unrelated","title":"Elsewhere","status":"open"}
        ]);
        let beads = parse_list(&serde_json::to_vec(&rows).unwrap()).unwrap();
        assert_eq!(open_descendants_in(&beads, "parent"), vec!["grandchild"]);
    }

    fn create_input() -> CreateBead {
        CreateBead {
            title: "  A new issue  ".to_owned(),
            description: "First line\n\nSecond line\r\n  indented  \n".to_owned(),
            issue_type: "task".to_owned(),
            priority: 2,
        }
    }

    #[test]
    fn create_args_trim_title_and_preserve_multiline_description() {
        assert_eq!(
            create_args(&create_input()).unwrap(),
            [
                "--no-color",
                "create",
                "--title=A new issue",
                "--description=First line\n\nSecond line\r\n  indented  \n",
                "--type=task",
                "--priority=2",
                "--json",
            ]
        );
    }

    #[test]
    fn create_keeps_flag_like_and_shell_text_in_single_arguments() {
        let mut input = create_input();
        input.title = "--force $(touch ignored); 'quoted'".to_owned();
        input.description = "--cascade\n\"; $(touch ignored)".to_owned();
        let args = create_args(&input).unwrap();
        assert_eq!(args.len(), 7);
        assert_eq!(args[2], format!("--title={}", input.title));
        assert_eq!(args[3], format!("--description={}", input.description));
        assert!(!args.iter().any(|arg| arg == "--readonly"));
    }

    #[test]
    fn update_targets_one_id_and_keeps_user_text_in_argument_values() {
        let mut fields = create_input();
        fields.title = "--force $(touch ignored); 'quoted'".into();
        fields.description = "First line\n--status=closed; $(touch ignored)".into();
        let input = UpdateBead {
            id: "ontrack-70t".into(),
            fields,
        };
        assert_eq!(
            update_args(&input).unwrap(),
            [
                "--no-color",
                "update",
                "--title=--force $(touch ignored); 'quoted'",
                "--description=First line\n--status=closed; $(touch ignored)",
                "--type=task",
                "--priority=2",
                "--json",
                "--",
                "ontrack-70t",
            ]
        );
    }

    #[test]
    fn update_rejects_invalid_id_and_title_before_running_bd() {
        let mut input = UpdateBead {
            id: "--force".into(),
            fields: create_input(),
        };
        assert_eq!(
            update_bead(&input).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        input.id = "ontrack-70t".into();
        input.fields.title = "first\nsecond".into();
        assert_eq!(
            update_bead(&input).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn create_accepts_all_core_types_and_priorities() {
        assert_eq!(
            BEAD_TYPES,
            [
                "task",
                "bug",
                "feature",
                "epic",
                "chore",
                "decision",
                "spike",
                "story",
                "milestone"
            ]
        );
        for issue_type in BEAD_TYPES {
            for priority in 0..=4 {
                let mut input = create_input();
                input.issue_type = (*issue_type).to_owned();
                input.priority = priority;
                input.description.clear();
                let args = create_args(&input).unwrap();
                assert_eq!(args[4], format!("--type={issue_type}"));
                assert_eq!(args[5], format!("--priority={priority}"));
            }
        }
    }

    #[test]
    fn create_rejects_invalid_inputs_before_running_a_process() {
        for title in [
            "",
            "  \t ",
            "first\nsecond",
            "first\rsecond",
            "title\n",
            "\n title",
            "a\0b",
            "a\u{85}b",
            "a\u{2028}b",
            "a\u{2029}b",
        ] {
            let mut input = create_input();
            input.title = title.to_owned();
            assert_eq!(
                create_bead(&input).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        for priority in [5, u8::MAX] {
            let mut input = create_input();
            input.priority = priority;
            assert_eq!(
                create_bead(&input).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        for issue_type in ["", "custom", "Task", "task ", "--force", "task\n--force"] {
            let mut input = create_input();
            input.issue_type = issue_type.to_owned();
            assert_eq!(
                create_bead(&input).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    fn create_parses_object_id_and_rejects_malformed_output() {
        assert_eq!(
            parse_created_id(br#"{"id":"ontrack-70t","title":"New issue","future":true}"#).unwrap(),
            "ontrack-70t"
        );
        for input in [
            "",
            "not json",
            "null",
            "[]",
            "{}",
            r#"{"id":null}"#,
            r#"{"id":3}"#,
            r#"{"id":""}"#,
            r#"{"id":"--force"}"#,
            r#"{"id":"a b"}"#,
            r#"{"id":"a\nb"}"#,
        ] {
            assert_eq!(
                parse_created_id(input.as_bytes()).unwrap_err().kind(),
                io::ErrorKind::InvalidData,
                "input: {input}"
            );
        }
    }

    #[test]
    fn delete_args_force_only_selected_issue_without_cascade() {
        assert_eq!(
            delete_args("ontrack-70t").unwrap(),
            [
                "--no-color",
                "delete",
                "--force",
                "--json",
                "--",
                "ontrack-70t"
            ]
        );
        for id in ["ontrack-70t", "ontrack-70t.1", "workspace:a-b", "a", "123"] {
            validate_id(id).unwrap();
        }
    }

    #[test]
    fn delete_and_show_reject_unsafe_ids_before_running_a_process() {
        for id in [
            "", " ", "--force", "-a", "a b", "a\nb", "a\tb", "a\0b", "a\u{7f}b", "a\u{a0}b",
        ] {
            assert_eq!(
                delete_bead(id).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "id: {id:?}"
            );
            assert_eq!(
                show_bead(id).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "id: {id:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn mutation_command_errors_and_create_success_use_injected_executor() {
        use std::os::unix::process::ExitStatusExt;

        let owned_args = create_args(&create_input()).unwrap();
        let create_args: Vec<&str> = owned_args.iter().map(String::as_str).collect();
        let delete_args = delete_args("ontrack-70t").unwrap();
        for args in [create_args.as_slice(), delete_args.as_slice()] {
            let error = run_bd_with(args, |actual| {
                assert_eq!(actual, args);
                Ok(Output {
                    status: std::process::ExitStatus::from_raw(1 << 8),
                    stdout: b"{}".to_vec(),
                    stderr: b"mutation rejected\n".to_vec(),
                })
            })
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert!(error.to_string().contains("exit status: 1"));
            assert!(error.to_string().contains("mutation rejected"));

            let error = run_bd_with(args, |_| {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "permission denied",
                ))
            })
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(error.to_string().contains("Could not run bd"));
        }
        let bytes = run_bd_with(&create_args, |_| {
            Ok(Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: br#"{"id":"ontrack-new"}"#.to_vec(),
                stderr: Vec::new(),
            })
        })
        .unwrap();
        assert_eq!(parse_created_id(&bytes).unwrap(), "ontrack-new");
    }

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
