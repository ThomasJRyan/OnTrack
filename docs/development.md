# Development guide

ontrack is a single Rust 2024 binary requiring Rust 1.88 or newer. Runtime workspace integration uses the public Beads CLI; no direct database integration is needed. Browse operations are read-only, while create and delete are explicit user actions. Consult the [official Beads documentation](https://beads.gascity.com) for CLI and workspace behavior and the [user guide](user-guide.md) for running the browser.

## Modules

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Defines the terminal type, initializes the terminal, runs the app, and restores the terminal before propagating application errors. |
| `src/beads.rs` | Builds CLI commands, runs `bd`, validates JSON responses, and preserves complete returned issue records. |
| `src/app.rs` | Holds selection, focus, scrolling, modal form state, cached data, loading/error state, and pane hit-test rectangles; handles input and background worker requests/responses. |
| `src/event.rs` | Polls crossterm events with a timeout and recognizes quit keys. |
| `src/ui.rs` | Renders splash, issue list, wrapped Markdown details, and status; maintains list visibility and scroll bounds. |
| `src/detail.rs` | Presents complete issue records with Markdown rendering for narrative fields and readable structured fallbacks for metadata. |
| `src/terminal.rs` | Manages raw mode, alternate screen, mouse capture, and cleanup, including initialization failures and panic handling. |

`src/ui.rs` embeds `assets/splash.png` with `include_bytes!`; keep that asset available at build time.

## Beads CLI contract

The subprocess inherits the application's working directory. Commands are passed as argument vectors, not shell strings:

```sh
bd --readonly --no-color list --all --limit 0 --json
bd --readonly --no-color show --id=ISSUE_ID --json --long --include-comments --include-dependents
bd --no-color create --title=TITLE --description=DESCRIPTION --type=task --priority=2 --json
bd --no-color delete --force --json -- ISSUE_ID
```

Create returns a JSON object containing the created ID; delete is successful on a successful command exit. Create/delete are not passed `--readonly`; delete uses `--force` for the selected issue only and never requests cascading deletion. The worker processes queued requests sequentially, preserving mutation order.

`--all` includes closed issues and `--limit 0` removes the default list cap. `--id=ISSUE_ID` is a single argument. The list and show responses must be JSON arrays. List parsing requires nonempty string `id`, `title`, and `status`; a missing or null priority defaults to 2, and supplied priority must be an unsigned integer. Each `Bead` retains the entire JSON object as well as its summary fields.

Detail parsing selects the requested ID, rejects duplicate matching records, and reports a missing ID. It retains the returned JSON intact, including comments, relationships, metadata, and fields unknown to ontrack. Process-start errors, unsuccessful exit statuses with stderr, and invalid JSON/records are surfaced as errors rather than panics.

## Background work and cached state

`App` uses standard-library channels and one background thread for blocking CLI calls. The UI loop receives available responses, draws, and polls input with a short timeout rather than waiting for subprocess completion.

The background worker executes each request in channel order. It does not coalesce or reorder requests, so mutation commands cannot be lost. Responses for a show ID other than the current selection are ignored.

Selection uses the issue's cached list JSON immediately, marks details as loading, and requests full details. Refresh keeps the prior list while loading and preserves selection by ID when possible. Errors retain available cached data and are displayed in the footer. The presentation layer renders narrative Markdown via `tui-markdown`; all remaining fields, including unknown nested values, remain represented. Rendered text drives both wrapping and scroll bounds.

Keep CLI calls off the input/render loop and preserve stale-response filtering when changing the worker. The worker is not joined on quit, so terminal cleanup does not wait for a slow CLI call; in-flight commands are not cancelled on quit.

## Terminal cleanup invariants

`main` attempts restoration after `app::run` returns, before returning its error. Terminal initialization attempts cleanup if entering the alternate screen, enabling mouse capture, or constructing the terminal fails. The installed panic hook attempts cleanup before invoking the previous hook. Restoration attempts cursor display, disables raw mode and mouse capture, and leaves the alternate screen; preserve these paths when changing startup or exit handling.

Run interactive checks in a real terminal or a PTY, not an ordinary non-interactive command runner. Exercise quit during startup, navigation, refresh/loading, errors, and tiny terminal sizes. Normal/error/panic cleanup does not imply cleanup after an uncatchable process kill.

## Tests and required checks

Tests are inline in the source modules. They cover CLI arguments and parsing, form input and mutation state, navigation and focus, mouse selection, cached/stale detail behavior, quit-key recognition, and rendering using ratatui's `TestBackend`.

Run all required gates from the repository root:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```

For focused iterations:

```sh
cargo test beads::tests
cargo test app::tests
cargo test event::tests
cargo test ui::tests
```

Unit tests do not replace an interactive terminal cleanup check. When changing functionality, update the relevant Markdown guides under `docs/` and keep the README concise, preserving its controls and links to detailed documentation.
## Mutations

Create and delete are explicit, asynchronous CLI operations. The create form uses the Beads type vocabulary and P0–P4 priorities; deletion requires a second, affirmative confirmation and never cascades. Each worker request is processed in channel order, without batching mutations together, and the UI disables repeat submissions while one is in flight. The CLI adapter is defined in `src/beads.rs`; the UI only calls its create/delete functions and refreshes the normal list/detail views after success.
