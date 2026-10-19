# Development guide

ontrack is a single Rust 2024 binary requiring Rust 1.88 or newer. Runtime workspace integration uses the public Beads CLI; no direct database integration is needed. Browse operations are read-only. Create, edit, delete, and Ready toggles are user actions; the scheduler automatically hands off Ready tasks through the existing tracked Herdr path. Consult the [official Beads documentation](https://beads.gascity.com) for CLI and workspace behavior and the [user guide](user-guide.md) for running the browser.

Tagged builds and release asset naming are documented in the [release guide](releases.md); installer behavior and offline checks are in the [installation guide](installation.md#release-asset-contract-for-maintainers). The workflow uses `scripts/release_asset.py` for tag validation, archive creation, and checksum verification; keep its target list aligned with the native runner matrix in `.github/workflows/release.yml`.

## Modules

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Dispatches `update` and `uninstall` before checking the Herdr launch context and Beads workspace; initializes, runs, and restores the browser terminal. |
| `src/startup.rs` | Requires the Herdr pane marker, then discovers the active workspace through Beads; prints manual initialization guidance and exits when none exists. |
| `src/uninstall.rs` | Validates and removes the running executable and user settings after explicit CLI confirmation, before Beads discovery or terminal setup. |
| `src/settings.rs` | Loads and atomically saves theme, default-agent, agent-limit, and worktree-cleanup preferences under the user's XDG config directory. |
| `src/theme.rs` | Defines the bundled theme catalog, stable IDs, display names, and semantic color palettes. |
| `src/beads.rs` | Builds issue CLI commands, runs `bd`, validates JSON responses, and preserves complete returned issue records. |
| `src/screenshots.rs` | Reads clipboard images, writes local screenshots into the shared Beads workspace, and parses description references for agent prompts. |
| `src/herdr.rs` | Discovers usable agent integrations, maps the current checkout to a Herdr workspace, focuses a selected managed workspace, and explicitly creates a worktree and starts/prompts an agent. |
| `src/planning.rs` | Runs interactive Herdr planning sessions, parses agent questions and plans, and creates reviewed Beads. |
| `src/worktrees.rs` | Journals OnTrack-created handoffs separately from Beads and verifies Git/Herdr evidence before merge finalization or cleanup. |
| `src/app.rs` | Holds selection, focus, scrolling, modal form and handoff toast state, cached data, loading/error state, and pane hit-test rectangles; handles input and background worker requests/responses. |
| `src/event.rs` | Polls crossterm events with a timeout and recognizes quit keys. |
| `src/update.rs` | Reads the official GitHub release, selects a compatible asset, verifies SHA-256, and atomically replaces the executable. |
| `src/update_cli.rs` | Runs the explicit update command, reports release status, and confirms installation. |
| `src/hotkeys.rs` | Maps the active pane or dialog to its keyboard command list and recognizes modified help-key reports. |
| `src/ui.rs` | Renders splash, issue list, wrapped Markdown details, status, and top-right handoff toasts; maintains list visibility and scroll bounds. |
| `src/detail.rs` | Presents complete issue records with Markdown rendering for narrative fields and readable structured fallbacks for metadata. |
| `src/terminal.rs` | Manages raw mode, alternate screen, mouse capture, and cleanup, including initialization failures and panic handling. |

Interactive `p` planning uses a dedicated Herdr tab in the inherited workspace. `planning::launch` validates the live workspace, selected agent, and returned tab/pane identities before prompting; it favors Codex and applies the same repository trust preflight as Epic generation. Agent turns must end with a single-line JSON response tagged by the current round marker. `planning::read` uses Herdr's unwrapped terminal output to extract that marker, validates questions or complete Task/Epic plans, and ignores prior rounds. The app polls through a background request while its planning view remains responsive. The agent is explicitly told to discuss only; `planning::create` is the sole issue creation step, reached after `y` on the review screen. Beads text is passed as argv values, never shell code. Each planned item receives description, acceptance criteria, and P0–P4 priority; child Tasks receive the root Epic ID as parent. A partial failure reports created IDs and disables retry in that dialog. Cancellation leaves the Herdr tab available for inspection without creating Beads. See [Herdr's API guide](https://herdr.dev/docs/socket-api/) for `agent.prompt` and terminal reads, and [Beads documentation](https://beads.gascity.com/) for issue creation.

`app::group_statuses` seeds the six standard issue panes before adding statuses from the current Beads list. The shared `list_layout` therefore gives empty panes a two-row border and a header target with count zero. Unknown and custom statuses are added only when present. Keyboard navigation includes empty headers, while issue actions still require a selected Bead. On the first successful list response, Closed becomes collapsed even if it has no issues; refresh retains collapse and empty-header focus through later list changes. Keep `src/ui.rs` header highlighting and scroll positioning aligned with these targets.

`src/ui.rs` renders the supplied centered ONTRACK wordmark and a terminal-drawn anime girl during the 1.2-second startup splash. `girl_frame` chooses the blink and wave pose from elapsed time supplied by `app::run`'s existing redraw loop, so the animation needs no separate timer or state. If the full wordmark and character do not fit, the renderer uses the compact ONTRACK title with the character when possible, then a text-only fallback. Width checks use terminal cell widths; preserve the wordmark verbatim, keep the splash centered and theme-aware, and ensure it yields to the normal browser view after startup.

Every `Block` in `src/ui.rs` that draws a full border sets `BorderType::Rounded`. This includes worktree panes, modal frames, form fields, planning, help, and the issue detail pane. Keep borderless backgrounds and compact-terminal fallbacks borderless; preserve existing titles, colours, and layout when adding a bordered widget.

`ontrack update` handles release discovery before Herdr, Beads, or terminal browser initialization. Browser startup does not make a release request.

In the managed-worktree view, Enter queues `herdr workspace focus WORKSPACE_ID` on the existing background worker. The ID comes from the selected OnTrack handoff, never global focus. The adapter passes arguments separately, rejects unsafe IDs, and requires Herdr's `workspace_focused` result to repeat the selected ID. Closed workspaces and removed checkouts are rejected before dispatch. Focus success and errors are shown in the view without changing Git, Beads, or the handoff registry.

`theme::worktree_status_color` maps every `WorktreeStatus` to a semantic palette role. `ui::worktree_status_style` applies that colour to the label's foreground characters and leaves the pane background intact; the surrounding selected-row highlight remains separate. Keep list and detail labels in sync when adding a worktree state. Theme tests enforce that pending and attention colours differ from integrated in every bundled theme, and UI buffer tests check all list labels and selected details across bundled themes.

`worktrees::inspect` recognizes a source checkout's `MERGE_HEAD` only when it equals the verified target HEAD, the target checkout is clean, and no OnTrack merge request is active. It displays `MERGE IN PROGRESS` without changing `verify` or action eligibility. `ontrack status <working|merging|blocked|clear> [detail]` writes an advisory `agent_report` to the existing locked registry after matching the caller's canonical Git top level to exactly one active managed checkout and verifying its identity. Reports expire from display after 15 minutes and are skipped for completed or identity-error states. `list_with` and reconciliation append them only to display details; merge and cleanup paths never consume them. Tests should keep these boundaries explicit.

`theme::status_color` gives the synthetic Ready issue-browser status a readable blue for light and dark themes. The same mapping colours the Ready group border and issue-row status text; it does not change the underlying Beads status or worktree status colours. Theme tests check contrast on both browser surfaces, and UI buffer tests check the rendered Ready border and row text across bundled themes.

`ui::footer_text` and `ui::worktree_footer_text` render static shortcut hints; the issue footer advertises `g` only when the selected Bead is an Epic. Both browser footers occupy one terminal row and never consume errors, notices, focus feedback, or confirmation text. `render_header_feedback` puts errors and action feedback on the second header row; confirmation instructions stay in dialogs. Worktree selection state and detail remain in the two panes. Keep footer hints within terminal cell width and prioritize focus, merge, cleanup, restart, detail scrolling, and help on compact terminals. The worktree footer intentionally omits refresh, selection, quit, and back hints, including aliases; keep those commands in `hotkeys::Context::Worktrees` and the existing input handling. Use the hotkey overlay for the full command list, and update its context when input handling changes.

Theme is explicit app state (`App::theme`) threaded into rendering; do not introduce global or thread-local palette state. Keep Dark as the existing default look. New colors need readable Light equivalents for surfaces, text, semantic metadata, startup splash, details, and dialogs. Settings changes immediately attempt persistence; update `App::settings` and the active theme only after `settings::save_to` succeeds. Failed writes retain the candidate for retry without changing the active theme. Tests should use isolated settings paths rather than changing process-wide environment variables.

Choice controls use the shared `cycle_choice` helper for Left / Right navigation. Both directions wrap at the ends; a closed focused dropdown changes immediately, while an open dropdown previews changes until Enter accepts or Esc restores its opening value. Keep this consistent for future dropdown controls. Up / Down behavior remains control-specific.

The create/edit Description field uses `description_layout` to insert visual line breaks before whole words and map its byte cursor to display cells; words wider than the field still wrap by character. `wrapped_description` renders that layout, and `move_description_vertical` uses its byte-boundary positions so Up / Down follows the same visual rows at the form's fixed 64-cell text width. Vertical movement retains a preferred display column across shorter rows, clamps at the top and bottom, and leaves the stored text untouched. Other cursor movement or editing clears that preference. Explicit newlines remain in the stored text. While Description has focus, Shift+Enter inserts a newline and plain Enter focuses the form's action without submitting. Terminal setup enables Crossterm's keyboard enhancement protocol so supported terminals can distinguish modified Enter; restoration pops that mode on normal exit, initialization failure, and panic. Terminals without modified-key reporting cannot distinguish Shift+Enter from Enter.

The hotkey reference is a separate `App::help` overlay, not a modal replacement. `App::key` handles its toggle and scroll keys before dispatching to the underlying pane or dialog; the mutation lock still takes precedence. `hotkeys::Context` captures the current pane or dialog when opened and owns its key/action list. Add new keyboard commands to the matching context when changing input handling. `ui::render_help` wraps the right column, clamps scrolling to the rendered lines, and renders over either browser view. Enhanced keyboard reports distinguish `Ctrl+?` or `Ctrl+Shift+/`; legacy terminals may report it as plain Backspace, which must remain text deletion/navigation rather than opening help.

The `s` settings dialog saves each theme/preference change immediately, writing the complete Settings value. Up / Down navigates rows, Left / Right changes theme, default agent, or agent limit, and Enter / Space toggles cleanup preferences. Failed saves must not update committed settings. The default-agent setting uses stable supported kind IDs; `automatic` preserves the old Codex/first-available form behavior, `unassigned` selects no harness, and an unavailable preferred kind falls back only for the current form. Agent limit controls automatic Ready-task scheduling. See the [settings guide](settings.md) for details.
Settings rows in `src/ui.rs` share a two-thirds title column and one-third
value column. `settings_row` aligns titles to the left and centers controls
by terminal cell width; keep dropdown entries on the same value boundary and
retain the existing compact-terminal fallback when changing this layout.

## Beads CLI contract

For an interactive browser launch, `main` first calls `startup::ensure_herdr_workspace`. It accepts only `HERDR_ENV=1`, the marker Herdr supplies to managed pane processes. Missing or other values return an error with launch guidance before any `bd` command, raw mode, or splash. Help and uninstall dispatch before this guard. The guard checks the inherited marker and does not query Herdr's socket or infer a workspace from global focus.

Before terminal initialization, `startup::ensure_workspace` runs only `bd --no-color where --json`. Successful discovery reuses the existing workspace with no configuration writes. Only the structured `no_beads_directory` error prints manual initialization guidance and returns `Ok(false)`; `main` then exits successfully before raw mode, the splash, or browser startup. Permission, malformed-output, and other discovery failures remain errors. Beads owns ancestor, worktree, and redirect discovery.

Missing-workspace guidance asks the user to run the literal command `bd init`, as requested by the startup UX contract. The command is bold on terminal stdout and plain in redirected output. Startup reads no stdin and never invokes init, init help, or configuration writes. Existing workspace settings, including JSONL auto-export, are preserved; `.beads/issues.jsonl` remains optional interchange data, never the source of truth. Global OnTrack preferences remain separate.

Focused `cargo test startup::tests` covers accepted and rejected Herdr markers, then injects command execution and captures output to cover missing-workspace early exit, terminal highlighting, plain redirected output, the single discovery command, existing/inherited/redirected workspace bypass, malformed discovery, other discovery errors, and an unavailable CLI. No live project initialization occurs in unit tests. For a manual terminal check, run the built binary from a disposable directory with no ancestor Beads workspace inside Herdr: verify the highlighted instruction, immediate successful exit, and absence of setup prompts, alternate-screen entry, or workspace files. Redirect stdout to verify plain output. Launch from an existing workspace to verify normal splash/browser startup and terminal cleanup on quit.

`ontrack uninstall` is dispatched before workspace discovery and terminal setup. It uses the running executable path and `settings::config_path` to select only the current installation and OnTrack user settings; preflight rejects unexpected file types and symlinks. The executable is removed last and checked again against its original file identity. `cargo test --test uninstall` copies the built binary into disposable directories and verifies confirmation, cancellation, missing settings, unsafe paths, and preservation of sibling `bd` and Herdr fixtures.

### Ready and agent harness labels

OnTrack uses `ontrack:ready`, `ontrack:agent:KIND`, and `ontrack:unassigned` labels instead of adding Beads statuses. `beads::is_ready` recognizes open Tasks and Epics for UI grouping. `App::poll_scheduler` prepares a Ready Epic with Ready descendant Tasks, then launches eligible Tasks after a managed-worktree scan. Creation writes the selected agent or explicit Unassigned label; editing updates those labels while preserving the Ready marker. Legacy or externally created Ready Tasks with neither label use the configured default agent when available, then the Codex/first-available fallback. Explicit Unassigned and unsupported harness labels do not launch. `r` (or `Shift+R`) in the issue browser adds or removes Ready using a guarded `bd update`. A Ready Task with a confirmed sent handoff cannot be cleared; an in-flight handoff is also protected until its outcome is known. The guard uses the same accepted handoff state as the `· sent` marker. Prepared and uncertain attempts remain eligible for the documented off/on retry after they settle. For an Epic, the worker reads the uncapped live Beads list, finds all open descendant Tasks through the parent hierarchy, skips known handoffs when adding Ready, preserves sent or in-flight descendants when clearing Ready, and updates the Epic and eligible Tasks in one `bd update` call. Beads applies multi-ID updates per issue, so any error forces a refresh to reveal partial changes. All values are passed as separate `Command` arguments. When a create form opens, the background worker calls `herdr agent start --help` and `herdr integration status`, then intersects supported kinds, current integrations, and OnTrack's known harness labels. This check does not require Herdr workspace mapping. The form starts with Unassigned while discovery runs and blocks task submission until it finishes. Empty or failed discovery leaves Unassigned as the only choice, with an error shown for failures. Responses carry a form sequence so late results cannot change a reopened form. Existing edit forms keep their stored choice and the full known-kind list; launch discovery still validates availability before handoff.

`App::poll_scheduler` runs after responses and worktree polling in the terminal loop. It waits for a successful managed-worktree scan before dispatch. A Ready Epic with a Ready descendant Task and no recorded worktree is prepared first through the normal tracked handoff path; its open children cause a reservation with no agent start, even when the agent limit is full. Subsequent scans make children eligible. Task selection follows priority and ID, one selection per poll, while distinct handoffs can be in flight concurrently within the agent capacity. It remembers attempted IDs in the running process and skips all journaled handoffs, including uncertain ones. A prepared parent can resume only after its open descendants and pending child worktrees clear. A successful agent launch retains Ready while the Beads status remains `open`, for both scheduled and manual handoffs. When the agent changes the status to `in_progress`, `beads::is_ready` becomes false and the task appears in In Progress; the Ready label does not need a second mutation. A merely prepared parent also retains Ready. On failure, the Ready marker remains for explicit retry. If the worktree scan fails, dispatch pauses. Herdr's launch journal remains the durable duplicate guard across app restarts and concurrent processes.

The subprocess inherits the application's working directory. Commands are passed as argument vectors, not shell strings:

```sh
bd --readonly --no-color list --all --limit 0 --json
bd --readonly --no-color show --id=ISSUE_ID --json --long --include-comments --include-dependents
bd --no-color create --title=TITLE --description=DESCRIPTION --type=task --priority=2 --json
bd --no-color create --title=TITLE --description=DESCRIPTION --type=task --priority=2 --json --labels=ontrack:agent:codex
bd --no-color create --title=TITLE --description=DESCRIPTION --type=task --priority=2 --json --parent=PARENT_ID --no-inherit-labels --labels=ontrack:agent:codex
bd --no-color update --title=TITLE --description=DESCRIPTION --type=task --priority=2 --json -- ISSUE_ID
bd --no-color update --add-label ontrack:ready --if-status open --status open -- ISSUE_ID
bd --no-color update --remove-label ontrack:ready -- ISSUE_ID
bd --no-color delete --force --json -- ISSUE_ID
```

Create returns a JSON object containing the created ID; update and delete are successful on a successful command exit. Mutations are not passed `--readonly`; update sends the form fields and changes the harness label only when its choice changes, and delete uses `--force` for the selected issue only and never requests cascading deletion. The worker processes queued requests sequentially, preserving mutation order. Edit opening fetches the current `bd show` record on that worker, validates the editable type and priority, and ignores stale load responses using a request sequence. A failed update leaves the draft open; success triggers an immediate refresh and reselects the same ID.

`Shift+N` opens the create form with an immutable selected parent ID. Child creation passes `--parent=PARENT_ID` as its own argument, so Beads records the hierarchy and list/show JSON exposes the child's `parent` field. It fetches the parent labels, uses `--no-inherit-labels`, then explicitly carries over ordinary labels; the parent's Ready, harness, and explicit Unassigned labels are excluded so the new child starts Open with its own selected harness. OnTrack reads the uncapped live list to find open descendants before a parent starts or merges. Beads rejects a `blocks` dependency from a parent to its descendant because blocked state would cascade back to that descendant; its `bd ready` output still includes the parent. The child-first gate is therefore enforced by OnTrack, not by a reverse Beads dependency.

`--all` includes closed issues and `--limit 0` removes the default list cap. `--id=ISSUE_ID` is a single argument. The list and show responses must be JSON arrays. List parsing requires nonempty string `id`, `title`, and `status`; a missing or null priority defaults to 2, and supplied priority must be an unsigned integer. Each `Bead` retains the entire JSON object as well as its summary fields.

The Ready scheduler counts accepted managed handoffs with `InProgress` or
`DirtyCheckout` display status plus all pending agent launches before selecting
another task. Pending manual launches count too; preparing an Epic checkout with
open children does not reserve an agent slot. A successful agent start retains a
capacity reservation until a fresh durable worktree scan acknowledges it, even
if an older scan was already in flight. Reserved parent worktrees and finished or cleanup states do not take a
slot. The setting is global to the user and defaults to five; Unlimited bypasses
the count. Explicit `h` launches bypass the gate but count while active. A full
limit leaves Ready labels and the existing priority and ID ordering untouched.

The issue list layout sorts each status pane by the visible ancestry derived from `parent` in the uncapped list records. It keeps the existing status and ID order for unrelated roots and siblings, places descendants after their parent, and uses the ancestry depth to indent titles. An absent parent or one in another display-status pane starts a visible root; the title row shows that parent ID so the relationship remains apparent. Selection and hit testing use the layout's issue indices, so sorting the display rows does not change the underlying Beads records or Ready scheduler order.

Detail parsing selects the requested ID, rejects duplicate matching records, and reports a missing ID. It retains the returned JSON intact, including comments, relationships, metadata, and fields unknown to ontrack. The detail renderer omits only the top-level `revision` field from display, for both cached list records and full show records; nested revision fields and other metadata remain visible. Process-start errors, unsuccessful exit statuses with stderr, and invalid JSON/records are surfaced as errors rather than panics.

## Epic planning tab

Existing Epic task generation uses `Request::GenerateEpic` after a readable `Modal::ConfirmGenerateEpic` receives explicit confirmation. Both this request and creation-time `Request::CreateEpic` call `spawn_epic_planning`, keeping Herdr work off the sequential request worker. `App::planning_epics` guards repeated generation for each Epic until launch failure or completion; creation-time planning also registers its ID. This guard is process-local. `epic::launch` fetches the live issue and rejects non-Epics or empty proposals before creating a tab. Launch and finish responses share progress/error feedback and completion refresh. The confirmation retains the original Epic ID, wraps its title and warnings to compute required height, and disables confirmation until fully renderable. Epic-specific hotkey contexts expose `g` in either issue pane, with a separate confirmation context for its controls.


`src/epic.rs` handles opt-in Epic planning separately from the tracked worktree handoff. The create worker persists the Epic first, then a separate thread validates the live Epic and originating Herdr workspace, discovers a current agent integration, and calls `herdr tab create --workspace SOURCE_ID --cwd CURRENT_CHECKOUT --label ... --no-focus`. Herdr workspaces have no parent parameter; a tab is the nested planning context and shares the checkout. No `herdr workspace create` or `herdr worktree create` call is involved. The agent is started and prompted by exact pane ID; the proposal comes from the live Bead Description, and the prompt requires `bd create --parent=EPIC_ID` children with an explicit `--priority=N` each. It asks the agent to compare impact, urgency, and enabling work and vary priorities when supported by the proposal, while allowing equal priorities when the evidence does not justify a distinction. This is guidance to the planning agent, not a deterministic ranking or a post-creation rewrite. The launch test checks that these instructions reach the prompt.

Before creating a Codex planning tab, `epic::launch_with` calls `herdr::codex_startup_trust` through its backend. This shares the normal handoff's canonical Git identity resolver and TOML key encoder, including linked-checkout resolution to the verified main repository. Validation failure creates no tab. Codex starts with `-- -c 'projects."VERIFIED_MAIN_ROOT".trust_level="trusted"' -c check_for_update_on_startup=false`; other agents receive neither overrides nor Git trust lookups. The settings are invocation-local and use the [Codex configuration keys](https://learn.chatgpt.com/docs/config-file/config-reference). They do not modify persistent configuration or sandbox/approval policy. Herdr's successful `agent start` response establishes interactive readiness; its workspace, tab, and pane must match before the planning prompt is dispatched once to that exact pane. Startup errors retain the new tab and identify the pane to inspect for authentication, enforced policy, or unsupported dialogs; no prompt is dispatched and no automatic retry occurs. Tests cover argument boundaries, prompt dispatch, fallback agents, trust failures before creation, and startup failure/identity mismatch. The shared resolver/encoder tests cover Git metadata rejection and unusual path escaping.

The planning thread waits for the recorded agent to finish, reads the live Beads list, and closes the new tab only after finding a direct child. If `herdr agent wait` fails because the agent exited, it reads the original pane with `herdr pane get`; a matching workspace, tab, and pane with no agent is accepted as settled, subject to the same child check. A missing pane, a replacement agent, a blocked agent, or absent children remains a recovery error, and the planning tab stays open. Before closing, it confirms the tab still belongs to the source workspace and another tab remains there; this avoids closing the last tab and its workspace. Failures report the Epic, workspace, and tab for inspection without an automatic retry. The thread runs within OnTrack's process, so closing OnTrack before generation finishes stops its automatic cleanup. Normal create and edit requests retain their previous path. The UI exposes the toggle only on Epic creation and clears it when the type changes away from Epic.

## Herdr launch adapter

`src/herdr.rs` exposes `discover() -> io::Result<LaunchContext>` and `launch(context: &LaunchContext, agent: &Agent, bead: &Bead) -> io::Result<LaunchOutcome>` for background-worker use. `LaunchContext` contains a `Workspace` and available `Agent` records (each with string `id` and `label`); `LaunchOutcome.message` carries the handoff result and `LaunchOutcome.reserved` distinguishes a prepared parent from a started agent. These public records derive `Debug`, `Clone`, `PartialEq`, and `Eq`. The app queues the full selected Bead and typed context/agent; the backend still captures authoritative `bd show` JSON before mutation and revalidates workspace/agent availability.

Restart is a separate confirmed worker request for a selected tracked handoff. The backend requires an active journal entry, verifies the main checkout identity and clean current target branch, checks the recorded checkout path/branch and Herdr ownership, and fetches the live Bead before mutation. It also checks that the recorded agent integration and source workspace are available. It saves a `restart_pending` marker before invoking `herdr worktree remove --workspace ID --force`, captures the response, and verifies both Herdr and Git inventories plus path absence. Only then does it mark the old handoff restarted and use the ordinary launch path with a new unique name and the live Bead. The old Git branch is retained. An uncertain removal is never retried automatically. A failed relaunch records an attention state; any partial new launch has its own normal launch journal for recovery. Normal cleanup still requires verified integration and never uses force.

Pressing `h` records the selected Bead and queues discovery once. Repeated `h` for that pending Bead is ignored; another selected Bead can launch independently. Pending entries are keyed by launch sequence and carry their captured Bead. The discovery response requires the Bead's selected harness when present. Legacy Beads without a harness use Codex if present, otherwise the first agent in the sorted discovery list; a valid choice immediately queues launch. Missing selection, discovery errors, and empty agent lists stop before launch. Stale or duplicate responses cannot dispatch another launch or clear another Bead's pending state. The newest handoff owns the progress/result toast; an older completion does not overwrite it. Durable worktrees refresh after success or uncertain failure; a scan already in flight triggers a trailing refresh. Ready removal protects every pending Bead, including descendants affected by Epic Ready toggling. The authoritative registry, not the session-local pending map, continues to prevent duplicate remote work across processes/restarts.

LaunchJournal retains repository/entry identity, captured Bead context, parent target/base, and immutable reserved-target snapshots rather than a locked Registry. Each mutation reloads the latest registry, verifies its own intent/ownership, updates only that entry, and atomically saves under an exclusive lock. Git/Herdr verification runs outside launch write transactions. Returned creation handles are saved before checkout verification; start and prompt possibly-sent fences precede dispatch; raw start/readiness evidence precedes prompting; `active` follows a validated prompt response. A reserved-parent resume atomically claims the still-reserved entry at the start fence; concurrent stale handles cannot start twice or overwrite the winner's evidence. Journal transactions retry lock contention for about two seconds on background threads, then fail closed. Managed merge/reconciliation still retains its existing longer lock and can exhaust this window. Post-create/start/prompt journal/readiness failures are typed uncertain; failure-recording errors are appended without erasing the original recovery error or declaring the attempt retry-safe.

Discovery is read-only. Supported kinds are parsed from the installed CLI's `herdr agent start --help` list and intersected with **text** `herdr integration status` entries in the observed `current (vN) (path)` format. Availability is not hardcoded: any supported current integration, including Hermes, can be selected when Codex is unavailable. Outdated/not-installed entries and unsupported kinds are excluded; malformed/duplicate current entries and incompatible help/status output fail closed. No integration is installed or repaired automatically. A current integration establishes integration configuration, not a guarantee its agent executable can start.

Workspace selection uses `herdr worktree list --cwd CURRENT_DIRECTORY` and its JSON envelope's `result.source.source_workspace_id`, never global focus, labels, a live agent list, or inherited workspace variables. An absent/null/unsafe workspace ID fails with guidance to open the checkout in Herdr. **Launch discovery from linked checkouts is unsupported:** observed Herdr 0.9.1 responses report the main `source_checkout_path` even when `--cwd` names a linked checkout, so that field does not independently identify the linked workspace. Launch from the main checkout instead. The absolute, accessible, canonical source path must contain the current directory and have a normal `.git` directory. Any intervening `.git` boundary (including a nested linked checkout/submodule) fails before creation, without guessing another workspace. Ordinary main-checkout subdirectories remain supported. Read-only output for the main checkout returned `source_workspace_id=wY` and `source_checkout_path=/home/thomasr/Documents/dev/OnTrack`.

Before any mutation, launch retrieves `beads::show_bead(id)` and composes a prompt with the complete captured JSON. It re-runs discovery and verifies the selected workspace ID and agent against that fresh result. Generated agent names are at most 32 ASCII characters: `ot-`, up to eight trailing alphanumeric bead-ID characters, a separator, and a base32 nonce retaining the 32-bit PID plus a monotonic 64-bit time/counter. Names must start with a lowercase letter and contain only lowercase letters, digits, hyphens or underscores; overlong/unsafe names are rejected before creation. Branches already listed for worktrees are rejected rather than silently reused. It invokes these commands as argument vectors using `std::process::Command`, never shell strings:

```sh
herdr worktree list --cwd CURRENT_DIRECTORY
herdr agent start --help
herdr integration status
herdr worktree create --workspace SOURCE_ID --branch ontrack/UNIQUE_NAME --label UNIQUE_NAME --no-focus
herdr agent start UNIQUE_NAME --kind codex --pane NEW_ROOT_PANE_ID --timeout 30000 -- -c 'projects."VERIFIED_MAIN_ROOT".trust_level="trusted"' -c check_for_update_on_startup=false
herdr agent prompt NEW_ROOT_PANE_ID PROMPT
```

Herdr 0.9.1 has no `worktree create --json` flag and treats `--workspace` and `--cwd` as mutually exclusive selectors; creation uses only the explicitly validated workspace. The example starts Codex; other discovered kinds use the same start contract without Codex-specific arguments. Prompt submission targets the newly returned pane ID, not an agent name or focused pane. The prompt carries the full Bead JSON, including comments, relationships, acceptance criteria, unknown fields, and the selected ID/title, as one argument. It asks the agent to read project instructions, inspect authoritative details, implement the task, and report checks/blockers. Issue text never becomes shell syntax or additional command options. Launch never supplies `--trust-repository`, changes focus, closes existing panes, or deletes worktrees. Git safe-directory trust remains an explicit user decision, separate from the authorized invocation-only Codex project trust below.

For title-only or incomplete live Beads, the prompt instructs the agent to infer an actionable description, scoped requirements and testable acceptance criteria from the live title and project context, explicitly record assumptions, and persist missing fields with `bd update` successfully **before implementation**. Existing nonempty requirements must be preserved and unrelated scope must not be invented. Missing description/criteria alone is not grounds for refusal; clarification is reserved for consequential ambiguity that context cannot reasonably resolve, after saving a useful inferred description, assumptions and open questions to the Bead. IDs and field values must remain safely separated arguments, not shell interpolation. This is agent prompt behavior, not app-side field generation or a new create-form requirement. Regression tests cover title-only, null/blank fields and detailed payload preservation; they verify the submitted instructions, not external agent compliance.

Only Codex receives the two `-c` overrides after Herdr's `--`: invocation-only project trust for the verified canonical **main repository root**, plus `check_for_update_on_startup=false`. The shell quotes in the example merely illustrate argument boundaries; implementation uses separate argv strings, with the path encoded as a TOML quoted key (quotes, backslashes, Unicode and control characters are handled). Herdr 0.9.1 forwards these arguments; the verified Codex 0.162.1 loader applies CLI overrides before project trust is evaluated. OpenCode and other discovered integrations receive no additional arguments or Git trust lookups. No positional prompt is added to startup: the full captured prompt is still separately submitted once to the verified created pane.

Codex trust resolution first checks the source checkout, then checks the **actual created worktree path** before agent start. Read-only `git -C PATH rev-parse --path-format=absolute --git-common-dir` must identify the same canonical common directory for both. Only the normal non-bare `MAIN_ROOT/.git` layout is accepted. The common directory must be a real directory named `.git`; the main root must independently report its own exact top-level, matching common directory and matching absolute Git directory. Checkout top-level and non-bare checks also apply. Thus linked-worktree `--show-toplevel` alone cannot become the trust key, and arbitrary/nonstandard metadata parents are not trusted. Missing, redirected, mismatched or inaccessible evidence fails safely; source failures precede creation, while created-worktree failures retain recovery details and never start an agent or send a prompt. The resolver follows the [Codex main-repository trust model](https://github.com/openai/codex/blob/main/codex-rs/git-utils/src/trust.rs), but intentionally rejects nonstandard metadata layouts rather than guessing.

This authorized trust can enable the repository's project configuration and hooks for that Codex invocation. It writes no persistent/global configuration, grants no wildcard trust, does not pass Herdr `--trust-repository`, and does not relax sandbox or approval policy. Authentication, administrator-enforced policy and unknown startup dialogs can still require human intervention; no screen scraping, key injection or auth bypass is attempted. Git trust and metadata are not protected by a cross-process transaction, so external filesystem changes remain a race limitation.

The installed `herdr api schema --json` (protocol 22) specifies a `result.type=worktree_created` response with required `workspace`, `tab`, `root_pane`, and `worktree`. Exact IDs are `workspace.workspace_id`, `tab.tab_id`, and `root_pane.pane_id`. The adapter requires matching nested workspace/tab identities, a workspace not already listed as open for this repository, an absolute different worktree path, and the requested branch. It never substitutes the current/focused pane when a response is missing or incompatible. Herdr checks that the created pane is at an interactive shell prompt before starting an agent. Start/prompt responses must have `agent_started`/`agent_prompted` types and identify that same workspace/pane. Future incompatible schema changes are errors, not guessed aliases.

Creation/start/prompt are sequential and stop on failure. Errors identify the failed stage; after creation is attempted they retain the worktree and report source/new workspace, root pane when known, branch, agent name, and recovery guidance. A failed creation command can have partial side effects; inspect `herdr worktree list` and `git worktree list` before retrying. A startup error can leave an agent running. A prompt failure or unparseable response does **not** prove non-delivery: inspect the agent/pane before resubmitting. Nothing is automatically deleted.

Agent start waits up to 30 seconds for readiness. Prompt submission deliberately omits `--wait`: a successful `LaunchOutcome` means submission was accepted, not that the task completed. All subprocess output is captured rather than leaking into the terminal. CLI readiness timeouts are not an operating-system watchdog for an unresponsive CLI/socket. Discovery and creation are not atomic across external workspace changes; explicit IDs and response validation reduce but do not eliminate cross-process races.

The main issue list adds `· sent` to Open or Ready Beads whose OnTrack journal has the persisted `active` phase. `ManagedWorktree.handoff_accepted` is derived from that phase even if later Git or Herdr inspection reports a worktree needing attention. Earlier phases, including a prompt that may have been sent but was not confirmed, do not assert a successful handoff. The app reloads tracked worktrees after a successful launch, including when a worktree snapshot was already in flight; startup discovery and subsequent polling preserve the marker across refresh and restart. The marker is presentation only and never rewrites the Beads status.

Focused tests (`cargo test herdr::tests`) inject command execution, Bead retrieval, Git identity lookup and launch journaling. Each test thread initializes a disposable main Git checkout with `.git`, `src`, and `target` directories, so tests work when Cargo runs from either a main checkout or a linked worktree, including with `CARGO_TARGET_DIR` set elsewhere. They use deterministic fake text/JSON responses and captured argv, with no process-wide environment mutation, real Git worktree creation, or live Herdr creation/start/prompt. The tracked-launch regression persists a fake journal and checks that each phase is recorded before the next action; real Git journal behavior is tested separately in `src/worktrees.rs`. Coverage includes dynamic supported kinds/current integrations, malformed output, unavailable CLI, missing workspace without focus fallback, rejection of outside/nested linked launch directories using the observed main-source response and filesystem-only `.git` markers, full Bead capture before mutation, arbitrary single-argument prompting, ordering, no prompt wait, selection/collision checks, 32-character agent-name bounds and pre-creation overlong-name rejection, new target validation, stop-on-failure, and partial-success recovery. Codex regressions cover both overrides after `--`, separate pane-targeted prompting, unchanged OpenCode startup, TOML key escaping, normal/linked **created-worktree metadata** main-root resolution (not linked-source discovery support), nonstandard metadata rejection, and source/new identity failure without starting an agent. Installed read-only help/worktree-list/schema established the 0.9.1 contracts; no live Herdr creation or agent lifecycle is exercised. Fixtures cover the required response subset, not every server version or external race.

Worktree registry tests create exclusive temporary fixture directories and keep each registry lock inside its fixture. These test paths do not change the production rule that Herdr launch discovery starts from the main checkout.

Merge requests use the repository-wide registry lock and persist `merge_queue` order and provenance on each handoff. Manual requests and automatic reconciliation both enqueue; reconciliation dispatches at most one prompt when no unresolved `merge_request` remains. A queued request is revalidated at dispatch against current Git and agent state, with a fresh closed-Bead check for automatic requests. Failed pre-prompt dispatch is marked for explicit retry; possibly sent prompts retain the existing durable request marker and block later prompts. Verified integration and a settled prompted agent release the slot on the next reconciliation. External Git clients do not share this queue, so Git checks and ff-only integration remain required.

## Background work and cached state

`ontrack update` calls `update::check` in the foreground, prints the running
version and release status, and requires an explicit `y` or `yes` before calling
`update::install`. The update module calls `curl` with fixed HTTPS endpoints and
separate arguments, parses semantic versions, accepts
`ontrack-vVERSION-TARGET.tar.gz` with a matching `.sha256` sidecar, and extracts
only the `ontrack` entry from the archive. The selected target is one of glibc
Linux or macOS on x86_64 or aarch64. Release URL and asset URL checks restrict
downloads to the official tagged repository. Downloads and candidate
executables are staged with unique names in the executable directory, and
temporary files are removed on errors. SHA-256 is verified before extraction or
replacement, including GitHub's API digest when present; the final rename is on
the same filesystem. Failure before that rename leaves the original path
untouched. A running browser process continues using its old image; restart it
after success. `curl` and `tar` are runtime prerequisites for this command,
while normal browsing does not need them.

`App` uses standard-library channels with separate serial browser-Beads, Ready-mutation, and managed-work queues. Herdr discovery, harness discovery, manual/Ready launches, workspace focus, screenshot reads, and display-only agent observations are independent detached jobs. The UI loop receives available responses, draws, and polls input with a short timeout rather than waiting for subprocess completion. While any handoff is pending, worktree polling chooses the read-only list path instead of reconciliation. List and reconciliation display passes copy registry entries and release its lock before Git, Beads, or Herdr display checks; this lets another launch record its intent during a slow status lookup. Reconciliation still holds its lock while applying managed mutations, so launch journaling retains its bounded lock retry and recovery error.

Ready mutations (`SetReady` and `SetEpicReady`) use their own serial worker lane so bulk label updates never hold up list/detail requests. `mutation_busy` still excludes conflicting actions and pauses scheduling; `ready_pending` permits browser navigation, help, worktree browsing, and quit through that exclusion. Pending feedback is a header notice. Completion clears both flags, applies only confirmed IDs, and invalidates list/detail snapshots on success or error. Existing refresh sequencing schedules a trailing read when an older snapshot is already in flight. Queue failure clears both flags immediately. Workers remain detached on exit; quitting does not cancel a Beads update already running.

Automatic refresh uses the same public read-only list/show commands above. Schedule the next poll roughly one second after the previous refresh completes, including reloading the selected detail; do not accumulate timer-driven requests while refresh work is in flight. This is polling, not an event stream: slow CLI calls extend refresh latency. No watcher, extra dependency, configuration, or optional Beads events setup is required. Successful create/edit/delete operations still request an immediate refresh. In the worktrees view, `r` separately refreshes tracked worktrees.

Each serial queue retains request order without dropping mutations. Launches and managed reconciliation also perform authoritative read-only `bd show` calls outside the browser queue; not all `bd` calls are serialized. Responses for a show ID other than the current selection are ignored. Independent jobs may complete out of order and are correlated by their sequence/Bead identity.

The durable `w` view appends `Agent observation (display only): ...` to existing details without adding a separate task screen or changing static footers. Observations snapshot the registry, release its lock, and query each exact recorded pane; workspace/kind, recorded native session, and recorded terminal must still match. Missing, malformed, mismatched, or failed observations report unknown and retain rows/recovery identity. Polls do not overlap and the next poll is due about two seconds after completion. This presentation evidence never authorizes retry, readiness, merging, or cleanup; existing closed-Bead, original-session/lifecycle, terminal, and Git gates remain authoritative.

Selection uses the issue's cached list JSON immediately, marks details as loading, and requests full details. Refresh keeps the prior list while loading and preserves selection by ID when possible. Automatic updates also retain group collapse state, focus, scroll positions within current bounds, and active form input. Failed list/detail refreshes retain the last loaded data or available snapshot, display errors on the second header row, and retry automatically. The presentation layer renders narrative Markdown via `tui-markdown`. The selected issue's ID, status, and priority appear in the details pane border title and are omitted from its body; other fields except the top-level revision, including unknown nested values, remain represented. Rendered text drives both wrapping and scroll bounds.

Keep CLI calls off the input/render loop and preserve stale-response filtering when changing the worker. The worker is not joined on quit, so terminal cleanup does not wait for a slow CLI call; in-flight commands are not cancelled on quit.

## Release check

`src/update.rs` queries the official `ThomasJRyan/OnTrack` GitHub Releases
[`latest` endpoint](https://docs.github.com/en/rest/releases/releases#get-the-latest-release)
with `curl`. It validates an official stable semantic-version tag and a
compatible asset before offering installation. HTTP 404 yields a no-release
result; unavailable assets and malformed metadata return errors. The running
version comes from `CARGO_PKG_VERSION`; `semver` compares it with the release.
`src/update_cli.rs` keeps the prompt and output separate from the browser;
injected checker/installer tests cover current, no-release, error, decline, and
confirmed-install paths without network access or executable replacement.

## Terminal cleanup invariants

`main` attempts restoration after `app::run` returns, before returning its error. Terminal initialization attempts cleanup if entering the alternate screen, enabling mouse capture, or constructing the terminal fails. The installed panic hook attempts cleanup before invoking the previous hook. Restoration attempts cursor display, disables raw mode and mouse capture, and leaves the alternate screen; preserve these paths when changing startup or exit handling.

Run interactive checks in a real terminal or a PTY, not an ordinary non-interactive command runner. Exercise quit during startup, navigation, refresh/loading, errors, and tiny terminal sizes. Normal/error/panic cleanup does not imply cleanup after an uncatchable process kill.

## Tests and required checks

Tests are inline in the source modules. They cover CLI arguments and parsing, form input and mutation state, navigation and focus, mouse selection, cached/stale detail behavior, quit-key recognition, and rendering using ratatui's `TestBackend`.

Escape is navigation, not a quit key. The app handles dialogs first, then explicit quit keys (`q` and `Ctrl+C`), then the outstanding-worktrees view, and finally the main browser. An open choice list consumes Escape before its dialog does; the main browser consumes Escape without leaving the app. The mutation-busy guard blocks all keys until the operation completes.

For automatic refresh, check external list/detail changes without manual input, completion-based scheduling without a request backlog, cached-data retention and recovery after transient errors, and preservation of selection, collapsed groups, focus, scrolling, and form input. Keep immediate refresh after mutations and stale-detail response filtering covered too.

Run all required gates from the repository root:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```

GitHub Actions runs these gates on pull requests and pushes to `main`. See the [CI guide](ci.md) for workflow details, logs, and branch protection setup.

The same commands can run from a linked Git worktree. Keep the default test concurrency for `cargo test` to exercise fixture and registry lock isolation.

For focused iterations:

```sh
cargo test beads::tests
cargo test herdr::tests
cargo test app::tests
cargo test event::tests
cargo test ui::tests
```

Unit tests do not replace an interactive terminal cleanup check. When changing functionality, update the relevant Markdown guides under `docs/` and keep the README concise, preserving its controls and links to detailed documentation.

## Mutations

Create, edit, and delete are explicit, asynchronous CLI operations. The create and edit forms use the Beads type vocabulary and P0–P4 priorities; deletion requires a second, affirmative confirmation and never cascades. Each worker request is processed in channel order, without batching mutations together, and the UI disables repeat submissions while one is in flight. The CLI adapter is defined in `src/beads.rs`; the UI calls its create/update/delete functions and refreshes the normal list/detail views after success.

## Status-grouped list

List results are sorted deterministically by status (open first, closed last, other statuses alphabetically), then ID. On a new app launch, the first successful list containing normalized `closed` issues initializes that group as collapsed. The pending default survives empty or open-only lists, but is applied only once so subsequent refreshes preserve user toggles while the group remains present. `list_layout` in `src/app.rs` is the shared geometry model for navigation, viewport scrolling, mouse hit-testing, and rendering. Expanded panes place the first bead directly below the header; keep the app geometry and renderer in sync if row spacing changes. Keyboard navigation targets visible bead rows and collapsed group headers in rendered order; expanded headers are skipped. A collapsed-header selection has no bead/detail/delete identity and must retain a clear rendered selection state. Refresh preserves that selection only while the same group remains collapsed. Keep selected IDs aligned with detail/delete actions when changing this view.

The list row's first line derives issue type from the retained `Bead.data` (`issue_type`, with `type` as a fallback); it does not replace the internal bead ID. Type, priority, and status spans each have a semantic colour helper in `src/ui.rs`; the display-width-aware metadata columns are shared across the list. Preserve white title text independently of selection emphasis and keep missing/custom types safe to display.
## Tracked worktree lifecycle

OnTrack records only worktrees created by its own explicit Herdr actions; it
does not adopt pre-existing Git worktrees. Outstanding work is a separate view
from Beads: a closed or missing Bead does not remove the managed-worktree record.
The app filters verified `CheckoutRemoved` rows from the visible worktree list
on refresh and immediately after a successful removal action. The registry still
retains those records, and `NeedsAttention` rows remain visible when inspection
finds a problem after removal. Selection clamps to a remaining row or the empty
state. Accepted handoff IDs are retained separately from visible rows so an Open
Bead's `· sent` marker still reflects its confirmed handoff.
The existing periodic reconciliation checks the recorded path of each active
handoff before considering automatic actions. A confirmed missing path sets
`checkout_removed` and `checkout_removed_externally` in the registry even when
cleanup preferences are off; it does not call Herdr, remove a Git branch, or
claim that branch was integrated. An invalid path or filesystem inspection
error is retained as `NeedsAttention`. A pending restart remains visible for
recovery; a pending cleanup operation still uses its existing read-only
confirmation before this check. External-removal records
remain removed across restarts while the path is absent; if it reappears, they
surface as `NeedsAttention` for identity review.
The launch journal stores the authoritative Bead title and description before
Herdr creates a worktree. The right pane prefers current text from the already
refreshed Beads list and falls back to this registry snapshot after Bead removal
or a list failure; it makes no extra `bd` request. Registry entries from older
versions may omit these optional fields; the view then shows a fallback. The
pane wraps multiline descriptions and keeps its own bounded scroll position,
reset when the selected worktree changes.

For a parent with open or unintegrated children, the first confirmed `h` action creates and verifies a managed parent checkout, then persists journal phase `reserved` without starting an agent. The parent stays in the tracked-worktrees view. A child launch requires one open managed parent checkout, records its parent Bead ID, target branch, and canonical target checkout path, and passes `--base PARENT_BRANCH` to Herdr worktree creation. If the child closes and integrates, a later `h` on the parent reuses the exact reserved workspace and root pane to start its agent; no second worktree is created. The registry rejects duplicate active handoffs for the same Bead. Reconciliation selects `auto_merge_children` for entries with `parent_bead_id` and `auto_merge` for top-level entries; the former defaults true, the latter false, including when a field is absent from an older settings file. Both use the same durable `queue_merge` path and guarded dispatch. The merge path verifies the recorded source and target identities, cleanliness, branch names, and Git tips before sending a resolution prompt or fast-forwarding the target checkout. Parent merge requests and finalization recheck live open descendants and pending child integrations. Child entries are sorted below their parent for display and indented; selection remains keyed by the worktree ID. A parent checkout cannot be closed or removed while tracked child checkouts still exist. For historical inspection of an already removed child checkout, a missing parent checkout path falls back to read-only ancestry between the retained child and recorded parent branch tips. A present but changed parent path still fails identity checks; live merge and cleanup operations never use this fallback.
The verified repository target branch is shown to the user. A merge request is
pending work, not a completed merge; Git ancestry/cleanliness checks decide
integration. Conflicts and uncertain outcomes remain inspectable for manual
recovery. Cleanup is permitted only after verified integration. Checkout removal
is one confirmed operation that removes the checkout and closes its Herdr
workspace; when both cleanup switches are enabled, removal runs instead of a
separate close. It never force-removes a checkout or deletes a branch. Bead
closure is an eligibility signal only, not evidence
that an agent is idle or its changes safe to merge. Automatic merge may therefore
defer; manual merge remains the user-controlled path.

`inspect` in `src/worktrees.rs` remains the Git-based safety classification.
Read-only inspection checks checkout identity, branch, Git tips, and unfinished
operations separately from ordinary tracked or untracked changes. A dirty source
or target yields `DirtyCheckout` (`UNCOMMITTED CHANGES`) rather than
`NeedsAttention`; it cannot be displayed as ready or integrated, so it cannot
enable cleanup or automatic merge. A pending merge request keeps
`MergeRequested` and includes a dirty-checkout detail. Unresolved source merge
conflicts still take `NeedsResolution`, and identity or history failures still
take `NeedsAttention`. Mutating paths use strict `verify`, which requires both
checkouts to be clean before any merge, close, or removal operation.
For an active branch with task commits outside its target, `inspect_for_display`
shows `InProgress` until a live `bd show` identifies the same Bead as closed and
the recorded Herdr agent has settled after prompt activity. A launch with a
native session uses the strict settled-session check. For older launch responses
without a session, the display checks the captured pre-prompt `agent_info`,
later lifecycle sequence, pane/workspace/kind, checkout paths, and terminal ID
when present. Failed or unavailable completion observations remain `InProgress`;
an untouched launch-base branch is also `InProgress`. The internal `Unmerged`
status renders as `READY TO MERGE` in both worktree panes. New handoff prompts
ask agents to close the Bead only after checked and committed work; unfinished
work must keep its Bead open or in progress.
The display check makes read-only Beads and Herdr calls during worktree refresh;
it does not change merge, finalization, or cleanup gates. A legacy launch without
a recorded session can show `READY TO MERGE` from weaker display evidence.
Automatic reconciliation separately requires a later native session on the same
recorded terminal and persists it before acting. Without that evidence, automatic
merge remains deferred. Explicit manual merge confirmation remains available
for `InProgress` rows and is still subject to the existing Git and agent checks.
The launch journal records `state_change_seq` and `terminal_id` from the
pre-prompt `agent get` response even when `agent_session` has not appeared yet.
Reconciliation can recover those observations from the retained `ready`
response in older entries. A later native session is bound and persisted only
after the same pane, workspace, agent kind, terminal, checkout, and post-prompt
activity pass the settled-agent check. Subsequent merge and cleanup passes
require that exact session. Missing or changed evidence leaves the worktree
visible; no automatic action is dispatched.
`inspect_for_display` only changes a `NeedsResolution` merge-request row to
`MergeRequested` when a read-only Herdr `agent get` response reports `working`
for the recorded pane, workspace, and agent kind, with the recorded session
matching when available. The original Git concern stays in the detail text.
Idle, done, blocked, mismatched, malformed, and unavailable observations retain
`NeedsResolution`. Both listing and reconciliation use this display mapping;
finalization, action gates, and cleanup continue to use the Git classification.

Manual merge provenance is persisted separately from automatic requests: optional
agent-session metadata is not required for an explicitly confirmed manual merge,
but current Git identity, cleanliness and ancestry checks still apply. Native
idle/sequence evidence is not proof of a particular prompt turn completing.
Cleanup revalidates the current Herdr workspace/path/branch mapping before any
mutation. Untouched launch-base branches remain unmerged. Close-first/remove-later
is explicitly unsupported; recovery guidance avoids guessing a closed workspace
ID or invoking force removal.
## Screenshot descriptions

`src/screenshots.rs` reads PNG and JPEG clipboard data using `wl-paste` or `xclip`, limits images to 10 MiB, and stores them in the shared workspace reported by `bd where --json`. The create/edit form inserts an absolute Markdown image path only after the bytes are saved. The Herdr launch prompt extracts these references from the live description and lists paths for the agent to inspect. Image storage is local and separate from Beads records and Git worktrees; do not assume remote sync or embed binary data in Herdr text prompts.
