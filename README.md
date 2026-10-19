# ontrack

A terminal browser for Beads issues in the current workspace.

## Quick install

On macOS or Ubuntu 24.04-compatible Linux, with Bash, `curl`, and Python 3 installed, run this when a compatible stable release is available. If Beads (`bd`) or Herdr is missing, the installer offers to install it using its official installer:

```sh
curl -fsSLo install-ontrack.sh https://raw.githubusercontent.com/ThomasJRyan/OnTrack/main/scripts/install.sh && bash install-ontrack.sh
```

The installer puts `ontrack` in `~/.local/bin` by default. Add that directory to `PATH` if needed, then run `ontrack` from a Herdr shell in the Git workspace you want to browse. See the [installation guide](docs/installation.md) for supported architectures, updates, and troubleshooting.

To remove the installed executable and OnTrack user settings, run `ontrack uninstall` and confirm the printed paths. Beads and Herdr remain installed; see [uninstall details](docs/installation.md#uninstall).

- Browse open and closed Beads, with explicit create, edit, and permanent-delete actions.
- Mark an open issue of any type Ready, or mark an Epic and all its open descendant Tasks Ready together. Press `r` again to unready an unsent issue; sent issues stay Ready until their Beads status changes. The scheduler prepares a Ready Epic's checkout and hands eligible Ready issues to Herdr agents. Set a concurrent agent limit in Settings (five by default).
- Press `Shift+N` on an open task to create a child task. Child issues appear indented beneath their parent within a status pane. Parent handoff waits for children, and completed child work automatically requests integration into the managed parent worktree by default; Settings can turn this off.
- When creating an Epic, or later with `g` and confirmation, optionally generate child tasks with assessed P0–P4 priorities from its Description using a Herdr agent in a planning tab inside the current workspace at the same checkout.
- Press `p` to discuss a new Task or Epic with an agent in OnTrack, answer its choices or type your own response, review the proposal, and confirm creation of Beads.
- Two-pane browsing with scrollable, Markdown-rendered descriptions, notes, comments, relationships, and Beads details.
- Paste PNG or JPEG screenshots into a Bead description with `Ctrl+V`; agent handoffs include the saved local image paths.
- Automatic background refresh picks up external changes, with cached records and visible errors; no watcher setup required.
- Run `ontrack update` in a terminal to check the latest compatible release and confirm a verified installation. Restart any running OnTrack session after an update.
- Persistent theme and default new-task agent selection in a structured settings dialog.
- A brief animated terminal splash with the ONTRACK wordmark and a waving anime girl, with compact layouts for smaller terminals.
- Press `h` to hand a selected Bead to a Herdr agent in a new worktree. A top-right toast shows progress and the result; Open and Ready issues show `· sent` once the prompt is confirmed. See the [user guide](docs/user-guide.md) for setup and recovery limits.
- Press `w` to inspect OnTrack-tracked outstanding worktrees, with colour-coded statuses and Bead details. Completed tasks show Ready to Merge after the Bead closes and the recorded agent settles; enabled preferences queue merge requests one at a time, then clean up verified integrated worktrees. Older handoffs without enough evidence show a manual merge hint. Active merge work shows its Git state, agents can add an advisory status report, and verified removed checkouts leave this view. See the [user guide](docs/user-guide.md) for status reporting. Press `Enter` to switch to a worktree's Herdr workspace, `a` to ask a verified idle agent to continue in the same checkout, or `Shift+R` to confirm a fresh start for the task; merge and safe cleanup remain available separately.

## Requirements

Source builds need Rust 1.88 or newer and the `bd` command-line tool. Launch the browser inside a Herdr workspace, where `HERDR_ENV=1` is set. `ontrack update` needs `curl` and `tar`, but does not require Herdr or a Beads workspace; release installation offers to install missing `bd` and Herdr commands. If no Beads workspace is available, OnTrack tells you to run **`bd init`** and exits; initialize the workspace yourself, then relaunch. Existing workspace settings are preserved. A terminal with alternate-screen and mouse support is recommended. See [installation and updates](docs/installation.md) for supported systems and prerequisites.

## Run

```sh
cargo run
```

Issues are grouped into coloured, collapsible panes. Open, Ready, Blocked, Deferred, In Progress, and Closed remain visible with zero counts when empty; Closed starts collapsed after loading. Press `Ctrl+?` for hotkeys, `r` (or `Shift+R`) to toggle Ready for any open issue, `p` to plan new work, `w` for tracked handoffs, `s` for settings, `n` to create a bead, `Shift+N` to create a child task, `e` to edit, and `d` to delete (confirmation required). `Esc` goes back or closes a dialog; `q` and `Ctrl+C` quit. See the [user guide](docs/user-guide.md) for controls and the [development guide](docs/development.md) for scheduling and tracked worktrees.

See the [installation guide](docs/installation.md) for release installs, the [user guide](docs/user-guide.md) for controls, the [settings guide](docs/settings.md) for preferences, the [development guide](docs/development.md) for architecture and checks, the [release guide](docs/releases.md) for publishing, and the [official Beads documentation](https://beads.gascity.com) for workspace setup.

## Checks

GitHub Actions runs these checks on pull requests and pushes to `main`. See the [CI guide](docs/ci.md) for results and required-check setup.

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```
