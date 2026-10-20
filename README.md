# OnTrack

**Turn your backlog into agent work, without leaving the terminal.**

OnTrack brings [Beads](https://beads.gascity.com) and [Herdr](https://herdr.dev) together in one keyboard-driven workspace. See what needs doing, give it to a coding agent, and follow the work through to integration.

> **Application screenshot coming soon.**
<!-- Replace the placeholder above with a screenshot of the issue browser and details pane. -->

## From idea to implementation

Browse your backlog beside its Markdown descriptions, comments, and dependencies. Create a task or Epic and add child tasks manually. Paste PNG or JPEG screenshots into descriptions with `Ctrl+V`; handoffs include the saved local image paths.

Mark tasks **Ready** and let OnTrack dispatch eligible work to Herdr agents in separate Git worktrees, with a concurrency limit you control. Need to start something immediately? Hand the selected Bead to an agent with `h`.

Keep the bigger picture in view as agents work. Status groups refresh automatically, and the worktree view lets you jump into an agent's workspace, request a merge, and clean up after verified integration. Configure automatic merging and cleanup when they fit your workflow. Continue work in the same checkout with `a`, or confirm a fresh start with `Shift+R`.

Other features include a two-pane browser with cached Beads details and visible refresh errors, child tasks that wait for their parent handoff and can request integration when complete, and a short animated startup splash with compact-terminal layouts. Settings include a theme, default agent, and concurrent-agent limit. Run `ontrack update` to check for and install a verified compatible release.

## Get started

For macOS or Ubuntu 24.04-compatible Linux, with Bash, `curl`, and Python 3:

```sh
curl -fsSLo install-ontrack.sh https://raw.githubusercontent.com/ThomasJRyan/OnTrack/main/scripts/install.sh && bash install-ontrack.sh
```

The installer uses a compatible stable release and offers to install missing Beads and Herdr commands. Add `~/.local/bin` to your `PATH` if needed, open your project in a **Herdr shell**, then run:

```sh
ontrack
```

Use an interactive terminal with alternate-screen support. If the project has no Beads workspace yet, run `bd init` first. Building from this checkout instead? With Rust 1.88+ installed, run `cargo run` inside Herdr.

See [installation and updates](docs/installation.md) for supported architectures, prerequisites, updates, and removal. To remove the installed executable and OnTrack user settings, run `ontrack uninstall` and confirm the printed paths; Beads and Herdr remain installed.

## Keys to keep moving

| Key | Action |
| --- | --- |
| `↑` / `↓`, `Tab` | Select issues; switch panes to scroll details |
| `←` / `→` | Collapse / expand a status group in the issue list |
| `n` / `Shift+N`, `e` | Create a Bead / child task, or edit the selection |
| `r` | Toggle Ready for an open Task or an Epic and its tasks |
| `h` | Hand the selected Bead to a Herdr agent |
| `w` | Inspect worktrees; `Enter` opens the selected Herdr workspace |
| `s`, `Ctrl+?` | Open settings or context-specific hotkey help |
| `Esc`, `q` / `Ctrl+C` | Go back; quit when no dialog is open |

Issues are grouped into coloured, collapsible panes. Open, Ready, Blocked, Deferred, In Progress, and Closed remain visible with zero counts when empty; Closed starts collapsed after loading. Press `Ctrl+?` for hotkeys, `s` for settings, and `d` to delete (confirmation required). See the [user guide](docs/user-guide.md) for the full workflow and controls, [settings](docs/settings.md) for preferences, the [development guide](docs/development.md) for architecture and checks, [CI checks](docs/ci.md), and the [release guide](docs/releases.md).
