mod app;
mod beads;
mod detail;
mod event;
mod terminal;
mod ui;

use std::{error::Error, io};

fn main() -> Result<(), Box<dyn Error>> {
    let mut terminal = terminal::init()?;
    let result = app::run(&mut terminal);
    let restore_result = terminal::restore(&mut terminal);
    result?;
    restore_result?;
    Ok(())
}

pub type Tui = ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>;
