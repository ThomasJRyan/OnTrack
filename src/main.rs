mod app;
mod beads;
mod detail;
mod epic;
mod event;
mod herdr;
mod hotkeys;
mod planning;
mod screenshots;
mod settings;
mod startup;
mod terminal;
mod theme;
mod ui;
mod uninstall;
mod update;
mod update_cli;
mod worktrees;

use std::{error::Error, io};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args
        .first()
        .is_some_and(|a| a == "inbox" || a == "ask" || a == "respond")
    {
        let args = args
            .into_iter()
            .map(|a| {
                a.into_string()
                    .map_err(|_| "Conversation arguments must be UTF-8")
            })
            .collect::<Result<Vec<_>, _>>()?;
        println!(
            "{}",
            worktrees::discussion_cli(&std::env::current_dir()?, &args)?
        );
        return Ok(());
    }
    match std::env::args_os().skip(1).collect::<Vec<_>>().as_slice() {
        [] => {}
        [command] if command == "update" => {
            update_cli::run()?;
            return Ok(());
        }
        [command] if command == "uninstall" => {
            uninstall::run(false)?;
            return Ok(());
        }
        [command, flag] if command == "uninstall" && flag == "--yes" => {
            uninstall::run(true)?;
            return Ok(());
        }
        [command, state] if command == "status" => {
            worktrees::report_activity(&std::env::current_dir()?, &state.to_string_lossy(), None)?;
            println!("Worktree status report saved: {}", state.to_string_lossy());
            return Ok(());
        }
        [command, state, detail] if command == "status" => {
            worktrees::report_activity(
                &std::env::current_dir()?,
                &state.to_string_lossy(),
                Some(&detail.to_string_lossy()),
            )?;
            println!("Worktree status report saved: {}", state.to_string_lossy());
            return Ok(());
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("Ticket conversation (from the managed checkout):\n  ontrack inbox [--json]\n  ontrack ask [--reply-to MESSAGE_ID ...] TEXT\n  ontrack respond --reply-to MESSAGE_ID [...] TEXT\nQuote TEXT as one argument. Ask then yield; inbox does not acknowledge.\n");
            println!(
                "Usage: ontrack [update | uninstall [--yes] | status <working|merging|blocked|clear> [detail]]\n\nRun without arguments to browse the current Beads workspace.\nUpdate checks for a newer compatible release and asks before installing it.\nStatus reports advisory activity for the current managed checkout; clear removes it.\nUninstall removes the running OnTrack executable and user settings after confirmation."
            );
            return Ok(());
        }
        _ => return Err(
            "Usage: ontrack [update | uninstall [--yes] | status <working|merging|blocked|clear> [detail]]"
                .into(),
        ),
    }
    startup::ensure_herdr_workspace()?;
    if !startup::ensure_workspace()? {
        return Ok(());
    }
    let mut terminal = terminal::init()?;
    let result = app::run(&mut terminal);
    let restore_result = terminal::restore(&mut terminal);
    result?;
    restore_result?;
    Ok(())
}

pub type Tui = ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>;
