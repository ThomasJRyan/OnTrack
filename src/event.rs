use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

pub fn poll_event(timeout: Duration) -> std::io::Result<Option<Event>> {
    if !event::poll(timeout)? {
        return Ok(None);
    }
    Ok(Some(event::read()?))
}

pub fn is_quit(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('q')
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_quit_keys_and_ignores_others() {
        assert!(is_quit(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE
        )));
        assert!(!is_quit(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(is_quit(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_quit(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE
        )));
    }
}
