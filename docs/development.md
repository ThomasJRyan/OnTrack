# Development guide

ontrack is a single Rust 2024 binary requiring Rust 1.88 or newer. Runtime workspace integration uses the public Beads CLI; no direct database integration is needed. Browse operations are read-only, while create, edit, delete, and Herdr handoff are explicit user actions. Consult the [official Beads documentation](https://beads.gascity.com) for CLI and workspace behavior and the [user guide](user-guide.md) for running the browser.

## Modules

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Defines the terminal type, initializes the terminal, runs the app, and restores the terminal before propagating application errors. |
| `src/settings.rs` | Loads and atomically saves theme and worktree-cleanup preferences under the user's XDG config directory. |
| `src/theme.rs` | Defines the bundled theme catalog, stable IDs, display names, and semantic color palettes. |
| `src/beads.rs` | Builds CLI commands, runs `bd`, validates JSON responses, and preserves complete returned issue records. |
| `src/herdr.rs` | Discovers usable agent integrations, maps the current checkout to a Herdr workspace, focuses a selected managed workspace, and explicitly creates a worktree and starts/prompts an agent. |
| `src/worktrees.rs` | Journals OnTrack-created handoffs separately from Beads and verifies Git/Herdr evidence before merge finalization or cleanup. |
| `src/app.rs` | Holds selection, focus, scrolling, modal form and handoff toast state, cached data, loading/error state, and pane hit-test rectangles; handles input and background worker requests/responses. |
| `src/event.rs` | Polls crossterm events with a timeout and recognizes quit keys. |
| `src/hotkeys.rs` | Maps the active pane or dialog to its keyboard command list and recognizes modified help-key reports. |
| `src/ui.rs` | Renders splash, issue list, wrapped Markdown details, status, and top-right handoff toasts; maintains list visibility and scroll bounds. |
| `src/detail.rs` | Presents complete issue records with Markdown rendering for narrative fields and readable structured fallbacks for metadata. |
| `src/terminal.rs` | Manages raw mode, alternate screen, mouse capture, and cleanup, including initialization failures and panic handling. |

`src/ui.rs` renders the supplied centered ONTRACK wordmark and a terminal-drawn anime girl during the 1.2-second startup splash. `girl_frame` chooses the blink and wave pose from elapsed time supplied by `app::run`'s existing redraw loop, so the animation needs no separate timer or state. If the full wordmark and character do not fit, the renderer uses the compact ONTRACK title with the character when possible, then a text-only fallback. Width checks use terminal cell widths; preserve the wordmark verbatim, keep the splash centered and theme-aware, and ensure it yields to the normal browser view after startup.

In the managed-worktree view, Enter queues `herdr workspace focus WORKSPACE_ID` on the existing background worker. The ID comes from the selected OnTrack handoff, never global focus. The adapter passes arguments separately, rejects unsafe IDs, and requires Herdr's `workspace_focused` result to repeat the selected ID. Closed workspaces and removed checkouts are rejected before dispatch. Focus success and errors are shown in the view without changing Git, Beads, or the handoff registry.

`theme::worktree_status_color` maps every `WorktreeStatus` to a semantic palette role. `ui::worktree_status_style` applies that colour to the label's foreground characters and leaves the pane background intact; the surrounding selected-row highlight remains separate. Keep list and detail labels in sync when adding a worktree state. Theme tests enforce that pending and attention colours differ from integrated in every bundled theme, and UI buffer tests check all list labels and selected details across bundled themes.

Theme is explicit app state (`App::theme`) threaded into rendering; do not introduce global or thread-local palette state. Keep Dark as the existing default look. New colors need readable Light equivalents for surfaces, text, semantic metadata, startup splash, details, and dialogs. Settings changes immediately attempt persistence; update `App::settings` and the active theme only after `settings::save_to` succeeds. Failed writes retain the candidate for retry without changing the active theme. Tests should use isolated settings paths rather than changing process-wide environment variables.

Choice controls use the shared `cycle_choice` helper for Left / Right navigation. Both directions wrap at the ends; a closed focused dropdown changes immediately, while an open dropdown previews changes until Enter accepts or Esc restores its opening value. Keep this consistent for future dropdown controls. Up / Down behavior remains control-specific.

The create/edit Description field uses `description_layout` to insert visual line breaks before whole words and map its byte cursor to display cells; words wider than the field still wrap by character. `wrapped_description` renders that layout, and `move_description_vertical` uses its byte-boundary positions so Up / Down follows the same visual rows at the form's fixed 64-cell text width. Vertical movement retains a preferred display column across shorter rows, clamps at the top and bottom, and leaves the stored text untouched. Other cursor movement or editing clears that preference. Explicit newlines remain in the stored text. While Description has focus, Shift+Enter inserts a newline and plain Enter focuses the form's action without submitting. Terminal setup enables Crossterm's keyboard enhancement protocol so supported terminals can distinguish modified Enter; restoration pops that mode on normal exit, initialization failure, and panic. Terminals without modified-key reporting cannot distinguish Shift+Enter from Enter.

The hotkey reference is a separate `App::help` overlay, not a modal replacement. `App::key` handles its toggle and scroll keys before dispatching to the underlying pane or dialog; the mutation lock still takes precedence. `hotkeys::Context` captures the current pane or dialog when opened and owns its key/action list. Add new keyboard commands to the matching context when changing input handling. `ui::render_help` wraps the right column, clamps scrolling to the rendered lines, and renders over either browser view. Enhanced keyboard reports distinguish `Ctrl+?` or `Ctrl+Shift+/`; legacy terminals may report it as plain Backspace, which must remain text deletion/navigation rather than opening help.

The `s` settings dialog saves each theme/preference change immediately, writing the complete Settings value. Up / Down navigates rows, Left / Right changes theme, and Enter / Space toggles cleanup preferences. Failed saves must not update committed settings. See the [settings guide](settings.md) for details.
Settings rows in `src/ui.rs` share a two-thirds title column and one-third
value column. `settings_row` aligns titles to the left and centers controls
by terminal cell width; keep dropdown entries on the same value boundary and
retain the existing compact-terminal fallback when changing this layout.

## Beads CLI contract

The subprocess inherits the application's working directory. Commands are passed as argument vectors, not shell strings:

```sh
bd --readonly --no-color list --all --limit 0 --json
bd --readonly --no-color show --id=ISSUE_ID --json --long --include-comments --include-dependents
bd --no-color create --title=TITLE --description=DESCRIPTION --type=task --priority=2 --json
bd --no-color create --title=TITLE --description=DESCRIPTION --type=task --priority=2 --json --parent=PARENT_ID
bd --no-color update --title=TITLE --description=DESCRIPTION --type=task --priority=2 --json -- ISSUE_ID
bd --no-color delete --force --json -- ISSUE_ID
```

Create returns a JSON object containing the created ID; update and delete are successful on a successful command exit. Mutations are not passed `--readonly`; update sends only the four form fields, and delete uses `--force` for the selected issue only and never requests cascading deletion. The worker processes queued requests sequentially, preserving mutation order. Edit opening fetches the current `bd show` record on that worker, validates the editable type and priority, and ignores stale load responses using a request sequence. A failed update leaves the draft open; success triggers an immediate refresh and reselects the same ID.

`Shift+N` opens the create form with an immutable selected parent ID. Child creation passes `--parent=PARENT_ID` as its own argument, so Beads records the hierarchy and list/show JSON exposes the child's `parent` field. OnTrack reads the uncapped live list to find open descendants before a parent starts or merges. Beads rejects a `blocks` dependency from a parent to its descendant because blocked state would cascade back to that descendant; its `bd ready` output still includes the parent. The child-first gate is therefore enforced by OnTrack, not by a reverse Beads dependency.

`--all` includes closed issues and `--limit 0` removes the default list cap. `--id=ISSUE_ID` is a single argument. The list and show responses must be JSON arrays. List parsing requires nonempty string `id`, `title`, and `status`; a missing or null priority defaults to 2, and supplied priority must be an unsigned integer. Each `Bead` retains the entire JSON object as well as its summary fields.

Detail parsing selects the requested ID, rejects duplicate matching records, and reports a missing ID. It retains the returned JSON intact, including comments, relationships, metadata, and fields unknown to ontrack. The detail renderer omits only the top-level `revision` field from display, for both cached list records and full show records; nested revision fields and other metadata remain visible. Process-start errors, unsuccessful exit statuses with stderr, and invalid JSON/records are surfaced as errors rather than panics.

## Epic planning workspace

`src/epic.rs` handles opt-in Epic planning separately from the tracked worktree handoff. The create worker persists the Epic first, then a separate thread validates the live Epic and originating Herdr workspace, discovers a current agent integration, and calls `herdr workspace create --cwd CURRENT_CHECKOUT --label ... --no-focus`. Herdr workspaces have no parent parameter: the new labeled workspace shares the checkout and records its source workspace in the label. No `herdr worktree create` call is involved. The agent is started and prompted by exact pane ID; the proposal comes from the live Bead Description, and the prompt requires `bd create --parent=EPIC_ID` children.

The planning thread waits for the recorded agent to finish, reads the live Beads list, and closes the new workspace only after finding a direct child. Failures report the Epic and workspace for inspection without an automatic retry. The thread runs within OnTrack's process, so closing OnTrack before generation finishes stops its automatic cleanup. Normal create and edit requests retain their previous path. The UI exposes the toggle only on Epic creation and clears it when the type changes away from Epic.

## Herdr launch adapter

`src/herdr.rs` exposes `discover() -> io::Result<LaunchContext>` and `launch(context: &LaunchContext, agent: &Agent, bead: &Bead) -> io::Result<LaunchOutcome>` for background-worker use. `LaunchContext` contains a `Workspace` and available `Agent` records (each with string `id` and `label`); `LaunchOutcome.message` carries the handoff result. These public records derive `Debug`, `Clone`, `PartialEq`, and `Eq`. The app queues the full selected Bead and typed context/agent; the backend still captures authoritative `bd show` JSON before mutation and revalidates workspace/agent availability.

Restart is a separate confirmed worker request for a selected tracked handoff. The backend requires an active journal entry, verifies the main checkout identity and clean current target branch, checks the recorded checkout path/branch and Herdr ownership, and fetches the live Bead before mutation. It also checks that the recorded agent integration and source workspace are available. It saves a `restart_pending` marker before invoking `herdr worktree remove --workspace ID --force`, captures the response, and verifies both Herdr and Git inventories plus path absence. Only then does it mark the old handoff restarted and use the ordinary launch path with a new unique name and the live Bead. The old Git branch is retained. An uncertain removal is never retried automatically. A failed relaunch records an attention state; any partial new launch has its own normal launch journal for recovery. Normal cleanup still requires verified integration and never uses force.

Pressing `h` records the selected Bead and queues discovery once. While that handoff is pending, another `h` is ignored and normal browsing remains available. The discovery response selects Codex if present, otherwise the first agent in the sorted discovery list, and immediately queues launch. Missing selection, discovery errors, and empty agent lists stop before launch. Stale or duplicate discovery responses cannot dispatch another launch. The top-right toast reports progress and a timed result; the outcome or error is also stored for the footer, where it may be truncated by terminal width. The existing tracked-worktree refresh runs after confirmed success. Rendering tests cover normal and compact toasts, and app tests cover automatic dispatch, selection capture, duplicate prevention, errors, and stale responses.

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

The main issue list adds `· sent` to Open Beads whose OnTrack journal has the persisted `active` phase. `ManagedWorktree.handoff_accepted` is derived from that phase even if later Git or Herdr inspection reports a worktree needing attention. Earlier phases, including a prompt that may have been sent but was not confirmed, do not assert a successful handoff. The app reloads tracked worktrees after a successful launch, including when a worktree snapshot was already in flight; startup discovery and subsequent polling preserve the marker across refresh and restart. The marker is presentation only and never rewrites the Beads status.

Focused tests (`cargo test herdr::tests`) inject command execution, Bead retrieval, Git identity lookup and launch journaling. Each test thread initializes a disposable main Git checkout with `.git`, `src`, and `target` directories, so tests work when Cargo runs from either a main checkout or a linked worktree, including with `CARGO_TARGET_DIR` set elsewhere. They use deterministic fake text/JSON responses and captured argv, with no process-wide environment mutation, real Git worktree creation, or live Herdr creation/start/prompt. The tracked-launch regression persists a fake journal and checks that each phase is recorded before the next action; real Git journal behavior is tested separately in `src/worktrees.rs`. Coverage includes dynamic supported kinds/current integrations, malformed output, unavailable CLI, missing workspace without focus fallback, rejection of outside/nested linked launch directories using the observed main-source response and filesystem-only `.git` markers, full Bead capture before mutation, arbitrary single-argument prompting, ordering, no prompt wait, selection/collision checks, 32-character agent-name bounds and pre-creation overlong-name rejection, new target validation, stop-on-failure, and partial-success recovery. Codex regressions cover both overrides after `--`, separate pane-targeted prompting, unchanged OpenCode startup, TOML key escaping, normal/linked **created-worktree metadata** main-root resolution (not linked-source discovery support), nonstandard metadata rejection, and source/new identity failure without starting an agent. Installed read-only help/worktree-list/schema established the 0.9.1 contracts; no live Herdr creation or agent lifecycle is exercised. Fixtures cover the required response subset, not every server version or external race.

Worktree registry tests create exclusive temporary fixture directories and keep each registry lock inside its fixture. These test paths do not change the production rule that Herdr launch discovery starts from the main checkout.

## Background work and cached state

`App` uses standard-library channels and one background thread for blocking CLI calls. The UI loop receives available responses, draws, and polls input with a short timeout rather than waiting for subprocess completion.

Automatic refresh uses the same public read-only list/show commands above. Schedule the next poll roughly one second after the previous refresh completes, including reloading the selected detail; do not accumulate timer-driven requests while refresh work is in flight. This is polling, not an event stream: slow CLI calls extend refresh latency. No watcher, extra dependency, configuration, or optional Beads events setup is required. Manual `r` and successful create/delete operations still request an immediate refresh.

The background worker executes each request in channel order. It does not coalesce or reorder requests, so mutation commands cannot be lost. Responses for a show ID other than the current selection are ignored.

Selection uses the issue's cached list JSON immediately, marks details as loading, and requests full details. Refresh keeps the prior list while loading and preserves selection by ID when possible. Automatic updates also retain group collapse state, focus, scroll positions within current bounds, and active form input. Failed list/detail refreshes retain the last loaded data or available snapshot, display errors in the footer, and retry automatically. The presentation layer renders narrative Markdown via `tui-markdown`; all remaining fields except the top-level revision, including unknown nested values, remain represented. Rendered text drives both wrapping and scroll bounds.

Keep CLI calls off the input/render loop and preserve stale-response filtering when changing the worker. The worker is not joined on quit, so terminal cleanup does not wait for a slow CLI call; in-flight commands are not cancelled on quit.

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

For a parent with open or unintegrated children, the first confirmed `h` action creates and verifies a managed parent checkout, then persists journal phase `reserved` without starting an agent. The parent stays in the tracked-worktrees view. A child launch requires one open managed parent checkout, records its parent Bead ID, target branch, and canonical target checkout path, and passes `--base PARENT_BRANCH` to Herdr worktree creation. If the child closes and integrates, a later `h` on the parent reuses the exact reserved workspace and root pane to start its agent; no second worktree is created. The registry rejects duplicate active handoffs for the same Bead. The merge path verifies the recorded source and target identities, cleanliness, branch names, and Git tips before sending a resolution prompt or fast-forwarding the target checkout. Parent merge requests and finalization recheck live open descendants and pending child integrations. Child entries are sorted below their parent for display and indented; selection remains keyed by the worktree ID. A parent checkout cannot be closed or removed while tracked child checkouts still exist.
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
For an active branch with task commits outside its target, `inspect_for_display`
shows `InProgress` until a live `bd show` identifies the same Bead as closed and
the recorded Herdr agent passes the existing settled-session check, including
post-handoff lifecycle activity. Failed or unavailable completion observations
remain `InProgress`; an untouched launch-base branch is also `InProgress`.
`Unmerged` therefore means outstanding task commits with completion evidence.
The display check makes read-only Beads and Herdr calls during worktree refresh;
it does not change merge, finalization, or cleanup gates. Explicit manual merge
confirmation remains available for `InProgress` rows and is still subject to
the existing Git and agent checks.
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
