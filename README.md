# ontrack

A terminal browser for Beads issues in the current workspace.

- Read-only `bd` access, including closed issues with no list limit.
- Two-pane browsing with scrollable JSON details, comments, relationships, and unknown fields returned by Beads.
- Background loading and refresh with cached records and visible errors.

## Requirements

Rust 1.88 or newer, the `bd` command-line tool, and a Beads workspace. A terminal with alternate-screen and mouse support is recommended.

## Run

```sh
cargo run
```

Use `↑` / `↓` to select issues, or click a visible row. The left pane includes open and closed issues. Press `Tab` or `←` / `→` to focus a pane; when the details pane is focused, use arrows or `PageUp` / `PageDown` to scroll. The mouse wheel over the left pane changes selection; over the right pane it scrolls details. Press `r` to refresh. Press `q`, `Esc`, or `Ctrl+C` to quit.

See the [user guide](docs/user-guide.md) for setup and troubleshooting, the [development guide](docs/development.md) for architecture and checks, and the [official Beads documentation](https://beads.gascity.com) for workspace setup.

## Checks

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```
