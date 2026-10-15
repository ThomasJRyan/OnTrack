//! Workspace initialization, read-only browsing, and explicit mutations through the Beads CLI.

use std::process::{Command, Output};
use std::{io, path::Path};

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
pub const READY_LABEL: &str = "ontrack:ready";
pub const HARNESS_PREFIX: &str = "ontrack:agent:";
pub const UNASSIGNED_LABEL: &str = "ontrack:unassigned";
pub const HARNESSES: &[&str] = &[
    "pi",
    "claude",
    "codex",
    "gemini",
    "cursor",
    "devin",
    "agy",
    "cline",
    "omp",
    "mastracode",
    "opencode",
    "copilot",
    "kimi",
    "kiro",
    "droid",
    "amp",
    "grok",
    "hermes",
    "kilo",
    "qodercli",
    "qwen",
    "letta",
    "maki",
    "muse",
];

#[derive(Debug, Clone)]
pub struct CreateBead {
    pub title: String,
    pub description: String,
    pub issue_type: String,
    pub priority: u8,
    pub harness: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateBead {
    pub id: String,
    pub fields: CreateBead,
    pub previous_harness: Option<String>,
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

const INIT_ARGS: &[&str] = &[
    "--no-color",
    "init",
    "--init-if-missing",
    "--non-interactive",
    "--skip-agents",
    "--skip-hooks",
    "--quiet",
];

/// Ensure the launch directory can use Beads before the terminal enters raw mode.
pub fn ensure_workspace() -> io::Result<()> {
    ensure_workspace_in(&std::env::current_dir()?)
}

fn ensure_workspace_in(directory: &Path) -> io::Result<()> {
    ensure_workspace_in_with(directory, |directory, args| {
        Command::new("bd")
            .args(args)
            .current_dir(directory)
            .output()
    })
}

fn ensure_workspace_in_with(
    directory: &Path,
    execute: impl FnOnce(&Path, &[&str]) -> io::Result<Output>,
) -> io::Result<()> {
    run_bd_with(INIT_ARGS, |args| execute(directory, args)).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Could not initialize Beads for {}: {error}. Install or check bd, then retry",
                directory.display()
            ),
        )
    })?;
    Ok(())
}

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
    validate_id(parent_id)?;
    let parent = show_bead(parent_id)?;
    let inherited = labels(&parent)
        .filter(|label| {
            *label != READY_LABEL
                && *label != UNASSIGNED_LABEL
                && !label.starts_with(HARNESS_PREFIX)
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let args = create_child_args(input, parent_id, &inherited)?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    parse_created_id(&run_bd(&args)?)
}

fn create_child_args(
    input: &CreateBead,
    parent_id: &str,
    inherited: &[String],
) -> io::Result<Vec<String>> {
    validate_id(parent_id)?;
    let mut args = create_args(input)?;
    args.push(format!("--parent={parent_id}"));
    args.push("--no-inherit-labels".into());
    for label in inherited {
        if label == READY_LABEL || label == UNASSIGNED_LABEL || label.starts_with(HARNESS_PREFIX) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OnTrack parent state cannot be inherited by child",
            ));
        }
        args.push(format!("--labels={label}"));
    }
    Ok(args)
}

/// Beads cannot add a blocking edge from a parent to its descendant: that
/// would cascade the blocked status back to the child. Check the hierarchy
/// itself before handing a parent to an agent.
pub fn open_descendants(parent_id: &str) -> io::Result<Vec<String>> {
    validate_id(parent_id)?;
    Ok(open_descendants_in(&list_beads()?, parent_id))
}

