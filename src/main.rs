mod app;
mod beads;
mod detail;
mod epic;
mod event;
mod herdr;
mod hotkeys;
mod release;
mod screenshots;
mod settings;
mod startup;
mod terminal;
mod theme;
mod ui;
mod uninstall;
mod update;
mod updates;
mod worktrees;

use std::{error::Error, io};

fn main() -> Result<(), Box<dyn Error>> {
    match std::env::args_os().skip(1).collect::<Vec<_>>().as_slice() {
        [] => {}
        [command] if command == "uninstall" => {
            uninstall::run(false)?;
            return Ok(());
        }
        [command, flag] if command == "uninstall" && flag == "--yes" => {
            uninstall::run(true)?;
            return Ok(());
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!(
                "Usage: ontrack [uninstall [--yes]]\n\nRun without arguments to browse the current Beads workspace.\nUninstall removes the running OnTrack executable and user settings after confirmation."
            );
            return Ok(());
        }
        _ => return Err("Usage: ontrack [uninstall [--yes]]".into()),
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
