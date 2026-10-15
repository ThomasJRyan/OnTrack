# ontrack

A terminal browser for Beads issues in the current workspace.

- Browse open and closed Beads, with explicit create, edit, and permanent-delete actions.
- Mark open Tasks Ready, or mark an Epic and all its open descendant Tasks Ready together. The scheduler prepares a Ready Epic's checkout and hands eligible child Tasks to Herdr agents. Sent Tasks remain Ready until their Beads status becomes In Progress. Set a concurrent agent limit in Settings (five by default).
- Press `Shift+N` on an open task to create a child task. Child issues appear indented beneath their parent within a status pane. Parent handoff waits for children, and completed child work automatically requests integration into the managed parent worktree by default; Settings can turn this off.
- When creating an Epic, optionally generate child tasks with assessed P0–P4 priorities from its Description using a Herdr agent in a separate workspace at the same checkout.
- Two-pane browsing with scrollable, Markdown-rendered descriptions, notes, comments, relationships, and Beads details.
- Automatic background refresh picks up external changes, with cached records and visible errors; no watcher setup required.
- Persistent theme and default new-task agent selection in a structured settings dialog.
- A brief animated terminal splash with the ONTRACK wordmark and a waving anime girl, with compact layouts for smaller terminals.
- Press `h` to hand a selected Bead to a Herdr agent in a new worktree. A top-right toast shows progress and the result; Open and Ready issues show `· sent` once the prompt is confirmed. See the [user guide](docs/user-guide.md) for setup and recovery limits.
- Press `w` to inspect OnTrack-tracked outstanding worktrees, with colour-coded statuses and Bead details. Completed tasks show Ready to Merge after the Bead closes and the recorded agent settles; enabled preferences queue merge requests one at a time, then clean up verified integrated worktrees. Older handoffs without enough evidence show a manual merge hint. Active merge work stays Pending, and verified removed checkouts leave this view. Press `Enter` to switch to a worktree's Herdr workspace, request a merge or safe cleanup, or use `Shift+R` to confirm a fresh start for the task.

## Requirements

Rust 1.88 or newer and the `bd` command-line tool. OnTrack initializes Beads in the launch directory when no workspace is available. A terminal with alternate-screen and mouse support is recommended.

## Run

```sh
cargo run
```

Issues are grouped into coloured, collapsible panes (Open, Ready, other statuses, Closed), with Closed collapsed on launch. Press `Ctrl+?` for hotkeys, `r` (or `Shift+R`) to toggle Ready for an open Task or Epic, `w` for tracked handoffs, `s` for settings, `n` to create a bead, `Shift+N` to create a child task, `e` to edit, and `d` to delete (confirmation required). `Esc` goes back or closes a dialog; `q` and `Ctrl+C` quit. See the [user guide](docs/user-guide.md) for controls and the [development guide](docs/development.md) for scheduling and tracked worktrees.

See the [user guide](docs/user-guide.md) for setup and controls, the [settings guide](docs/settings.md) for theme, default agent, agent limit, and worktree preferences, the [development guide](docs/development.md) for architecture and checks, and the [official Beads documentation](https://beads.gascity.com) for workspace setup.

## Checks

GitHub Actions runs these checks on pull requests and pushes to `main`. See the [CI guide](docs/ci.md) for results and required-check setup.

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```