pub(crate) fn open_descendants_in(beads: &[Bead], parent_id: &str) -> Vec<String> {
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

pub fn set_ready(id: &str, ready: bool) -> io::Result<()> {
    set_ready_many(&[id.to_owned()], ready)
}

fn set_ready_many(ids: &[String], ready: bool) -> io::Result<()> {
    for id in ids {
        validate_id(id)?;
    }
    let flag = if ready {
        "--add-label"
    } else {
        "--remove-label"
    };
    let mut args = vec!["--no-color", "update", flag, READY_LABEL];
    if ready {
        args.extend(["--if-status", "open", "--status", "open"]);
    }
    args.push("--");
    args.extend(ids.iter().map(String::as_str));
    run_bd(&args)?;
    Ok(())
}

/// Change an Epic and its open descendant Tasks in one CLI invocation.
/// Beads applies updates per ID, so callers must refresh after any error.
pub fn set_epic_ready(
    epic_id: &str,
    ready: bool,
    known_handoffs: &std::collections::HashSet<String>,
) -> io::Result<Vec<String>> {
    validate_id(epic_id)?;
    let beads = list_beads()?;
    let ids = epic_ready_targets_in(&beads, epic_id, ready, known_handoffs)?;
    set_ready_many(&ids, ready)?;
    Ok(ids)
}

fn epic_ready_targets_in(
    beads: &[Bead],
    epic_id: &str,
    ready: bool,
    known_handoffs: &std::collections::HashSet<String>,
) -> io::Result<Vec<String>> {
    let epic = beads
        .iter()
        .find(|bead| bead.id == epic_id)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Selected Epic is no longer listed")
        })?;
    if epic.status != "open" || epic.data["issue_type"] != "epic" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Only open Epics can change descendant Ready markers",
        ));
    }
    let descendants: std::collections::HashSet<_> =
        open_descendants_in(beads, epic_id).into_iter().collect();
    let mut ids = vec![epic_id.to_owned()];
    ids.extend(
        beads
            .iter()
            .filter(|bead| {
                descendants.contains(&bead.id)
                    && bead.status == "open"
                    && bead.data["issue_type"] == "task"
                    && (!ready || !known_handoffs.contains(&bead.id))
                    && (ready != is_ready(bead))
            })
            .map(|bead| bead.id.clone()),
    );
    Ok(ids)
}

pub fn is_ready(bead: &Bead) -> bool {
    bead.status == "open"
        && (bead.data["issue_type"] == "task" || bead.data["issue_type"] == "epic")
        && labels(&bead.data).any(|label| label == READY_LABEL)
}

pub fn ready_record(data: &Value) -> bool {
    data["status"] == "open"
        && data["issue_type"] == "task"
        && labels(data).any(|label| label == READY_LABEL)
}

pub fn harness(data: &Value) -> Option<&str> {
    labels(data)
        .filter_map(|label| label.strip_prefix(HARNESS_PREFIX))
        .find(|kind| HARNESSES.contains(kind))
}

pub fn explicitly_unassigned(data: &Value) -> bool {
    labels(data).any(|label| label == UNASSIGNED_LABEL)
}

pub fn has_harness_label(data: &Value) -> bool {
    labels(data).any(|label| label.starts_with(HARNESS_PREFIX))
}

