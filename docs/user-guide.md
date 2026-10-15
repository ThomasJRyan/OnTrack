# User guide

ontrack browses Beads issues in a two-pane terminal view and can create, edit, or permanently delete one issue at a time. It uses the public `bd` CLI and the current working directory to select the workspace. It does not access the Beads database directly.

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

A brief splash precedes the browser. A terminal-drawn anime girl blinks and waves beneath the ONTRACK wordmark; narrower terminals use a compact title, and very small terminals show text only. The animation lasts about 1.2 seconds and uses the selected theme. Issue loading starts in the background; quit keys work during startup too.

## Status groups

Issues are shown in rounded, colour-coded panes. Open appears first and closed last; other and unrecognized statuses remain grouped alphabetically. An empty status is labelled unknown. The Closed group starts collapsed on each launch, including when closed issues first appear after startup; other groups start expanded. Pane titles show the status and count. Each issue's type, priority, and status columns align across groups; the selected issue's details pane title shows its ID, status, and priority. Use `↑` / `↓` to move through visible issues and collapsed group headers. Expanded group headers are skipped, so navigation crosses directly between their issue rows; collapsed headers appear in order among the remaining issues, and movement clamps at the first and last target. A focused collapsed header is highlighted and does not select an issue or retain detail/delete identity. Press `Right` or `Space` on that header to expand the group and select its first issue; `Left` leaves it collapsed. Press `Space` on a selected issue or use `←` / `→` to collapse or expand its group; clicking a pane title also toggles that group. Collapsing a selected issue's group selects its collapsed header. If all groups are collapsed, headers remain keyboard targets; mouse title clicks can also reopen them. Refresh preserves focus on a collapsed header while its group still exists.

## Browsing and controls

### Ready tasks and scheduler

Press `Shift+R` on an open Task to mark it Ready; press it again to return it to Open. Ready is an OnTrack group between Open and the other Beads statuses. The underlying Beads status stays `open`; OnTrack stores the marker as the `ontrack:ready` label. Tasks already tracked as handed off cannot be marked Ready again.

The task form has an Agent harness choice after Priority. It defaults to Codex, can be changed to any of the listed Herdr kinds, or set to Unassigned. The choice is saved on the Bead as an `ontrack:agent:KIND` label and can be changed in Edit. A task needs a supported choice to be scheduled. Non-task Beads do not use this choice.

New child tasks keep ordinary labels from their parent, but start Open with their own harness selection; they do not inherit the parent's Ready marker or harness choice.

While OnTrack is running from a Herdr main checkout, its scheduler picks Ready tasks by Beads priority, then ID. It waits for the managed-worktree scan, checks that the chosen harness has a current Herdr integration, and uses the same tracked launch path as `h`. A successful agent handoff clears Ready; a parent prepared for children stays Ready until its children are closed and integrated, then resumes its prepared handoff. Mark both parent and child Ready for automatic child scheduling; the child waits until the parent's managed checkout exists. Failed or uncertain attempts show an error and are not retried repeatedly in the same app session. To retry after fixing the cause, turn Ready off and on; inspect `w` first if a worktree may already exist. Starting OnTrack again retries Ready tasks that have no recorded handoff.

The left pane lists all returned issues, including closed ones, without the CLI's usual list cap. Each entry shows its colour-coded issue type, priority, status, and a white title; the selected issue's ID appears in the details pane title. The rounded details pane shares the issue list's theme background. Its title shows the selected issue's ID, status, and P0–P4 priority; those fields are omitted from the body. It presents narrative fields as Markdown, while other metadata, comments, relationships, and unfamiliar nested values remain available as you scroll.

An Open issue shows `· sent` beside its status after Herdr confirms the initial agent prompt and OnTrack saves the handoff. The marker survives refresh and restart through the tracked worktree record. The Beads status still reads Open until the agent changes it; once it changes, the marker disappears. A handoff without confirmed prompt submission does not show the marker. Use `w` to inspect any incomplete or uncertain launch.

