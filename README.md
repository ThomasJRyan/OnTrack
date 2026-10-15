# ontrack

A terminal browser for Beads issues in the current workspace.

- Browse open and closed Beads, with explicit create, edit, and permanent-delete actions.
- Two-pane browsing with scrollable, Markdown-rendered descriptions, notes, comments, relationships, and Beads details.
- Automatic background refresh picks up external changes, with cached records and visible errors; no watcher setup required.
- Persistent theme selection in a structured settings dialog with a live preview.
- Press `h` to hand a selected Bead to a Herdr agent in a new worktree. A top-right toast shows progress and the result; Open issues show `· sent` once the prompt is confirmed. See the [user guide](docs/user-guide.md) for setup and recovery limits.
- Press `w` to inspect OnTrack-tracked outstanding worktrees, with colour-coded statuses and Bead details; verified removed checkouts, including externally removed ones, leave this view. Press `Enter` on a worktree to switch to its Herdr workspace, or request a merge or safe cleanup.

## Requirements

Rust 1.88 or newer, the `bd` command-line tool, and a Beads workspace. A terminal with alternate-screen and mouse support is recommended.

## Run

```sh
cargo run
```

Issues are grouped into coloured, collapsible status panes (open first, closed last), with Closed collapsed on launch. Press `w` for outstanding tracked handoffs, `s` for appearance and cleanup preferences, `n` to create a bead, `e` to edit the selected bead, and `d` to permanently delete one (confirmation required). `Esc` goes back or closes a dialog; `q` and `Ctrl+C` quit. See the [user guide](docs/user-guide.md) for controls and the [development guide](docs/development.md) for the tracked-worktree model.

See the [user guide](docs/user-guide.md) for setup and controls, the [settings guide](docs/settings.md) for theme and worktree preferences, the [development guide](docs/development.md) for architecture and checks, and the [official Beads documentation](https://beads.gascity.com) for workspace setup.

## Checks

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```