fn labels(data: &Value) -> impl Iterator<Item = &str> {
    data.get("labels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

/// Permanently delete only this issue, never cascading to dependent issues.
pub fn delete_bead(id: &str) -> io::Result<()> {
    run_bd(&delete_args(id)?)?;
    Ok(())
}

fn create_args(input: &CreateBead) -> io::Result<Vec<String>> {
    let title = validated_title(input)?;
    let mut args = vec![
        "--no-color".to_owned(),
        "create".to_owned(),
        format!("--title={title}"),
        format!("--description={}", input.description),
        format!("--type={}", input.issue_type),
        format!("--priority={}", input.priority),
        "--json".to_owned(),
    ];
    if input.issue_type == "task" {
        args.push(match &input.harness {
            Some(harness) => format!("--labels={HARNESS_PREFIX}{harness}"),
            None => format!("--labels={UNASSIGNED_LABEL}"),
        });
    }
    Ok(args)
}

fn update_args(input: &UpdateBead) -> io::Result<Vec<String>> {
    validate_id(&input.id)?;
    let title = validated_title(&input.fields)?;
    let mut args = vec![
        "--no-color".to_owned(),
        "update".to_owned(),
        format!("--title={title}"),
        format!("--description={}", input.fields.description),
        format!("--type={}", input.fields.issue_type),
        format!("--priority={}", input.fields.priority),
        "--json".to_owned(),
    ];
    if input.fields.harness != input.previous_harness {
        if let Some(old) = &input.previous_harness {
            args.push(format!("--remove-label={HARNESS_PREFIX}{old}"));
        }
        if let Some(new) = &input.fields.harness {
            args.push(format!("--add-label={HARNESS_PREFIX}{new}"));
        }
    }
    if input.fields.issue_type == "task" && input.fields.harness.is_none() {
        args.push(format!("--add-label={UNASSIGNED_LABEL}"));
    } else {
        args.push(format!("--remove-label={UNASSIGNED_LABEL}"));
    }
    args.extend(["--".to_owned(), input.id.clone()]);
    Ok(args)
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
    if input
        .harness
        .as_deref()
        .is_some_and(|h| !HARNESSES.contains(&h))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsupported agent harness",
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
    fn startup_initializes_missing_workspace_and_preserves_existing_one() {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).ok();
            }
        }

        let directory = std::env::temp_dir().join(format!(
            "ontrack-beads-init-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let _fixture = Fixture(directory.clone());

        ensure_workspace_in(&directory).unwrap();
        assert!(directory.join(".beads").is_dir());
        assert!(!directory.join("AGENTS.md").exists());

        let marker = directory.join(".beads/ontrack-marker");
        std::fs::write(&marker, b"keep").unwrap();
        ensure_workspace_in(&directory).unwrap();
        assert_eq!(std::fs::read(&marker).unwrap(), b"keep");

        let nested = directory.join("nested");
        std::fs::create_dir(&nested).unwrap();
        ensure_workspace_in(&nested).unwrap();
        assert!(!nested.join(".beads").exists());
    }

    #[test]
    fn startup_failure_is_reported_before_terminal_setup() {
        let directory = Path::new("/missing-workspace");
        let error = ensure_workspace_in_with(directory, |actual_directory, args| {
            assert_eq!(actual_directory, directory);
            assert_eq!(args, INIT_ARGS);
            Err(io::Error::new(io::ErrorKind::NotFound, "bd not installed"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("/missing-workspace"));
        assert!(error.to_string().contains("Install or check bd"));
    }

    #[test]
    fn child_creation_keeps_parent_and_user_text_in_separate_arguments() {
        let args = create_child_args(
            &CreateBead {
                title: "$(touch /tmp/unwanted)".into(),
                description: "--priority=0\nsecond line".into(),
                issue_type: "task".into(),
                priority: 2,
                harness: None,
            },
            "ontrack-parent",
            &[],
        )
        .unwrap();
        assert!(args.contains(&"--parent=ontrack-parent".to_owned()));
        assert!(args.contains(&"--no-inherit-labels".to_owned()));
        assert!(args.contains(&"--title=$(touch /tmp/unwanted)".to_owned()));
        assert!(args.contains(&"--description=--priority=0\nsecond line".to_owned()));
        assert!(create_child_args(&create_input(), "--help", &[]).is_err());
        let inherited = vec!["team:core".to_owned()];
        let args = create_child_args(&create_input(), "ontrack-parent", &inherited).unwrap();
        assert!(args.contains(&"--labels=team:core".to_owned()));
        assert!(
            create_child_args(&create_input(), "ontrack-parent", &[READY_LABEL.into()]).is_err()
        );
        assert!(
            create_child_args(
                &create_input(),
                "ontrack-parent",
                &[UNASSIGNED_LABEL.into()]
            )
            .is_err()
        );
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
            harness: None,
        }
    }

    #[test]
    fn ready_and_harness_labels_are_app_state_without_changing_beads_status() {
        let data = json!({"issue_type":"task", "labels":["team:core", READY_LABEL, "ontrack:agent:opencode"]});
        let bead = Bead {
            id: "b1".into(),
            title: "Task".into(),
            status: "open".into(),
            priority: 2,
            data: data.clone(),
        };
        assert!(is_ready(&bead));
        assert_eq!(harness(&data), Some("opencode"));
        assert!(!is_ready(&Bead {
            status: "closed".into(),
            ..bead.clone()
        }));
        assert!(!is_ready(&Bead {
            data: json!({"issue_type":"feature", "labels":[READY_LABEL]}),
            ..bead
        }));
        assert!(is_ready(&Bead {
            id: "epic".into(),
            title: "Epic".into(),
            status: "open".into(),
            priority: 2,
            data: json!({"issue_type":"epic", "labels":[READY_LABEL]}),
        }));
    }

    #[test]
    fn epic_ready_targets_include_nested_open_tasks_and_skip_existing_handoffs() {
        let rows = json!([
            {"id":"epic","title":"Epic","status":"open","issue_type":"epic"},
            {"id":"direct","title":"Direct","status":"open","issue_type":"task","parent":"epic"},
            {"id":"ready","title":"Ready","status":"open","issue_type":"task","parent":"epic","labels":[READY_LABEL]},
            {"id":"closed","title":"Closed","status":"closed","issue_type":"task","parent":"epic"},
            {"id":"group","title":"Group","status":"closed","issue_type":"feature","parent":"epic"},
            {"id":"nested","title":"Nested","status":"open","issue_type":"task","parent":"group"},
            {"id":"sent","title":"Sent","status":"open","issue_type":"task","parent":"epic"},
            {"id":"other","title":"Other","status":"open","issue_type":"task"}
        ]);
        let beads = parse_list(&serde_json::to_vec(&rows).unwrap()).unwrap();
        let handoffs = std::collections::HashSet::from(["sent".to_owned()]);
        assert_eq!(
            epic_ready_targets_in(&beads, "epic", true, &handoffs).unwrap(),
            ["epic", "direct", "nested"]
        );
        assert_eq!(
            epic_ready_targets_in(&beads, "epic", false, &handoffs).unwrap(),
            ["epic", "ready"]
        );
        assert!(epic_ready_targets_in(&beads, "other", true, &handoffs).is_err());
    }

    #[test]
    fn creation_and_edit_persist_harness_as_a_safe_label_argument() {
        let mut fields = create_input();
        fields.harness = Some("opencode".into());
        let args = create_args(&fields).unwrap();
        assert!(args.contains(&"--labels=ontrack:agent:opencode".into()));
        let update = UpdateBead {
            id: "b1".into(),
            fields,
            previous_harness: Some("codex".into()),
        };
        let args = update_args(&update).unwrap();
        assert!(args.contains(&"--remove-label=ontrack:agent:codex".into()));
        assert!(args.contains(&"--add-label=ontrack:agent:opencode".into()));
        assert!(args.contains(&"--remove-label=ontrack:unassigned".into()));
        assert_eq!(&args[args.len() - 2..], ["--", "b1"]);
    }

    #[test]
    fn unassigned_marker_distinguishes_new_choice_from_legacy_missing_label() {
        let legacy = json!({"labels": [READY_LABEL]});
        let unassigned = json!({"labels": [READY_LABEL, UNASSIGNED_LABEL]});
        assert!(!explicitly_unassigned(&legacy));
        assert!(explicitly_unassigned(&unassigned));
        assert!(harness(&legacy).is_none());
        assert!(harness(&unassigned).is_none());
        let args = create_args(&create_input()).unwrap();
        assert!(args.contains(&"--labels=ontrack:unassigned".into()));
        let mut selected = create_input();
        selected.harness = Some("codex".into());
        let args = update_args(&UpdateBead {
            id: "b1".into(),
            fields: selected,
            previous_harness: None,
        })
        .unwrap();
        assert!(args.contains(&"--remove-label=ontrack:unassigned".into()));
        assert!(args.contains(&"--add-label=ontrack:agent:codex".into()));
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
                "--labels=ontrack:unassigned",
            ]
        );
    }

    #[test]
    fn create_keeps_flag_like_and_shell_text_in_single_arguments() {
        let mut input = create_input();
        input.title = "--force $(touch ignored); 'quoted'".to_owned();
        input.description = "--cascade\n\"; $(touch ignored)".to_owned();
        let args = create_args(&input).unwrap();
        assert_eq!(args.len(), 8);
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
            previous_harness: None,
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
                "--add-label=ontrack:unassigned",
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
            previous_harness: None,
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
