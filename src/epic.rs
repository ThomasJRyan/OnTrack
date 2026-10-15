//! Opt-in Epic planning in a separate Herdr workspace using this checkout.

use std::{io, path::Path, process::Command};

use serde_json::Value;

use crate::{beads, herdr};

#[derive(Debug, Clone)]
pub struct Session {
    pub workspace_id: String,
    pub pane_id: String,
}

trait Backend {
    fn herdr(&mut self, args: &[&str]) -> io::Result<Vec<u8>>;
    fn show(&mut self, id: &str) -> io::Result<Value>;
    fn children(&mut self, id: &str) -> io::Result<Vec<String>>;
}

struct System;

impl Backend for System {
    fn herdr(&mut self, args: &[&str]) -> io::Result<Vec<u8>> {
        let output = Command::new("herdr").args(args).output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "Herdr exited {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(output.stdout)
    }

    fn show(&mut self, id: &str) -> io::Result<Value> {
        beads::show_bead(id)
    }

    fn children(&mut self, id: &str) -> io::Result<Vec<String>> {
        Ok(beads::list_beads()?
            .into_iter()
            .filter(|bead| bead.data["parent"].as_str() == Some(id))
            .map(|bead| bead.id)
            .collect())
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn id<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    let value = value[key]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("Herdr response lacks {key}")))?;
    if value.starts_with('-')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b":_-".contains(&byte))
    {
        return Err(invalid(format!("Herdr response has unsafe {key}")));
    }
    Ok(value)
}

fn result(bytes: &[u8], expected: &str) -> io::Result<Value> {
    let value = herdr::envelope(bytes)?;
    if value["type"] != expected {
        return Err(invalid(format!("Expected Herdr {expected} response")));
    }
    Ok(value)
}

pub fn launch(epic_id: &str) -> io::Result<Session> {
    let cwd = std::env::current_dir()?;
    let source_id = std::env::var("HERDR_WORKSPACE_ID")
        .map_err(|_| invalid("Open OnTrack inside a Herdr workspace to plan an Epic"))?;
    launch_with(&mut System, &cwd, &source_id, epic_id)
}

