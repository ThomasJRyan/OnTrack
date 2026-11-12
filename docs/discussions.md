# Ticket discussions

Discuss a Bead without leaving the board, preserve decisions in its comments, and send follow-up input to its original managed agent. This is a durable ticket thread, not live chat or an embedded terminal log.

## Write a comment or send input

Select a ticket and press `t` on the board or in its detail drawer. Existing comments are shown in the detail drawer.

| Control | Action |
| --- | --- |
| `Shift+Enter` in text | Insert a newline. Requires a terminal that reports modified Enter keys. |
| `Enter` in text | Focus **Save comment**, without submitting. |
| `Tab` / `Shift+Tab` | Cycle text and enabled actions; unavailable Send is skipped. |
| `Left` / `Right` on an action | Switch between enabled actions. |
| `Enter` on an action | Execute the focused action, if available. |
| `Esc` | Discard the draft, except while submission is running. |

**Save comment** records ordinary Beads history. It always omits the composer's captured question reply reference, so saving a note while a question is pending neither answers nor acknowledges it. It does not prompt an agent. You can save comments before launch; there is no prelaunch discussion agent. The normal handoff supplies issue context, including comments, when an agent is later launched.

Before a managed record exists, Save writes plain text directly to Beads without creating a discussion journal. When exactly one non-restarted managed record matches the ticket, Save also journals a correlated message and writes an OnTrack metadata marker with the comment. A confirmed Save alone creates no agent obligation or readiness change; an uncertain managed comment write still holds integration until its persistence is resolved.

**Send to agent** requires a live Bead in `open` or `in_progress` status and exactly one current accepted OnTrack-managed handoff. It targets the original recorded session and worktree, not whichever pane currently has focus. A reserved parent checkout without an accepted agent handoff is not a delivery target. After task commits are integrated, an exact answer to an existing question remains allowed, but new unrelated messages and questions are refused. Merge, continuation, restart, or cleanup in flight also prevents new discussion submission. An enabled Send button is only a cached indication: authoritative checks can still reject submission. Sending does not launch a replacement agent, switch sessions, or authorize Git integration.

When the agent is busy, blocked, in error, or its activity is unknown, input remains queued. Automatic prompt delivery occurs only for a verified original agent reporting `idle` or `done`, and may batch multiple messages. Keep OnTrack running for delivery reconciliation; an agent can also read queued input with `inbox` before a prompt is sent. There is no automatic live interrupt. For approvals, authentication, or other native prompts that cannot accept ordinary agent input, press `w`, select the handoff, and press `Enter` to focus its recorded Herdr workspace. Handle the native prompt there; a discussion message does not grant approval or bypass authentication.

Comments are **durable project data**, not private or ephemeral chat. Avoid passwords, tokens, and other secrets. Saving or sending text is not a way to hide it from other readers of the Bead.

If the initial journal write has uncertain durability, or an unjournaled prelaunch comment write fails, the composer retains your draft and blocks **both** submission actions. Editing and `Esc` still work. Inspect Beads comments and any journal evidence before recreating the draft or sending it another way; retaining text is not permission to duplicate a possibly completed write. Ordinary errors before a write preserve the draft and permit an explicit retry. Once durable intent exists, later delivery errors instead return a receipt, close the composer, and leave pending/uncertain status for recovery. Read the receipt rather than assuming that closing the composer means delivery completed.

## Understand discussion state

| State | Meaning / next step |
| --- | --- |
| **Needs your input** | The agent has an unresolved question. Read it and send your answer. Saving a comment alone does not resolve it. |
| **Pending delivery** | Input is queued, comment persistence/application is pending, or readiness withdrawal has not finished. The original session/worktree must still validate before prompting. |
| **Awaiting agent reply** | Delivery was accepted, but the agent has not explicitly acknowledged handling that input. |
| **Delivery uncertain** | A Beads comment write or agent prompt may have succeeded without confirmation. Inspect the recorded Herdr workspace and thread before manual recovery. |

Transport acceptance is **not** proof that the agent read or acted on a message. OnTrack does not automatically resend uncertain deliveries. External CLI calls and journal writes do not form one transaction, so inspect retained evidence when an outcome is uncertain.

Each handoff can have one unresolved agent question at a time. You may still send independent follow-up messages. The composer captures the pending question ID when it opens: if that question changes before submission, the backend rejects the stale answer. Sending an answer does not itself clear the question; the agent must explicitly acknowledge that exact answer. An unrelated acknowledgement cannot clear it or a newer unseen question.

Sending input or asking a question withdraws Ready To Merge. Pending or unresolved discussion prevents merge, closure, and cleanup, including force removal and restart; it also blocks the separate continue action. A pending child discussion holds parent integration even if the child's Git commits are already integrated. An acknowledgement withdraws stale readiness again rather than restoring it. The agent must review the input, update the work, run the relevant checks, and explicitly reassert Ready To Merge when appropriate. Fresh Git integration proof remains necessary before a tracked Bead closes; discussion never replaces those checks.

