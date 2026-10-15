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

Issues are shown in rounded, colour-coded panes. Open appears first and closed last; other and unrecognized statuses remain grouped alphabetically. An empty status is labelled unknown. The Closed group starts collapsed on each launch, including when closed issues first appear after startup; other groups start expanded. Pane titles show the status and count. Each issue's type, priority, and status columns align across groups; issue details retain the ID. Use `↑` / `↓` to move through visible issues and collapsed group headers. Expanded group headers are skipped, so navigation crosses directly between their issue rows; collapsed headers appear in order among the remaining issues, and movement clamps at the first and last target. A focused collapsed header is highlighted and does not select an issue or retain detail/delete identity. Press `Right` or `Space` on that header to expand the group and select its first issue; `Left` leaves it collapsed. Press `Space` on a selected issue or use `←` / `→` to collapse or expand its group; clicking a pane title also toggles that group. Collapsing a selected issue's group selects its collapsed header. If all groups are collapsed, headers remain keyboard targets; mouse title clicks can also reopen them. Refresh preserves focus on a collapsed header while its group still exists.

## Browsing and controls

The left pane lists all returned issues, including closed ones, without the CLI's usual list cap. Each entry shows its colour-coded issue type, priority, status, and a white title; IDs remain available in the selected issue's full detail record. The rounded details pane shares the issue list's theme background. It presents narrative fields as Markdown, while metadata, comments, relationships, and unfamiliar nested values remain available as you scroll.

| Input | Action |
| --- | --- |
| `↑` / `↓` with the list focused | Move through visible issues and collapsed group headers; expanded headers are skipped. Clamp at either end. |
| `←` / `→` with the list focused | Collapse / expand the selected issue's group or the focused collapsed header. Repeating an unchanged direction has no effect. |
| `Space` on a selected issue or collapsed header | Collapse or expand that status group. |
| Left-click a pane title / visible issue row | Toggle the group / select the issue. |
| `Tab` | Toggle pane focus. |
| `↑` / `↓` with details focused | Scroll details one line. |
| `PageUp` / `PageDown` with details focused | Scroll details by a pane-sized page. |
| Mouse wheel over the list | Change issue selection. |
| Mouse wheel over details | Scroll details. |
| `r` | Refresh the issue list and reload the selected issue's details. |
| `s` | Open Settings. Up / Down navigates Theme, Auto-request merge, Close Herdr workspace, Remove checkout, and Close. Left / Right cycles the theme; Enter / Space toggles a focused preference. Changes save immediately as one complete settings file. All cleanup options default off; removal also closes the Herdr workspace. |
| `n` | Open the new-bead form. Tab / Shift+Tab moves through Title, Type, Priority, Description, Create, and Cancel. Left / Right cycles any focused dropdown choice with wraparound, immediately when closed or as a preview while open. Enter (or Space) opens the choice list; Up / Down previews options, Enter commits, and Esc restores the opening choice. Enter inserts a newline in the description. An empty title shows a required message. |
| `d` | Ask to permanently delete the selected bead. `y` confirms; `n` or Enter cancels. Deletion does not cascade; dependency links are removed and dependents may be orphaned. |
| `h` | Open the keyboard-operated handoff dialog for the selected bead. ontrack discovers the workspace and installed agents in the background; use Up / Down to choose, Enter to explicitly confirm, or Esc to cancel. Mouse selection is not available. Opening the dialog is read-only and does not launch anything. |
| `w` | Open outstanding tracked worktrees. This list is independent of Beads statuses, so closing or deleting a Bead does not hide unfinished work. A merge is a request, not proof of integration; inspect conflict/error states and recover in Git before retrying. Cleanup is separately confirmed and available only after verified integration. |
| `Esc` | Go back one level: close a dialog (or its open choice list first), return from outstanding worktrees to the main browser, or stay on the main browser if already there. A dialog stays locked while a mutation is running. |
| `q` or `Ctrl+C` | Quit when no dialog is open. |

Inside the create form, keys are text unless they are form controls; `q`, `r`, `n`, and `d` do not trigger app shortcuts, and arrow keys continue to edit text or choose options. Dropdowns share one rule: Left / Right cycles choices with wraparound, whether the dropdown is closed or open. In text fields, Left / Right retain their cursor-editing behavior. Type defaults to Task and priority to P2. A mutation runs once at a time in the background; the modal is locked until it finishes. Failures remain visible and keep the form open for correction or retry. Successful creation refreshes the list and selects the new bead when it appears. Dialogs ask you to enlarge a terminal that is too small to show the full controls and warnings.

The settings dialog captures input until it closes, so `q` cannot quit while it is open. Theme changes and each cleanup toggle save immediately. All automatic cleanup options default off. Automatic merge may defer when there is no trustworthy evidence that the agent is idle; a closed Bead alone never proves it is safe to merge. Removing a checkout also closes its Herdr workspace. Settings load/save errors remain visible; failed saves preserve the last successfully saved settings. See the [settings guide](settings.md) for recovery.

## Details, loading, and refresh