fn launch_with(
    backend: &mut impl Backend,
    cwd: &Path,
    source_id: &str,
    epic_id: &str,
) -> io::Result<Session> {
    // Check the inherited ID against the live topology. Never use global focus.
    let workspaces = result(&backend.herdr(&["workspace", "list"])?, "workspace_list")?;
    if !workspaces["workspaces"].as_array().is_some_and(|rows| {
        rows.iter()
            .any(|row| row["workspace_id"].as_str() == Some(source_id))
    }) {
        return Err(invalid("Source Herdr workspace is no longer open"));
    }
    let live = backend.show(epic_id)?;
    if live["issue_type"] != "epic" {
        return Err(invalid("Created Bead is no longer an Epic"));
    }
    let proposal = live["description"]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| invalid("Epic proposal is empty; add a description before planning"))?;
    let supported = herdr::supported_agents(&backend.herdr(&["agent", "start", "--help"])?)?;
    let available =
        herdr::available_agents(&backend.herdr(&["integration", "status"])?, &supported)?;
    let agent = available
        .iter()
        .find(|agent| agent.as_str() == "codex")
        .unwrap_or(&available[0]);
    let cwd = cwd.canonicalize()?;
    let cwd = cwd
        .to_str()
        .ok_or_else(|| invalid("Current checkout path is not UTF-8"))?;
    let label = format!("Epic {epic_id} · from {source_id}");
    let created = backend
        .herdr(&["workspace", "create", "--cwd", cwd, "--label", &label, "--no-focus"])
        .and_then(|bytes| result(&bytes, "workspace_created"))
        .map_err(|error| io::Error::other(format!(
            "Workspace creation may have succeeded: {error}. Inspect Herdr workspaces before retrying"
        )))?;
    let workspace_id = id(&created["workspace"], "workspace_id")?.to_owned();
    let pane_id = id(&created["root_pane"], "pane_id")?.to_owned();
    if workspace_id == source_id
        || created["root_pane"]["workspace_id"] != workspace_id
        || !pane_id.starts_with(&format!("{workspace_id}:"))
    {
        return Err(invalid(
            "Herdr returned inconsistent Epic workspace and pane IDs",
        ));
    }
    let session = Session {
        workspace_id,
        pane_id,
    };
    let name = format!(
        "epic-{}",
        epic_id
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(16)
            .collect::<String>()
    );
    let started = backend.herdr(&[
        "agent",
        "start",
        &name,
        "--kind",
        agent,
        "--pane",
        &session.pane_id,
        "--timeout",
        "30000",
    ]);
    let started = started.and_then(|bytes| result(&bytes, "agent_started"));
    if started.as_ref().map_or(true, |value| {
        value["agent"]["pane_id"] != session.pane_id
            || value["agent"]["workspace_id"] != session.workspace_id
    }) {
        return Err(io::Error::other(format!(
            "Agent start failed or identity changed in workspace {}. Inspect pane {} before retrying: {}",
            session.workspace_id,
            session.pane_id,
            started
                .err()
                .map_or_else(|| "identity mismatch".to_owned(), |error| error.to_string())
        )));
    }
    let prompt = format!(
        "Create actionable direct child Beads for Epic {epic_id} from the proposal below. Read AGENTS.md and CLAUDE.md first. Use bd create with --parent={epic_id} for each child; give each task a useful description and acceptance criteria. Choose a Beads priority for each child on the P0-P4 scale (P0 highest, P4 lowest), comparing the tasks by impact, urgency, and which work enables other tasks. Pass --priority=N explicitly on every bd create. Make meaningful priority distinctions when the proposal supports them instead of assigning every child the same default priority. Equal priorities are fine when tasks are genuinely comparable or the proposal gives too little evidence; do not invent urgency just to vary the numbers. Do not implement the tasks. Verify at least one direct child exists. Report the child IDs and then finish your turn. The OnTrack parent process will verify and close this workspace; if it is no longer running, close only this workspace ({}) after verification.\n\nProposal:\n{proposal}",
        session.workspace_id
    );
    // Wait for the server to observe this prompt's working state before a
    // separate completion wait. This avoids matching the pre-prompt idle state.
    let prompted = backend.herdr(&[
        "agent",
        "prompt",
        &session.pane_id,
        &prompt,
        "--wait",
        "--until",
        "working",
    ]);
    let prompted = prompted.and_then(|bytes| result(&bytes, "agent_prompted"));
    if prompted.as_ref().map_or(true, |value| {
        value["agent"]["pane_id"] != session.pane_id
            || value["agent"]["workspace_id"] != session.workspace_id
    }) {
        return Err(io::Error::other(format!(
            "Prompt may have been delivered in workspace {} pane {}. Inspect the agent before retrying; do not generate duplicate children: {}",
            session.workspace_id,
            session.pane_id,
            prompted
                .err()
                .map_or_else(|| "identity mismatch".to_owned(), |error| error.to_string())
        )));
    }
    Ok(session)
}

pub fn finish(session: &Session, epic_id: &str) -> io::Result<Vec<String>> {
    finish_with(&mut System, session, epic_id).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "{error}. Inspect Herdr workspace {} and the Epic's children before retrying",
                session.workspace_id
            ),
        )
    })
}