| Input | Action |
| --- | --- |
| `Ctrl+?` | Open the hotkey reference for the active issue pane, worktree view, or dialog. Keys appear in the left column and their actions on the right. Use `↑` / `↓` or `PageUp` / `PageDown` to scroll, `Home` / `End` to jump, and `Esc` or `Ctrl+?` to close it. The underlying selection and form text stay as they were. |
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
| `n` | Open the new-bead form. Tab / Shift+Tab moves through Title, Type, Priority, Agent harness, Description, Create, and Cancel. Left / Right cycles any focused dropdown choice with wraparound, immediately when closed or as a preview while open. Enter (or Space) opens the choice list; Up / Down previews options, Enter commits, and Esc restores the opening choice. In Description, Shift+Enter inserts a newline and Enter focuses Create; press Enter again to submit. An empty title shows a required message. |
| `Shift+N` | On a selected open task, open the same form for a child task. The form names its parent; creation records a Beads parent-child relationship. |
| `e` | Load the selected bead's current details and open the edit form. Edit Title, Type, Priority, Agent harness, and Description with the same controls as the new-bead form; Save updates that bead, while Esc or Cancel discards the draft. The loading dialog offers `e` to retry a failed detail load. |
| `d` | Ask to permanently delete the selected bead. `y` confirms; `n` or Enter cancels. Deletion does not cascade; dependency links are removed and dependents may be orphaned. |
| `h` | Immediately hand the selected bead to Herdr in the background. The top-right toast shows discovery, launch, and the result. No dialog or second confirmation appears. Repeated `h` during a pending handoff is ignored. |
| `Shift+R` in the issue list | Mark an open task Ready or return it to Open. This does not change the Beads status. |
| `w` | Open outstanding tracked worktrees. This list is independent of Beads statuses, so closing or deleting a Bead does not hide unfinished work. A checkout disappears after removal is verified, including when it was removed outside OnTrack; uncertain cleanup stays visible for recovery. Select an item and press Enter to focus its recorded Herdr workspace. A merge is a request, not proof of integration; inspect conflict/error states and recover in Git before retrying. Cleanup is separately confirmed and available only after verified integration. In this view, `Shift+R` opens a restart confirmation for the selected active handoff. |
| `Esc` | Go back one level: close a dialog (or its open choice list first), return from outstanding worktrees to the main browser, or stay on the main browser if already there. A dialog stays locked while a mutation is running. |
| `q` or `Ctrl+C` | Quit when no dialog is open. |

Inside the create and edit forms, keys are text unless they are form controls; app shortcuts do not trigger, and arrow keys continue to edit text or choose options. Dropdowns share one rule: Left / Right cycles choices with wraparound, whether the dropdown is closed or open. In text fields, Left / Right retain their cursor-editing behavior. Description wraps at word boundaries, splitting only words wider than the field. In Description, Up / Down moves the caret through the visible lines, including wrapped lines and lines separated by Shift+Enter. The caret keeps its display column when possible and stops at the first or last line. Shift+Enter requires a terminal that reports modified Enter keys; in terminals without that support, it may arrive as plain Enter. New beads default to Task, P2, and Codex as the agent harness. The edit form loads the latest `bd show` record before opening and preserves fields outside its five controls. Beads with unsupported types or priorities cannot be opened in the edit form. A mutation runs once at a time in the background; the modal is locked until it finishes. Failures remain visible and keep the form open for correction or retry. Successful creation refreshes the list and selects the new bead; successful edit refreshes the list and detail while retaining the edited bead. Dialogs ask you to enlarge a terminal that is too small to show the full controls and warnings.

The hotkey reference accepts terminal reports of `Ctrl+?`, `Ctrl+Shift+/`, and `Ctrl+/`. Some terminals send the same unmodified Backspace code for `Ctrl+?`; on those terminals the app cannot distinguish the keys, so use a terminal with modified-key reporting. The reference is unavailable while a mutation locks its dialog.

