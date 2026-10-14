# User guide

ontrack browses Beads issues in a two-pane terminal view and can create or permanently delete one issue at a time. It uses the public `bd` CLI and the current working directory to select the workspace. It does not access the Beads database directly.

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

## Status groups

Issues are shown in rounded, colour-coded panes. Open appears first and closed last; other and unrecognized statuses remain grouped alphabetically. An empty status is labelled unknown. Pane titles show the status and count. Use `↑` / `↓` to move through group titles and visible issues. Press `Space` on a group title or issue to collapse/expand that status; clicking the pane title does the same. Collapsing a selected issue clears its detail selection; expanding the focused group selects its first issue. Collapse state is retained across refreshes while groups still exist.

## Browsing and controls

The left pane lists all returned issues, including closed ones, without the CLI's usual list cap. Each entry shows its colour-coded issue type, priority, status, and a white title; IDs remain available in the selected issue's full detail record. The right pane presents narrative fields as Markdown, while metadata, comments, relationships, and unfamiliar nested values remain available as you scroll.

| Input | Action |
| --- | --- |
| `↑` / `↓` with the list focused | Move through group titles and visible issues. |
| `←` / `→` with the list focused | Collapse / expand the focused status group. Repeating either key leaves the group unchanged. |
| `Space` on a group or issue | Collapse or expand that status group. |
| Left-click a pane title / visible issue row | Toggle the group / select the issue. |
| `Tab` | Toggle pane focus. |
| `↑` / `↓` with details focused | Scroll details one line. |
| `PageUp` / `PageDown` with details focused | Scroll details by a pane-sized page. |
| Mouse wheel over the list | Change issue selection. |
| Mouse wheel over details | Scroll details. |
| `r` | Refresh the issue list and reload the selected issue's details. |
| `n` | Open the new-bead form. Tab / Shift+Tab moves through Title, Type, Priority, Description, Create, and Cancel. Enter inserts a newline in the description; Enter opens type/priority choices, arrows preview options, Enter commits, and Esc restores the prior choice or cancels the form. An empty title shows a required message. |
| `d` | Ask to permanently delete the selected bead. `y` confirms; `n` or Enter cancels. Deletion does not cascade; dependency links are removed and dependents may be orphaned. |
| `q`, `Esc`, or `Ctrl+C` | Quit the browser when no dialog is open. In a form, `Esc` cancels the dialog; while a mutation is running the dialog is locked until completion. |

Inside the create form, keys are text unless they are form controls; `q`, `r`, `n`, and `d` do not trigger browser shortcuts, and arrow keys continue to edit text or choose options. Type defaults to Task and priority to P2. A mutation runs once at a time in the background; the modal is locked until it finishes. Failures remain visible and keep the form open for correction or retry. Successful creation refreshes the list and selects the new bead when it appears. Dialogs ask you to enlarge a terminal that is too small to show the full controls and warnings.

## Details, loading, and refresh

Selecting an issue immediately displays its cached list record while a richer `bd show` request runs. The returned detail record replaces that snapshot. ontrack requests long details, comments, and dependents; descriptions, notes, design and acceptance text render as Markdown. Metadata, comments, relationships, and unknown nested values are preserved whenever present in the returned JSON. It does not invent missing fields or fetch data outside those CLI responses. Scroll the details pane to see fields below the visible area.

The footer reports refreshing, detail loading, ready, or an error. Refresh retains the existing list while loading and keeps the selected ID if it still exists in the new list; otherwise it selects the first issue. A failed list refresh leaves the existing records available. A failed detail request leaves the available snapshot visible alongside the error. Press `r` to retry after resolving the cause. An empty successful list displays “No issues found in this workspace.”

Backend work does not run on the terminal input loop, so browsing remains responsive while a command is slow. During a mutation, its modal stays open and locked until a result arrives. Mutations are queued and executed in order without coalescing. Quitting restores raw-mode, mouse-capture, and alternate-screen settings through the application's cleanup path.

## Troubleshooting

- **“Could not run bd”**: check that `bd` is installed and on the launch environment's `PATH`, including when starting from an editor or a different shell.
- **“bd failed” / workspace or database errors**: check the error text and launch directory. Run the read-only list command below from that same directory to diagnose the CLI separately; consult the official Beads documentation for workspace access or setup.
- **Unknown CLI options**: ensure your `bd` supports `--readonly`, uncapped listing, and the detail flags in the development guide. ontrack reports failures rather than silently dropping those flags.
- **Invalid JSON or malformed records**: list/show responses must be JSON arrays; creation must return an object with an ID. List entries need nonempty string IDs, titles, and statuses. Check the command output and your CLI version.
- **“expand terminal” / clipped content**: enlarge the terminal. Very small terminals show an expansion prompt instead of the two panes; browser quit keys still work when no dialog is open. Use focus and scrolling for long details.
- **Mouse input does not work**: ensure the terminal supports mouse reporting, or use the keyboard controls.

```sh
bd --readonly --no-color list --all --limit 0 --json
```
