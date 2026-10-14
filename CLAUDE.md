# Agent Instructions

## Project

- Single Rust 2024 binary; requires Rust 1.88+. This is an early terminal-app scaffold, with no project or task tracking features implemented.
- Run `cargo run` in a terminal with alternate-screen support, not a non-interactive runner.
- When working with Herdr integrations, reference https://herdr.dev/docs.
- When working with Beads, reference https://beads.gascity.com.

## Checks and invariants

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```

- Tests are inline; focused checks: `cargo test event::tests` or `cargo test ui::tests`.
- Preserve terminal cleanup: `src/main.rs` restores after the app returns; `src/terminal.rs` handles raw mode and alternate-screen cleanup on normal exit, initialization failure, and panic.
- `src/ui.rs` embeds `assets/splash.png` at compile time; keep the asset available when building.

## Workflow

- Load `.agents/skills/beads/SKILL.md` and run `bd prime` for workflow context.
- Use `bd` for all task tracking and `bd remember` for persistent memory; do not create markdown TODO or MEMORY files.
- Issues live in the local Dolt database (`.beads/dolt/`). `.beads/issues.jsonl` is a passive export, not the source of truth; do not run `bd import` during normal work.
- All changes must be committed with semantic commit messages (e.g. `docs: refresh agent instructions`).
- Git currently has no remote. Do not push or run Dolt remote sync without an explicit request.
- Keep `AGENTS.md` and `CLAUDE.md` identical; they are independently tracked files.
- At handoff, report changed files, checks performed, issue status, and any blockers.