The settings dialog captures input until it closes, so `q` cannot quit while it is open. Theme changes and each cleanup toggle save immediately. All automatic cleanup options default off. Automatic merge may defer when there is no trustworthy evidence that the agent is idle; a closed Bead alone never proves it is safe to merge. Removing a checkout also closes its Herdr workspace. Settings load/save errors remain visible; failed saves preserve the last successfully saved settings. See the [settings guide](settings.md) for recovery.

## Generate tasks for an Epic

In the create form, choose **Epic** to reveal **Generate child tasks with a Herdr agent** below Description. Tab moves from Agent harness through Description to this option, then Create. The option starts off. Enter the proposal in Description, turn on the option, and create the Epic. OnTrack saves the Epic first, then opens a separate Herdr workspace at the current checkout, starts an available agent, and asks it to create direct child Beads with descriptions and acceptance criteria. It does not create another Git worktree. The form hides and clears this option for other issue types; editing a Bead does not run generation.

Keep OnTrack open while planning runs. The top-right message reports launch, completion, or a recovery error. After the agent finishes, OnTrack checks that at least one direct child exists and closes only the planning workspace. If creation, agent start, prompt delivery, or child verification fails, the Epic remains. An already created planning workspace stays open so you can inspect its agent and Beads. Check that workspace before retrying; a prompt failure may still have delivered the proposal, and another run can duplicate children. This workflow requires OnTrack to run inside Herdr with a current agent integration.

## Details, loading, and refresh

Selecting an issue immediately displays its cached list record while a richer `bd show` request runs. The returned detail record replaces that snapshot. ontrack requests long details, comments, and dependents; descriptions, notes, design and acceptance text render as Markdown. Metadata, comments, relationships, and unknown nested values are shown whenever present in the returned JSON, except for the top-level revision field. It does not invent missing fields or fetch data outside those CLI responses. Scroll the details pane to see fields below the visible area.

ontrack automatically polls the issue list and selected issue's details in the background, starting another refresh roughly one second after the previous refresh completes. Changes made by another terminal or agent appear without pressing `r` or configuring events, a watcher, or extra dependencies. Slow `bd` commands increase the delay; updates are not instantaneous or event-driven. Successful create/edit/delete actions in ontrack still trigger an immediate refresh, and `r` remains available for a manual refresh.

The footer reports refreshing, detail loading, ready, or an error. Refresh retains the existing list while loading and keeps the selected ID if it still exists in the new list; otherwise it selects the first issue. Automatic updates preserve group collapse state, pane focus, scroll position within available content, and active form input. A failed list or detail refresh keeps the last loaded data (or the available list snapshot) visible alongside the error and retries automatically. You can also press `r` after resolving the cause. An empty successful list displays “No issues found in this workspace.”

## Launch an agent with Herdr

