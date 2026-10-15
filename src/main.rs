mod app;
mod beads;
mod detail;
mod epic;
mod event;
mod herdr;
mod hotkeys;
mod settings;
mod terminal;
mod theme;
mod ui;
mod update;
mod worktrees;

use std::{error::Error, io};

fn main() -> Result<(), Box<dyn Error>> {
    beads::ensure_workspace()?;
    let mut terminal = terminal::init()?;
    let result = app::run(&mut terminal);
    let restore_result = terminal::restore(&mut terminal);
    result?;
    restore_result?;
    Ok(())
}

pub type Tui = ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>;
