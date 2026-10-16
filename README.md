# ontrack

A terminal browser for Beads issues in the current workspace.

- Browse open and closed Beads, with explicit create and permanent-delete actions.
- Two-pane browsing with scrollable, Markdown-rendered descriptions, notes, comments, relationships, and complete Beads details.
- Automatic background refresh picks up external changes, with cached records and visible errors; no watcher setup required.
- Persistent dark or light appearance with a live preview in settings.
- Press `h` to hand a selected Bead to an installed Herdr agent in a new worktree, after explicit confirmation. Handoffs run concurrently in the background; press `t` for session-local status and recovery. See the [user guide](docs/user-guide.md) for setup and recovery limits.
- Press `w` to inspect OnTrack-tracked outstanding worktrees independently of Bead status, request a merge, or confirm safe cleanup.

## Requirements

Rust 1.88 or newer, the `bd` command-line tool, and a Beads workspace. A terminal with alternate-screen and mouse support is recommended.

## Run

```sh
cargo run
```

Issues are grouped into coloured, collapsible status panes (open first, closed last). Press `w` for outstanding tracked handoffs, `s` for appearance and cleanup preferences, `n` to create a bead, and `d` to permanently delete one (confirmation required). See the [user guide](docs/user-guide.md) for controls and the [development guide](docs/development.md) for the tracked-worktree model.

See the [user guide](docs/user-guide.md) for setup and controls, the [settings guide](docs/settings.md) for theme and worktree preferences, the [development guide](docs/development.md) for architecture and checks, and the [official Beads documentation](https://beads.gascity.com) for workspace setup.

## Checks

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```
