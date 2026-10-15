use crate::app::{App, Modal};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Issues,
    Details,
    Worktrees,
    Form,
    FormChoices,
    Settings,
    SettingsChoices,
    EditLoading,
    Delete,
    WorktreeConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Help {
    pub context: Context,
    pub offset: usize,
}

impl Context {
    pub fn current(app: &App) -> Self {
        match app.modal.as_ref() {
            Some(Modal::Create(form) | Modal::Edit { form, .. }) if form.dropdown.is_some() => {
                Self::FormChoices
            }
            Some(Modal::Create(_) | Modal::Edit { .. }) => Self::Form,
            Some(Modal::Settings {
                dropdown_open: true,
                ..
            }) => Self::SettingsChoices,
            Some(Modal::Settings { .. }) => Self::Settings,
            Some(Modal::EditLoading { .. }) => Self::EditLoading,
            Some(Modal::ConfirmDelete { .. }) => Self::Delete,
            Some(Modal::ConfirmWorktree { .. }) => Self::WorktreeConfirm,
            None if app.worktrees_view => Self::Worktrees,
            None if app.focus_detail => Self::Details,
            None => Self::Issues,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Issues => "Issue list",
            Self::Details => "Issue details",
            Self::Worktrees => "Outstanding worktrees",
            Self::Form => "Create / edit form",
            Self::FormChoices => "Form choice list",
            Self::Settings => "Settings",
            Self::SettingsChoices => "Theme choices",
            Self::EditLoading => "Loading edit",
            Self::Delete => "Delete confirmation",
            Self::WorktreeConfirm => "Worktree confirmation",
        }
    }

    pub fn entries(self) -> Vec<(&'static str, &'static str)> {
        let mut rows = match self {
            Self::Issues => vec![
                ("↑ / ↓", "Select an issue or collapsed group"),
                ("← / →", "Collapse / expand selected status group"),
                ("Space", "Toggle selected status group"),
                ("Tab", "Focus issue details"),
                ("r", "Toggle Ready for an open Task or Epic and its tasks"),
                ("s", "Open settings"),
                ("n", "Create a bead"),
                ("Shift+N", "Create child of selected open task"),
                ("e", "Edit selected bead"),
                ("d", "Delete selected bead after confirmation"),
                ("h", "Hand selected bead to Herdr"),
                (
                    "Shift+R",
                    "Toggle Ready for an open Task or Epic and its tasks",
                ),
                ("w", "Open outstanding worktrees"),
                ("Esc", "Stay in the issue browser"),
            ],
            Self::Details => vec![
                ("↑ / ↓", "Scroll issue details one line"),
                ("PgUp / PgDn", "Scroll issue details one page"),
                ("Tab", "Focus issue list"),
                ("r", "Toggle Ready for an open Task or Epic and its tasks"),
                (
                    "Shift+R",
                    "Toggle Ready for an open Task or Epic and its tasks",
                ),
                ("s", "Open settings"),
                ("n", "Create a bead"),
                ("Shift+N", "Create child of selected open task"),
                ("e", "Edit selected bead"),
                ("d", "Delete selected bead after confirmation"),
                ("h", "Hand selected bead to Herdr"),
                ("w", "Open outstanding worktrees"),
                ("Esc", "Stay in the issue browser"),
            ],
            Self::Worktrees => vec![
                ("↑ / k", "Select previous worktree"),
                ("↓ / j", "Select next worktree"),
                ("PgUp / PgDn", "Scroll worktree details one page"),
                ("Enter", "Focus selected Herdr workspace"),
                ("m", "Request merge for selected handoff"),
                ("c", "Close integrated Herdr workspace"),
                ("x", "Remove integrated checkout"),
                ("Shift+R", "Restart selected active task"),
                ("r", "Refresh outstanding worktrees"),
                ("w / Esc / Backspace", "Return to issue browser"),
            ],
            Self::Form => vec![
                ("Text / Space", "Type in title or description"),
                ("Tab / Shift+Tab", "Move between form controls"),
                ("← / →", "Move text cursor or cycle focused choice"),
                ("↑ / ↓", "Move description cursor between visible lines"),
                ("Home / End", "Move to start / end of text line"),
                ("Backspace / Delete", "Remove text beside the cursor"),
                (
                    "Enter",
                    "Advance from text; open choice; submit or cancel on buttons",
                ),
                ("Space", "Open focused choice list"),
                ("Shift+Enter", "Insert a description newline"),
                ("Esc", "Discard form and close"),
            ],
            Self::FormChoices => vec![
                ("↑ / ↓", "Move through choices"),
                ("← / →", "Cycle choices with wraparound"),
                ("Enter / Space", "Accept selected choice"),
                ("Esc", "Restore original choice and close list"),
            ],
            Self::Settings => vec![
                ("↑ / ↓", "Move between setting rows"),
                ("Tab / Shift+Tab", "Move between setting rows"),
                ("← / →", "Cycle themes when Theme is focused"),
                (
                    "Enter / Space",
                    "Open theme choices, toggle preference, or close",
                ),
                ("Esc", "Close settings"),
            ],
            Self::SettingsChoices => vec![
                ("↑ / ↓", "Move to another setting and close choices"),
                (
                    "Tab / Shift+Tab",
                    "Move to another setting and close choices",
                ),
                ("← / →", "Cycle themes and save immediately"),
                ("Enter / Space", "Close choices and save theme"),
                ("Esc", "Close theme choices"),
            ],
            Self::EditLoading => vec![
                ("e", "Retry a failed edit load"),
                ("Esc", "Cancel edit loading"),
            ],
            Self::Delete | Self::WorktreeConfirm => vec![
                ("y / Y", "Confirm action"),
                ("n / N / Enter", "Cancel action"),
                ("Esc", "Cancel action"),
            ],
        };
        rows.extend([
            ("Ctrl+?", "Open or close this hotkey reference"),
            ("↑ / k · ↓ / j", "Scroll this reference while it is open"),
            ("PgUp / PgDn", "Scroll this reference by ten lines"),
            ("Home / End", "Jump to start / end of this reference"),
            ("Esc", "Close this reference while it is open"),
        ]);
        if matches!(self, Self::Issues | Self::Details | Self::Worktrees) {
            rows.push(("q / Ctrl+C", "Quit ontrack"));
        }
        rows
    }
}

pub fn is_help_key(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(
            key.code,
            KeyCode::Char('?') | KeyCode::Char('/') | KeyCode::Char('_')
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_reported_help_keys_without_stealing_text_editing() {
        for (code, modifiers) in [
            (KeyCode::Char('?'), KeyModifiers::CONTROL),
            (
                KeyCode::Char('/'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            (KeyCode::Char('/'), KeyModifiers::CONTROL),
            (KeyCode::Char('_'), KeyModifiers::CONTROL),
        ] {
            assert!(is_help_key(KeyEvent::new(code, modifiers)));
        }
        for code in [KeyCode::Char('?'), KeyCode::Char('/'), KeyCode::Backspace] {
            assert!(!is_help_key(KeyEvent::new(code, KeyModifiers::NONE)));
        }
    }
}