fn finish_with(
    backend: &mut impl Backend,
    session: &Session,
    epic_id: &str,
) -> io::Result<Vec<String>> {
    match backend.herdr(&[
        "agent",
        "wait",
        &session.pane_id,
        "--until",
        "done",
        "--until",
        "idle",
        "--until",
        "blocked",
    ]) {
        Ok(bytes) => {
            let waited = result(&bytes, "agent_info")?;
            if waited["agent"]["pane_id"] != session.pane_id
                || waited["agent"]["workspace_id"] != session.workspace_id
                || !matches!(
                    waited["agent"]["agent_status"].as_str(),
                    Some("done" | "idle")
                )
            {
                return Err(invalid(
                    "Herdr wait returned a different agent or unfinished state",
                ));
            }
        }
        Err(wait_error) => {
            // An agent may exit after completing the prompt, removing its name
            // before Herdr can return a settled state. Confirm that the same
            // pane is still present and has no replacement agent.
            let pane = result(
                &backend.herdr(&["pane", "get", &session.pane_id])?,
                "pane_info",
            )?;
            if pane["pane"]["pane_id"] != session.pane_id
                || pane["pane"]["workspace_id"] != session.workspace_id
                || !pane["pane"]["agent"].is_null()
                || !pane["pane"]["agent_status"].is_null()
            {
                return Err(io::Error::new(
                    wait_error.kind(),
                    format!(
                        "Herdr agent wait failed and the planning pane is not empty: {wait_error}"
                    ),
                ));
            }
        }
    }
    let children = backend.children(epic_id)?;
    if children.is_empty() {
        return Err(invalid(format!(
            "No direct child Beads found for {epic_id}; workspace {} remains open for recovery",
            session.workspace_id
        )));
    }
    let closed = result(
        &backend.herdr(&["workspace", "close", &session.workspace_id])?,
        "workspace_closed",
    )?;
    if closed["workspace_id"]
        .as_str()
        .is_some_and(|id| id != session.workspace_id)
    {
        return Err(invalid("Herdr closed a different workspace"));
    }
    Ok(children)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::VecDeque;

    struct Mock {
        responses: VecDeque<io::Result<Vec<u8>>>,
        calls: Vec<Vec<String>>,
        children: Vec<String>,
    }

    impl Mock {
        fn new(responses: Vec<Value>, children: Vec<&str>) -> Self {
            Self {
                responses: responses
                    .into_iter()
                    .map(|value| Ok(json!({"result": value}).to_string().into_bytes()))
                    .collect(),
                calls: Vec::new(),
                children: children.into_iter().map(str::to_owned).collect(),
            }
        }
    }

    impl Backend for Mock {
        fn herdr(&mut self, args: &[&str]) -> io::Result<Vec<u8>> {
            self.calls
                .push(args.iter().map(|arg| (*arg).to_owned()).collect());
            self.responses
                .pop_front()
                .unwrap_or_else(|| Err(invalid("No mock response")))
        }

        fn show(&mut self, _id: &str) -> io::Result<Value> {
            Ok(json!({"issue_type":"epic", "description":"A large proposal"}))
        }

        fn children(&mut self, _id: &str) -> io::Result<Vec<String>> {
            Ok(self.children.clone())
        }
    }

    struct LaunchMock {
        step: usize,
        calls: Vec<Vec<String>>,
    }

    impl Backend for LaunchMock {
        fn herdr(&mut self, args: &[&str]) -> io::Result<Vec<u8>> {
            self.calls
                .push(args.iter().map(|arg| (*arg).to_owned()).collect());
            let bytes = match self.step {
                0 => json!({"result":{"type":"workspace_list","workspaces":[{"workspace_id":"wSource"}]}}).to_string(),
                1 => "--kind <KIND> [possible values: codex, opencode]".to_owned(),
                2 => "codex: current (v1) (/tmp/codex)\nopencode: not installed".to_owned(),
                3 => json!({"result":{"type":"workspace_created","workspace":{"workspace_id":"wEpic"},"root_pane":{"workspace_id":"wEpic","pane_id":"wEpic:p1"}}}).to_string(),
                4 => json!({"result":{"type":"agent_started","agent":{"workspace_id":"wEpic","pane_id":"wEpic:p1"}}}).to_string(),
                5 => json!({"result":{"type":"agent_prompted","agent":{"workspace_id":"wEpic","pane_id":"wEpic:p1"}}}).to_string(),
                _ => panic!("unexpected Herdr call"),
            };
            self.step += 1;
            Ok(bytes.into_bytes())
        }

        fn show(&mut self, _id: &str) -> io::Result<Value> {
            Ok(json!({"issue_type":"epic", "description":"A large proposal"}))
        }

        fn children(&mut self, _id: &str) -> io::Result<Vec<String>> {
            unreachable!()
        }
    }

    #[test]
    fn launch_uses_same_checkout_and_parented_prompt_without_worktree() {
        let mut backend = LaunchMock {
            step: 0,
            calls: Vec::new(),
        };
        let cwd = std::env::current_dir().unwrap();
        let session = launch_with(&mut backend, &cwd, "wSource", "epic-1").unwrap();
        assert_eq!(session.workspace_id, "wEpic");
        assert_eq!(backend.calls[3][..2], ["workspace", "create"]);
        assert_eq!(
            backend.calls[3][3],
            cwd.canonicalize().unwrap().to_str().unwrap()
        );
        assert!(
            !backend
                .calls
                .iter()
                .any(|call| call.first().is_some_and(|arg| arg == "worktree"))
        );
        assert!(backend.calls[5][3].contains("--parent=epic-1"));
        assert!(backend.calls[5][3].contains("A large proposal"));
        assert!(backend.calls[5][3].contains("useful description and acceptance criteria"));
        assert!(backend.calls[5][3].contains("P0-P4 scale (P0 highest, P4 lowest)"));
        assert!(
            backend.calls[5][3].contains("impact, urgency, and which work enables other tasks")
        );
        assert!(backend.calls[5][3].contains("--priority=N explicitly on every bd create"));
        assert!(backend.calls[5][3].contains("meaningful priority distinctions"));
        assert!(backend.calls[5][3].contains("Equal priorities are fine"));
        assert_eq!(&backend.calls[5][4..], ["--wait", "--until", "working"]);
    }

    #[test]
    fn finished_agent_closes_only_after_children_exist() {
        let session = Session {
            workspace_id: "wEpic".into(),
            pane_id: "wEpic:p1".into(),
        };
        let waited = json!({"type":"agent_info","agent":{"workspace_id":"wEpic","pane_id":"wEpic:p1","agent_status":"done"}});
        let closed = json!({"type":"workspace_closed","workspace_id":"wEpic"});
        let mut success = Mock::new(vec![waited.clone(), closed], vec!["epic-1.1"]);
        assert_eq!(
            finish_with(&mut success, &session, "epic-1").unwrap(),
            ["epic-1.1"]
        );
        assert_eq!(success.calls[1], ["workspace", "close", "wEpic"]);

        let mut empty = Mock::new(vec![waited], vec![]);
        assert!(finish_with(&mut empty, &session, "epic-1").is_err());
        assert_eq!(empty.calls.len(), 1);

        let blocked = json!({"type":"agent_info","agent":{"workspace_id":"wEpic","pane_id":"wEpic:p1","agent_status":"blocked"}});
        let mut blocked = Mock::new(vec![blocked], vec!["epic-1.1"]);
        assert!(finish_with(&mut blocked, &session, "epic-1").is_err());
        assert_eq!(blocked.calls.len(), 1);
    }

    #[test]
    fn exited_agent_with_direct_children_closes_planning_workspace() {
        let session = Session {
            workspace_id: "wEpic".into(),
            pane_id: "wEpic:p1".into(),
        };
        let empty_pane =
            json!({"type":"pane_info","pane":{"workspace_id":"wEpic","pane_id":"wEpic:p1"}});
        let closed = json!({"type":"workspace_closed","workspace_id":"wEpic"});
        let mut backend = Mock::new(vec![empty_pane, closed], vec!["epic-1.1"]);
        backend
            .responses
            .push_front(Err(io::Error::other("agent exited")));

        assert_eq!(
            finish_with(&mut backend, &session, "epic-1").unwrap(),
            ["epic-1.1"]
        );
        assert_eq!(backend.calls[1], ["pane", "get", "wEpic:p1"]);
        assert_eq!(backend.calls[2], ["workspace", "close", "wEpic"]);
    }

    #[test]
    fn exited_agent_without_children_or_intact_empty_pane_stays_open() {
        let session = Session {
            workspace_id: "wEpic".into(),
            pane_id: "wEpic:p1".into(),
        };
        let empty_pane =
            json!({"type":"pane_info","pane":{"workspace_id":"wEpic","pane_id":"wEpic:p1"}});
        let mut no_children = Mock::new(vec![empty_pane.clone()], vec![]);
        no_children
            .responses
            .push_front(Err(io::Error::other("agent exited")));
        assert!(finish_with(&mut no_children, &session, "epic-1").is_err());
        assert_eq!(no_children.calls.len(), 2);

        for pane in [
            json!({"type":"pane_info","pane":{"workspace_id":"wOther","pane_id":"wEpic:p1"}}),
            json!({"type":"pane_info","pane":{"workspace_id":"wEpic","pane_id":"wEpic:p1","agent":"codex","agent_status":"idle"}}),
        ] {
            let mut unsafe_pane = Mock::new(vec![pane], vec!["epic-1.1"]);
            unsafe_pane
                .responses
                .push_front(Err(io::Error::other("agent exited")));
            assert!(finish_with(&mut unsafe_pane, &session, "epic-1").is_err());
            assert_eq!(unsafe_pane.calls.len(), 2);
        }

        let mut missing_pane = Mock::new(vec![], vec!["epic-1.1"]);
        missing_pane
            .responses
            .push_back(Err(io::Error::other("agent exited")));
        missing_pane
            .responses
            .push_back(Err(io::Error::other("pane not found")));
        assert!(finish_with(&mut missing_pane, &session, "epic-1").is_err());
        assert_eq!(missing_pane.calls.len(), 2);
    }
}