An early agent acknowledgement cannot release integration while the corresponding prompt submission is still in flight. If the sender crashes before recording its return, that outstanding attempt remains a conservative hold requiring human inspection. It has no automatic replay, lease expiry, or dedicated CLI command to clear it.

With no discussion messages, the existing one-shot handoff and verified-integration workflow is unchanged. Advisory `ontrack status` reports remain display-only; a blocked activity report is not an agent question or a reply acknowledgement.

## Agent workflow

Run these commands from the original managed agent checkout. They dispatch before the Herdr startup marker check, Beads startup discovery, and terminal setup. All three require an active, uniquely matching managed checkout with valid Git identity. `ask` and `respond` additionally verify the live Bead, open workspace, original native session, pane, agent kind, recorded terminal when available, and both agent checkout paths. `inbox` reads journal state without querying the live native session. Use the installed executable's absolute path if `ontrack` is not on `PATH`; if it is unavailable, stop and request access rather than bypassing the protocol.

If the native session was unavailable at launch, later binding must match the recorded launch terminal, later lifecycle activity, and verified checkout paths. A missing saved session is not permission to adopt an arbitrary current agent; insufficient or changed evidence leaves the operation blocked.

```sh
ontrack inbox --json
ontrack ask 'Which compatibility target should this change preserve?'
ontrack respond --reply-to MESSAGE_ID 'Implemented the requested compatibility change and checked it.'
ontrack respond --reply-to MESSAGE_ID_1 --reply-to MESSAGE_ID_2 'Handled both inputs and checked the result.'
```

`inbox --json` returns an object with `handoff` (the managed record ID), `question` (the unresolved question ID or `null`), and `messages` (confirmed user-message records not yet acknowledged). Records include `id`, `kind`, `text`, `body`, `reply_to`, `session`, `comment`, `applied`, and `delivery`. Queued and uncertain deliveries can appear alongside accepted ones: the agent can handle these directly from the inbox. Saved notes, agent responses, and full question text are not inbox messages; read the Beads thread for that history. Plain `ontrack inbox` prints each input ID, its reply references, and text. Neither form acknowledges input.

1. Read `inbox --json` before work and again before marking Ready To Merge; keep the exact message IDs of the input you handle. Do not infer acknowledgement from successful prompt transport or an idle observation.
2. Use `ask TEXT` for a consequential question, then yield rather than claiming readiness while waiting. Only one agent question may remain unresolved per handoff.
3. Handle the user's input before using `respond --reply-to MESSAGE_ID TEXT`. Repeat `--reply-to` once per distinct user-message ID handled. A response acknowledges only those IDs, not all current or future messages; question IDs, unknown IDs, and duplicate IDs in one command are rejected. An already acknowledged input may be referenced again, but that cannot clear a replacement question.
4. Check for additional input before declaring completion. Recheck the work and explicitly set the usual Ready To Merge signal only when discussion is settled and checks/commits are complete. Never close the Bead before verified integration.

To acknowledge an answer and atomically replace its question without opening an integration window:

```sh
ontrack ask --reply-to ANSWER_MESSAGE_ID 'Understood. Should the compatibility fallback also cover offline use?'
```

Use the **user answer's message ID**, not the earlier question ID. When a question is unresolved, replacement must reference an answer explicitly linked to that question. `ask` also accepts repeated `--reply-to` flags for additional inputs handled. The acknowledgement and replacement take effect together only after the correlated Beads comment is confirmed; then yield again.

Quote `TEXT` as one UTF-8 argument. Both write commands accept `--` after the reply flags to separate text beginning with `--`. Successful command exit may still return an uncertain/pending receipt; inspect its message ID and do not repeat the write merely because completion is unconfirmed.

## Recovery and limits

- **Native agent blocked:** focus its recorded workspace with `w`, then `Enter`, and inspect approvals/authentication yourself. Do not repeatedly send messages as a substitute for native interaction.
- **Session or checkout changed:** queued input must not be redirected to another session. Inspect the recorded identity and recovery details; sending is not an automatic restart mechanism.
- **Comment persistence uncertain:** managed recovery looks for the exact marked comment body in authoritative Beads history; if present, it confirms and applies the message. Absence is not permission to replay an uncertain write. Prelaunch plain comments have no journal marker/recovery record: inspect Beads yourself before retrying. A prepared managed write not yet attempted can still proceed safely during reconciliation.
- **Prompt delivery uncertain:** inspect the thread and agent workspace. The original verified agent can handle the exact input from `inbox` and acknowledge it with `respond`, settling that message without resending even if the transport attempt remains uncertain in history. This does not release an attempt still recorded as in flight; submission must have returned and its completion been recorded. A sender crash requires human inspection, not a blind resend. There is no dedicated retry/reset command or timeout that clears the hold.
- **Retained draft with submission blocked:** inspect comments and journal evidence before recreating or resending it. Editing the retained draft does not remove the uncertainty or re-enable Save/Send. You may close it with `Esc`; that does not cancel a write that may already have occurred.
- **External Git integration while discussion remains:** an exact answer to the existing question may still be sent, and existing inputs can still be acknowledged with `respond`, subject to the original-session/live-Bead checks. New unrelated input and `ask` are refused; Git ancestry alone does not bypass the discussion hold.
- **Acknowledged but not ready:** this is intentional. The agent must finish its checks and reassert readiness; acknowledgement alone cannot authorize integration or cleanup.

