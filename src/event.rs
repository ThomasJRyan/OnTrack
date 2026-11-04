use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

const RESPONSE_CHECK_INTERVAL: Duration = Duration::from_millis(100);

pub fn redraw_due(dirty: bool, visual_update: Option<Instant>, now: Instant) -> bool {
    dirty || visual_update.is_some_and(|deadline| now >= deadline)
}

pub fn wait_timeout(visual_update: Option<Instant>, now: Instant) -> Duration {
    visual_update.map_or(RESPONSE_CHECK_INTERVAL, |deadline| {
        deadline
            .saturating_duration_since(now)
            .min(RESPONSE_CHECK_INTERVAL)
    })
}

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
    fn idle_wait_is_bounded_without_requesting_redraw() {
        let now = Instant::now();
        assert!(!redraw_due(false, None, now));
        assert_eq!(wait_timeout(None, now), Duration::from_millis(100));
        assert_eq!(
            wait_timeout(Some(now + Duration::from_secs(1)), now),
            Duration::from_millis(100)
        );
        assert!(redraw_due(true, None, now));
    }

    #[test]
    fn visual_deadline_wakes_and_is_consumed_by_a_settled_render() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(35);
        assert!(!redraw_due(false, Some(deadline), now));
        assert_eq!(wait_timeout(Some(deadline), now), Duration::from_millis(35));
        for rendered_at in [deadline, deadline + Duration::from_secs(1)] {
            assert!(redraw_due(false, Some(deadline), rendered_at));
            assert_eq!(wait_timeout(Some(deadline), rendered_at), Duration::ZERO);
            // A completed visual update replaces its deadline after rendering.
            assert!(!redraw_due(false, None, rendered_at));
            assert_eq!(wait_timeout(None, rendered_at), RESPONSE_CHECK_INTERVAL);
        }
    }

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