Run ontrack inside a running [Herdr](https://herdr.dev/docs) shell in a Git checkout. Herdr must supply `HERDR_ENV=1`; `herdr` and the selected agent executable must be on `PATH`, and that agent's Herdr integration must already be installed and current. ontrack does not install integrations or grant Git safe-directory trust automatically.

Codex launches trust the verified main repository and disable startup update checks **for that invocation only**; no global configuration is changed. Source and new-worktree Git metadata must agree before Codex starts. This project trust may load repository configuration and hooks, but does not weaken sandbox or approval policy. OpenCode startup is unchanged. Authentication, administrator policy or unknown startup prompts may still need your attention; ontrack does not bypass them. Nonstandard or mismatched Git metadata stops startup safely and preserves any created worktree for inspection.

Handoff uses the selected bead and its mapped Herdr workspace, not global window focus. Pressing `h` starts discovery and launch without further input. If the Bead has an agent harness choice, OnTrack requires that exact current Herdr integration. Older Beads without a choice use Codex when available, then the first available integration. Discovery failure can be retried with `h` after resolving the cause. A valid launch creates a worktree from committed checkout content only, starts the chosen integration, and sends the issue context with instructions to inspect the authoritative bead and project guidance. The prompt asks the agent to commit and check finished work, then close the Bead as its completion signal. Blocked or unfinished work keeps the Bead open or in progress and reports the blocker.

For child tasks, press `h` on the parent after creating children. OnTrack prepares its worktree without starting an agent while children are open or their worktrees await integration. Then hand off each child; it starts from the parent branch, and its merge target is the parent worktree. The tracked-worktrees list places child handoffs directly beneath their parent with indentation. Close and integrate each child, then press `h` on the parent again to start its agent in the prepared worktree. Parent merge requests also wait for children. A child may exist in Beads before the parent worktree is ready; its handoff will ask you to prepare the parent first. Beads itself still lists the parent as ready: it disallows a blocking edge from a parent to its own descendant because that would also block the child. OnTrack enforces the child-first rule for its handoffs and merges; external Beads clients do not inherit this gate. An agent started on a parent before a new child is added cannot be paused automatically; OnTrack still blocks parent integration until the child is complete. Keep the parent checkout clean before starting or integrating a child. Remove child checkouts before closing or removing the managed parent checkout.

Only new handoffs recorded by OnTrack are managed in the outstanding-worktrees view; pre-existing worktrees are not adopted. The target branch comes from verified repository state and is shown before a merge request. Git integration—not Beads status—determines whether work is integrated. Conflicts and uncertain outcomes remain visible for manual recovery. Cleanup never force-removes a checkout or removes its branch.

An active checkout shows `IN PROGRESS` while its Bead is open or the recorded agent has not been verified as finished, even after commits appear on the task branch. It shows `READY TO MERGE` when the Bead is closed, the recorded agent has settled after prompt activity, and the clean task branch has commits outstanding against its target. Older handoffs whose launch response lacked a native agent session can use their recorded pre-prompt Herdr observation for this display state when the same pane and terminal report later activity. The details explain that automatic merge stays deferred for these handoffs; use a confirmed manual merge after reviewing the work. An unavailable Bead, working or mismatched agent, missing activity evidence, or untouched branch does not claim readiness. `READY TO MERGE` means the task work awaits integration, not that it has already merged. Manual merge requests remain available with confirmation, while the existing Git and agent checks decide whether the request can proceed.

In the `w` view, select a handoff with Up / Down and press Enter to switch to its recorded Herdr workspace. This uses the existing workspace; it does not create or reopen one. Closed workspaces cannot be focused, and Herdr errors appear in the view. The right pane shows its Bead title and description from the refreshed Beads list when available, plus the current worktree state and Git/Herdr details. If the Bead is unavailable, the pane uses the title and description captured at handoff. Older handoffs without captured text show an unavailable-text message; a Bead without a description shows the same fallback. PageUp / PageDown or the mouse wheel over the right pane scrolls long descriptions. Press `m` to request a merge, `c` to close its integrated Herdr workspace, or `x` to remove its integrated checkout; each action requires `y` confirmation. The agent merges the target into the task branch and resolves conflicts there; OnTrack then verifies current Git history and fast-forwards the target. Keep OnTrack running for reconciliation. Press `r` to refresh or `w` to return to Beads.

Press `Shift+R` on an active tracked handoff to restart its task. The dialog identifies the handoff and warns that confirmation with `y` discards all uncommitted checkout changes and closes the current Herdr workspace; `n`, Enter, or Esc cancels without touching Git or Herdr. OnTrack verifies the recorded checkout and workspace, force-removes that worktree, and launches the same live Bead with the recorded agent in a fresh branch from the current main checkout HEAD. The Bead and its status are retained, and the abandoned branch is kept for recovery. Restart is independent of integration status, unlike normal cleanup. If removal or relaunch is uncertain, the old record remains visible as Needs Attention; inspect the reported Herdr and Git state before taking manual recovery action. OnTrack never blindly repeats a restart.

Once OnTrack verifies checkout removal, that handoff leaves the `w` view immediately and stays hidden after refresh or restart. The record remains in the local registry. If later checks find a new problem, its `NEEDS ATTENTION` status makes it visible again. A closed Herdr workspace with its checkout still present remains visible.

OnTrack also checks recorded checkout paths during its regular worktree refresh. If a managed checkout was removed with Git or another external action, it records the missing checkout and removes it from `w` without requiring an automatic cleanup preference. This only tracks the checkout path: the source branch is retained, and its commits may still need integration. Check Git history before deleting that branch. A path that reappears is shown as `NEEDS ATTENTION` for identity review.

Worktree statuses appear as coloured text in both the list and details, on the pane's normal background. Merge Pending uses the theme warning colour, Integrated uses success, and states needing attention use danger. Other labels use the theme's info, secondary accent, or muted colours. The words remain visible, including when a row is selected, so colour is not the only state cue.

Ordinary uncommitted changes in either the task checkout or its target show `UNCOMMITTED CHANGES`, rather than `NEEDS ATTENTION`. The detail asks you to commit or resolve them before merging or cleanup. A merge request already in flight stays `MERGE PENDING` and includes the same warning. These labels do not make a dirty checkout eligible for a merge or cleanup; those actions still check that both checkouts are clean. Checkout identity errors and unfinished Git operations continue to need attention, and unresolved merge conflicts retain `NEEDS RESOLUTION` during a merge request.

During a requested merge, Git may briefly contain unresolved files or the target branch may advance while the agent is still working. When Herdr confirms the recorded agent is working, OnTrack keeps the label at `MERGE PENDING` and shows the Git concern in the details. If the agent is idle, done, blocked, or cannot be verified, the same unresolved Git state shows `NEEDS RESOLUTION`. That label calls for inspection; it does not mean OnTrack has verified that the agent failed. A pending label does not make the worktree eligible for integration or cleanup.

Untouched branches with no task commits are not automatically considered complete. Automatic operations defer without reliable native agent evidence; an explicitly confirmed manual merge can use Git verification without optional session metadata. Removal also closes an open Herdr workspace, so enable removal before closing if both are wanted. Removal after a workspace has already been closed is unsupported: inspect the reported checkout, verify its current changes are integrated and clean, then use Git's exact-path, non-force worktree removal manually. OnTrack never guesses a replacement workspace ID.

Title-only or incomplete beads are supported: the agent is instructed to infer a useful description, scoped requirements and testable acceptance criteria from the live bead and project context, record assumptions, and save missing fields with `bd update` before implementation. It must preserve existing requirements, avoid unrelated scope, and not refuse solely because description or criteria are missing. Only consequential ambiguity unresolved by context warrants clarification; useful inferred details, assumptions and questions should still be saved first. ontrack sends these instructions rather than generating fields itself; handoff success does not prove the agent followed them.

Start ontrack from the **main checkout** (or an ordinary subdirectory) to hand off work. Launch discovery from existing linked checkouts, submodules or nested repositories is unsupported and stops before creation; Herdr 0.9.1 reports the main source checkout for linked paths, so ontrack will not guess a linked workspace. Newly created worktrees remain the verified targets for agent startup.

Contributors can run `cargo test` from a linked Git worktree. The Herdr test fixtures model a main checkout independently of the directory where Cargo runs; this test support does not change the launch restriction above.

The new checkout contains committed Git content only; uncommitted changes are not copied. A successful handoff does not mean the agent completed the bead. Failures may leave a worktree or may have delivered a prompt. Inspect Herdr/Git worktree lists and the reported workspace/branch before retrying; nothing is automatically deleted or blindly resubmitted.

The top-right toast stays visible during discovery and launch, then shows success for seven seconds or failure for fifteen seconds. The footer also shows the result or error while space permits, and `w` shows recorded worktrees for recovery. Compact terminals shorten the toast with a pointer to the footer and tracked worktrees. If a failure may have created a worktree or sent a prompt, inspect the recorded and Herdr/Git state before pressing `h` again.

Backend work does not run on the terminal input loop, so browsing remains responsive while a command is slow. Handoff keeps the browser usable; create, edit, and delete mutations still lock their dialogs until a result arrives. Worker requests are queued and executed in order without coalescing. Quitting restores raw-mode, mouse-capture, and alternate-screen settings through the application's cleanup path.

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
