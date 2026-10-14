# User guide

ontrack browses Beads issues in a two-pane terminal view. It invokes the public `bd` CLI in read-only mode, using the current working directory to select the workspace. It does not edit issues or access the Beads database directly.

## Requirements and startup

- Rust 1.88 or newer to build this Rust 2024 project.
- `bd` available on `PATH`, with support for the JSON commands shown in the [development guide](development.md#beads-cli-contract).
- An existing Beads workspace accessible from the directory where you launch ontrack.
- An interactive terminal supporting raw mode and the alternate screen; mouse support is needed for clicking and wheel input.

See the [official Beads documentation](https://beads.gascity.com) for installing `bd` and setting up a workspace.

From this repository, when it is the workspace you want to browse:

```sh
cargo run
```

To browse another workspace, build with `cargo build`, change to that workspace, and launch the built binary using its absolute path. The launch directory, not the binary's location, determines which workspace `bd` uses. Do not launch the interactive application in a runner without a terminal.

A brief splash precedes the browser. Issue loading starts in the background; quit keys work during startup too.

## Browsing and controls

The left pane lists all returned issues, including closed ones, without the CLI's usual list cap. Each entry shows its ID, priority, status, and title. The right pane shows the selected issue's formatted JSON, wrapped to the pane width.

| Input | Action |
| --- | --- |
| `↑` / `↓` with the list focused | Select the previous / next issue; selection stops at the ends. |
| Left-click a visible issue row | Select it and focus the list. |
| `Tab` | Toggle pane focus. |
| `←` / `→` | Focus the list / details pane. |
| `↑` / `↓` with details focused | Scroll details one line. |
| `PageUp` / `PageDown` with details focused | Scroll details by a pane-sized page. |
| Mouse wheel over the list | Change issue selection. |
| Mouse wheel over details | Scroll details. |
| `r` | Refresh the issue list and reload the selected issue's details. |
| `q`, `Esc`, or `Ctrl+C` | Quit. |

## Details, loading, and refresh

Selecting an issue immediately displays its cached list record while a richer `bd show` request runs. The returned detail record replaces that snapshot. ontrack requests long details, comments, and dependents; descriptions, relationships, metadata, nested values, and unknown fields are preserved whenever they are present in the returned JSON. It does not invent missing fields or fetch data outside those CLI responses. Scroll the details pane to see fields below the visible area.

The footer reports refreshing, detail loading, ready, or an error. Refresh retains the existing list while loading and keeps the selected ID if it still exists in the new list; otherwise it selects the first issue. A failed list refresh leaves the existing records available. A failed detail request leaves the available snapshot visible alongside the error. Press `r` to retry after resolving the cause. An empty successful list displays “No issues found in this workspace.”

Backend work does not run on the terminal input loop, so navigation and quitting remain available while a command is slow. Quitting restores raw-mode, mouse-capture, and alternate-screen settings through the application's cleanup path.

## Troubleshooting

- **“Could not run bd”**: check that `bd` is installed and on the launch environment's `PATH`, including when starting from an editor or a different shell.
- **“bd failed” / workspace or database errors**: check the error text and launch directory. Run the read-only list command below from that same directory to diagnose the CLI separately; consult the official Beads documentation for workspace access or setup.
- **Unknown CLI options**: ensure your `bd` supports `--readonly`, uncapped listing, and the detail flags in the development guide. ontrack reports failures rather than silently dropping those flags.
- **Invalid JSON or malformed records**: ontrack expects JSON arrays from `bd`; list entries need nonempty string IDs, titles, and statuses. Check the command output and your CLI version.
- **“expand terminal” / clipped content**: enlarge the terminal. Very small terminals show an expansion prompt instead of the two panes; quit keys still work. Use focus and scrolling for long details.
- **Mouse input does not work**: ensure the terminal supports mouse reporting, or use the keyboard controls.

```sh
bd --readonly --no-color list --all --limit 0 --json
```