Selecting an issue immediately displays its cached list record while a richer `bd show` request runs. The returned detail record replaces that snapshot. ontrack requests long details, comments, and dependents; descriptions, notes, design and acceptance text render as Markdown. Metadata, comments, relationships, and unknown nested values are preserved whenever present in the returned JSON. It does not invent missing fields or fetch data outside those CLI responses. Scroll the details pane to see fields below the visible area.

ontrack automatically polls the issue list and selected issue's details in the background, starting another refresh roughly one second after the previous refresh completes. Changes made by another terminal or agent appear without pressing `r` or configuring events, a watcher, or extra dependencies. Slow `bd` commands increase the delay; updates are not instantaneous or event-driven. Successful create/delete actions in ontrack still trigger an immediate refresh, and `r` remains available for a manual refresh.

The footer reports refreshing, detail loading, ready, or an error. Refresh retains the existing list while loading and keeps the selected ID if it still exists in the new list; otherwise it selects the first issue. Automatic updates preserve group collapse state, pane focus, scroll position within available content, and active form input. A failed list or detail refresh keeps the last loaded data (or the available list snapshot) visible alongside the error and retries automatically. You can also press `r` after resolving the cause. An empty successful list displays “No issues found in this workspace.”

## Launch an agent with Herdr

Run ontrack inside a running [Herdr](https://herdr.dev/docs) shell in a Git checkout. Herdr must supply `HERDR_ENV=1`; `herdr` and the selected agent executable must be on `PATH`, and that agent's Herdr integration must already be installed and current. ontrack does not install integrations or grant Git safe-directory trust automatically.

Codex launches trust the verified main repository and disable startup update checks **for that invocation only**; no global configuration is changed. Source and new-worktree Git metadata must agree before Codex starts. This project trust may load repository configuration and hooks, but does not weaken sandbox or approval policy. OpenCode startup is unchanged. Authentication, administrator policy or unknown startup prompts may still need your attention; ontrack does not bypass them. Nonstandard or mismatched Git metadata stops startup safely and preserves any created worktree for inspection.

Handoff uses the selected bead and its mapped Herdr workspace, not global window focus. The dialog shows the bead and workspace before confirmation. Discovery failure can be retried with `h` after resolving the cause. Confirmation creates a worktree from committed checkout content only, starts the selected installed integration, and sends the issue context with instructions to inspect the authoritative bead and project guidance.

Only new handoffs recorded by OnTrack are managed in the outstanding-worktrees view; pre-existing worktrees are not adopted. The target branch comes from verified repository state and is shown before a merge request. Git integration—not Beads status—determines whether work is integrated. Conflicts and uncertain outcomes remain visible for manual recovery. Cleanup never force-removes a checkout or removes its branch.

In the `w` view, select a handoff with Up / Down. Press `m` to request a merge, `c` to close its integrated Herdr workspace, or `x` to remove its integrated checkout; each action requires `y` confirmation. The agent merges the target into the task branch and resolves conflicts there; OnTrack then verifies current Git history and fast-forwards the target. Keep OnTrack running for reconciliation. Press `r` to refresh or `w` to return to Beads.

Untouched branches with no task commits are not automatically considered complete. Automatic operations defer without reliable native agent evidence; an explicitly confirmed manual merge can use Git verification without optional session metadata. Removal also closes an open Herdr workspace, so enable removal before closing if both are wanted. Removal after a workspace has already been closed is unsupported: inspect the reported checkout, verify its current changes are integrated and clean, then use Git's exact-path, non-force worktree removal manually. OnTrack never guesses a replacement workspace ID.

Title-only or incomplete beads are supported: the agent is instructed to infer a useful description, scoped requirements and testable acceptance criteria from the live bead and project context, record assumptions, and save missing fields with `bd update` before implementation. It must preserve existing requirements, avoid unrelated scope, and not refuse solely because description or criteria are missing. Only consequential ambiguity unresolved by context warrants clarification; useful inferred details, assumptions and questions should still be saved first. ontrack sends these instructions rather than generating fields itself; handoff success does not prove the agent followed them.

Start ontrack from the **main checkout** (or an ordinary subdirectory) to hand off work. Launch discovery from existing linked checkouts, submodules or nested repositories is unsupported and stops before creation; Herdr 0.9.1 reports the main source checkout for linked paths, so ontrack will not guess a linked workspace. Newly created worktrees remain the verified targets for agent startup.

The new checkout contains committed Git content only; uncommitted changes are not copied. A successful handoff does not mean the agent completed the bead. Failures may leave a worktree or may have delivered a prompt. Inspect Herdr/Git worktree lists and the reported workspace/branch before retrying; nothing is automatically deleted or blindly resubmitted.

Long launch results stay in a height-limited dialog instead of requiring a taller terminal. Use PageUp / PageDown to read the full recovery message; the dialog marks when more text is outside the visible area. Enter does not launch again after a result arrives. Press Esc to close the result. The compact dialog keeps the same scroll controls; widen the terminal only if the dialog itself cannot fit.

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
