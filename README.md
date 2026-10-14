# ontrack

A terminal browser for Beads issues in the current workspace.

- Browse open and closed Beads, with explicit create and permanent-delete actions.
- Two-pane browsing with scrollable, Markdown-rendered descriptions, notes, comments, relationships, and complete Beads details.
- Automatic background refresh picks up external changes, with cached records and visible errors; no watcher setup required.
- Persistent dark or light appearance with a live preview in settings.

## Requirements

Rust 1.88 or newer, the `bd` command-line tool, and a Beads workspace. A terminal with alternate-screen and mouse support is recommended.

## Run

```sh
cargo run
```

Issues are grouped into coloured, collapsible status panes (open first, closed last). Each row colour-codes issue type, priority, and status while keeping its title readable. Use `↑` / `↓` to move through issues and group titles; `←` collapses and `→` expands the focused group. `Space` toggles a group, and clicking its pane title also toggles it. Press `Tab` to switch panes. Press `s` to preview and save a dark or light theme, `n` to create a bead, or `d` to permanently delete the selected issue (confirmation required). Changes refresh automatically; press `r` for a manual refresh. Press `q`, `Esc`, or `Ctrl+C` to quit outside a dialog. See the [user guide](docs/user-guide.md) for details.

See the [user guide](docs/user-guide.md) for setup and controls, the [settings guide](docs/settings.md) for theme storage, the [development guide](docs/development.md) for architecture and checks, and the [official Beads documentation](https://beads.gascity.com) for workspace setup.

## Checks

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```