See the [user guide](user-guide.md#launch-an-agent-with-herdr) for worktree recovery and verified closure, and the [development guide](development.md) for the surrounding lifecycle.

## Developer contract and checks

### Ownership and persistence

- `src/main.rs` dispatches `inbox`, `ask`, and `respond` before interactive terminal setup. Keep validation errors out of raw mode and avoid changing the ordinary browser startup path.
- `src/beads.rs` owns public Beads comment CLI arguments and response validation. Thread text lives in durable Beads comments; do not access the Beads database directly.
- `src/worktrees/discussion.rs` and `src/worktrees.rs` own per-entry discussion control, message IDs, delivery evidence, original session/worktree validation, and lifecycle gates in the repository journal. Preserve the existing registry locking and possibly-sent fences rather than treating comment history as a transport queue.
- `src/herdr.rs` supplies conversation instructions in the initial agent prompt. `src/worktrees/discussion.rs` validates native identity and submits pane-targeted discussion prompts through the worktree executor; never substitute global focus or treat native blocked state as permission to inject approval input.
- `src/app.rs`, `src/hotkeys.rs`, and `src/ui.rs` own the `t` composer, captured question identity, worker responses, controls/help, and visible discussion state. CLI/Beads/Herdr work must stay off the terminal input loop, and stale responses must not overwrite a newer ticket or composer.

The journal is `<Git common directory>/ontrack-worktrees/registry.json`, shared by linked worktrees. Registry versions 1 and 2 are accepted; the next successful save writes version 2 while preserving entries. Per-entry `discussion_control` and marked-comment metadata use their own version 1. Unknown registry or control versions, malformed references, or mismatched repository identity are rejected without rewriting the registry. Do not remove the journal or edit version numbers to bypass a hold; use compatible software and inspect the retained evidence.

Message IDs are handoff-local monotonic IDs (`HANDOFF_ID:mSEQUENCE`), not Beads comment IDs. Managed comments contain an `[ontrack-message JSON]` marker with version, message ID, kind, reply references, handoff, and session; recovery compares the complete stored body, not a loosely matching text fragment. The journal owns control state, not a reconstruction from arbitrary comment edits.

Beads comments and external prompt calls cannot commit atomically with the journal. Persist intent and possibly-sent fences before external writes. Release the registry lock around prompt submission so a synchronous agent response can journal its acknowledgement; reload and update only the matching attempt afterward, preserving newer messages and acknowledgements. Keep the outstanding attempt's `in_flight` gate independent of message acknowledgement until submission actually returns and completion is recorded. A sender crash leaves that gate conservative; do not add replay or lease expiry as recovery. Distinguish accepted delivery from explicit acknowledgement, and never automatically replay a possibly sent operation.

Uncertain initial journal durability and failed unjournaled prelaunch comment writes are typed errors, not successful durable-intent receipts. The composer must retain the draft and block both actions for these outcomes while allowing editing/cancellation. Normal prewrite failures remain explicitly retryable. Later delivery failures after durable intent return receipts instead, preventing the UI from treating that intent as a fresh unsaved draft.

### Expected regression coverage

Use inline tests and injected CLI executors/disposable Git fixtures, not live agent sessions, for:

- Comment argv/validation and durable history; saving before launch or during a question without prompting, carrying a reply reference, or acknowledging anything.
- Composer multiline input, text Enter versus action Enter, focus cycling, submission locking, cancellation, stale worker responses, retryable prewrite errors, retained-but-submission-blocked uncertain drafts, and durable-intent receipts.
- Answers bound to the question captured at composer opening; a newer unseen question remains unresolved.
- One unresolved question per handoff, atomic answer acknowledgement/question replacement, independent user sends, repeated exact reply-ID acknowledgement, invalid/duplicate reference rejection, and CLI dispatch without terminal initialization.
- Busy/blocked delivery deferral, original-session/worktree mismatch rejection, conservative late native-session binding, accepted-but-unacknowledged state, uncertain submission across restart, and no automatic resend.
- Early acknowledgement while submission remains in flight, sender-crash holds, and exact existing-question answers after external integration without admitting unrelated obligations.
- Readiness withdrawal and merge/closure/cleanup refusal for pending or unresolved discussion, including manual actions and automatic reconciliation; acknowledgement must not restore readiness.
- Unchanged one-shot behavior without messages, advisory-only status reports, v1-to-v2 registry migration on save, unsupported registry/control schema refusal, and verified Git closure.

Run focused suites for the changed modules (for example, `cargo test worktrees::`, `cargo test beads::tests`, `cargo test herdr::tests`, `cargo test app::tests`, and `cargo test ui::tests`), then the repository gates:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```

These are required verification paths and expected coverage, not a claim that checks have passed. Interactive validation must also exercise composer keys, native-workspace focus, and terminal restoration in a real terminal or PTY; mocked CLI tests cannot prove an external agent read a message.
