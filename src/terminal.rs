use std::{io, panic};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::backend::CrosstermBackend;

use crate::Tui;

pub fn init() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
        let _ = restore_terminal();
        return Err(error);
    }
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        previous_hook(info);
    }));
    match ratatui::Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            let _ = restore_terminal();
            Err(error)
        }
    }
}

pub fn restore(terminal: &mut Tui) -> io::Result<()> {
    let draw_result = terminal.show_cursor();
    let restore_result = restore_terminal();
    draw_result.and(restore_result)
}

fn restore_terminal() -> io::Result<()> {
    let raw_result = disable_raw_mode();
    let screen_result = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    raw_result.and(screen_result)
}
