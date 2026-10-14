use crate::{
    Tui,
    beads::{self, Bead, CreateBead},
    event, ui,
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub(super) enum Request {
    List,
    Show(String, u64),
    Create(CreateBead),
    Delete(String),
}
pub(super) enum Response {
    List(std::io::Result<Vec<Bead>>),
    Show(String, u64, std::io::Result<serde_json::Value>),
    Created(std::io::Result<String>),
    Deleted(std::io::Result<()>),
}
pub struct App {
    pub beads: Vec<Bead>,
    pub selected: Option<usize>,
    pub detail: Option<serde_json::Value>,
    pub focus_detail: bool,
    pub list_offset: usize,
    pub detail_offset: u16,
    pub list_loading: bool,
    pub detail_loading: bool,
    pub list_error: Option<String>,
    pub detail_error: Option<String>,
    pub(super) detail_seq: u64,
    pub list_rect: Rect,
    pub detail_rect: Rect,
    pub modal: Option<Modal>,
    pub notice: Option<String>,
    pub mutation_busy: bool,
    pub modal_ready: bool,
    pub preferred_id: Option<String>,
    pub(super) requests: mpsc::Sender<Request>,
    pub(super) responses: mpsc::Receiver<Response>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Create(CreateForm),
    ConfirmDelete { id: String, title: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateForm {
    pub title: String,
    pub description: String,
    pub issue_type: usize,
    pub priority: usize,
    pub focus: usize,
    pub cursor: usize,
    pub description_cursor: usize,
    pub dropdown: Option<usize>,
    pub dropdown_original: Option<usize>,
}
impl Default for CreateForm {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            issue_type: 0,
            priority: 2,
            focus: 0,
            cursor: 0,
            description_cursor: 0,
            dropdown: None,
            dropdown_original: None,
        }
    }
}
fn previous_boundary(s: &str, cursor: usize) -> usize {
    s[..cursor.min(s.len())]
        .char_indices()
        .last()
        .map_or(0, |(i, _)| i)
}
fn next_boundary(s: &str, cursor: usize) -> usize {
    let cursor = cursor.min(s.len());
    s[cursor..]
        .char_indices()
        .nth(1)
        .map_or(s.len(), |(i, _)| cursor + i)
}
fn move_vertical(s: &str, cursor: usize, down: bool) -> usize {
    let cursor = cursor.min(s.len());
    let line_start = s[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let column = s[line_start..cursor].chars().count();
    let target_start = if down {
        s[cursor..].find('\n').map(|i| cursor + i + 1)
    } else if line_start > 0 {
        Some(s[..line_start - 1].rfind('\n').map_or(0, |i| i + 1))
    } else {
        None
    };
    let Some(start) = target_start else {
        return cursor;
    };
    let end = s[start..].find('\n').map_or(s.len(), |i| start + i);
    start
        + s[start..end]
            .char_indices()
            .nth(column)
            .map_or(end - start, |(i, _)| i)
}
impl App {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Request>();
        let (out, responses) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(request) = rx.recv() {
                let response = match request {
                    Request::List => Response::List(beads::list_beads()),
                    Request::Show(id, seq) => {
                        Response::Show(id.clone(), seq, beads::show_bead(&id))
                    }
                    Request::Create(input) => Response::Created(beads::create_bead(&input)),
                    Request::Delete(id) => Response::Deleted(beads::delete_bead(&id)),
                };
                if out.send(response).is_err() {
                    break;
                }
            }
        });
        let mut a = Self {
            beads: vec![],
            selected: None,
            detail: None,
            focus_detail: false,
            list_offset: 0,
            detail_offset: 0,
            list_loading: true,
            detail_loading: false,
            list_error: None,
            detail_error: None,
            detail_seq: 0,
            list_rect: Rect::default(),
            detail_rect: Rect::default(),
            modal: None,
            notice: None,
            mutation_busy: false,
            modal_ready: true,
            preferred_id: None,
            requests: tx,
            responses,
        };
        a.refresh();
        a
    }
    fn refresh(&mut self) {
        self.list_loading = true;
        self.list_error = None;
        self.detail_offset = 0;
        let _ = self.requests.send(Request::List);
    }
    fn select(&mut self, index: usize) {
        if self.beads.is_empty() {
            self.selected = None;
            self.detail = None;
            return;
        }
        let index = index.min(self.beads.len() - 1);
        if self.selected == Some(index) {
            return;
        }
        self.selected = Some(index);
        self.detail_seq = self.detail_seq.wrapping_add(1);
        self.detail = Some(self.beads[index].data.clone());
        self.detail_offset = 0;
        self.detail_loading = true;
        self.detail_error = None;
        let _ = self
            .requests
            .send(Request::Show(self.beads[index].id.clone(), self.detail_seq));
    }
    fn step(&mut self, d: isize) {
        if let Some(i) = self.selected {
            self.select(
                i.saturating_add_signed(d)
                    .min(self.beads.len().saturating_sub(1)),
            )
        } else if !self.beads.is_empty() {
            self.select(0)
        }
    }
    fn receive(&mut self) {
        while let Ok(r) = self.responses.try_recv() {
            match r {
                Response::List(res) => {
                    self.list_loading = false;
                    match res {
                        Ok(items) => {
                            let id = self
                                .selected
                                .and_then(|i| self.beads.get(i))
                                .map(|b| b.id.clone());
                            self.beads = items;
                            self.selected = self
                                .preferred_id
                                .take()
                                .and_then(|id| self.beads.iter().position(|b| b.id == id))
                                .or_else(|| {
                                    id.and_then(|id| self.beads.iter().position(|b| b.id == id))
                                        .or_else(|| (!self.beads.is_empty()).then_some(0))
                                });
                            self.detail = None;
                            self.detail_loading = false;
                            self.detail_error = None;
                            self.detail_offset = 0;
                            if let Some(i) = self.selected {
                                self.detail = Some(self.beads[i].data.clone());
                                self.detail_loading = true;
                                self.detail_error = None;
                                self.detail_seq = self.detail_seq.wrapping_add(1);
                                let _ = self
                                    .requests
                                    .send(Request::Show(self.beads[i].id.clone(), self.detail_seq));
                            }
                            self.list_error = None;
                        }
                        Err(e) => self.list_error = Some(e.to_string()),
                    }
                }
                Response::Show(id, seq, res) => {
                    if self
                        .selected
                        .and_then(|i| self.beads.get(i))
                        .is_some_and(|b| b.id == id)
                        && seq == self.detail_seq
                    {
                        match res {
                            Ok(v) => {
                                self.detail = Some(v);
                                self.detail_loading = false;
                                self.detail_error = None;
                            }
                            Err(e) => {
                                self.detail_loading = false;
                                self.detail_error = Some(e.to_string());
                            }
                        }
                    }
                }
                Response::Created(result) => {
                    self.mutation_busy = false;
                    match result {
                        Ok(id) => {
                            self.notice = Some(format!("Created {id}"));
                            self.modal = None;
                            self.preferred_id = Some(id);
                            self.refresh();
                        }
                        Err(e) => {
                            self.notice = Some(format!("Create failed: {e}"));
                        }
                    }
                }
                Response::Deleted(result) => {
                    self.mutation_busy = false;
                    match result {
                        Ok(()) => {
                            self.notice = Some("Bead deleted".into());
                            self.modal = None;
                            self.refresh();
                        }
                        Err(e) => self.notice = Some(format!("Delete failed: {e}")),
                    }
                }
            }
        }
    }
    fn key(&mut self, k: KeyEvent) -> bool {
        if self.mutation_busy {
            return false;
        }
        if self.modal.is_some() {
            return self.modal_key(k);
        }
        if event::is_quit(k) {
            return true;
        }
        match k.code {
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Char('n') => {
                self.modal = Some(Modal::Create(CreateForm::default()));
                self.modal_ready = false;
                self.notice = None;
            }
            KeyCode::Char('d') => {
                if let Some(b) = self.selected.and_then(|i| self.beads.get(i)) {
                    self.modal = Some(Modal::ConfirmDelete {
                        id: b.id.clone(),
                        title: b.title.clone(),
                    });
                    self.modal_ready = false;
                    self.notice = None;
                }
            }
            KeyCode::Tab => self.focus_detail = !self.focus_detail,
            KeyCode::Left => self.focus_detail = false,
            KeyCode::Right => self.focus_detail = true,
            KeyCode::Up if self.focus_detail => {
                self.detail_offset = self.detail_offset.saturating_sub(1)
            }
            KeyCode::Down if self.focus_detail => {
                self.detail_offset = self.detail_offset.saturating_add(1)
            }
            KeyCode::PageUp if self.focus_detail => {
                self.detail_offset = self
                    .detail_offset
                    .saturating_sub(self.detail_rect.height.saturating_sub(2))
            }
            KeyCode::PageDown if self.focus_detail => {
                self.detail_offset = self
                    .detail_offset
                    .saturating_add(self.detail_rect.height.saturating_sub(2))
            }
            KeyCode::Up => self.step(-1),
            KeyCode::Down => self.step(1),
            _ => {}
        }
        false
    }
    fn modal_key(&mut self, key: KeyEvent) -> bool {
        use crossterm::event::KeyCode::*;
        if self.mutation_busy {
            return false;
        }
        if !self.modal_ready && key.code != Esc {
            return false;
        }
        if key.code == Esc {
            if let Some(Modal::Create(form)) = self.modal.as_mut()
                && let Some(which) = form.dropdown.take()
            {
                if let Some(old) = form.dropdown_original.take() {
                    match which {
                        2 => form.issue_type = old,
                        _ => form.priority = old,
                    }
                }
                return false;
            }
            self.modal = None;
            return false;
        }
        if let Some(Modal::ConfirmDelete { id, .. }) = self.modal.as_ref() {
            if matches!(key.code, Char('y') | Char('Y')) && !self.mutation_busy {
                let id = id.clone();
                self.mutation_busy = true;
                if self.requests.send(Request::Delete(id)).is_err() {
                    self.mutation_busy = false;
                    self.notice = Some("Delete could not be queued; try again".into());
                }
            }
            if matches!(key.code, Char('n') | Char('N') | Enter) {
                self.modal = None;
            }
            return false;
        }
        let Some(Modal::Create(form)) = self.modal.as_mut() else {
            return false;
        };
        if let Some(which) = form.dropdown {
            match key.code {
                Up => {
                    if which == 2 {
                        form.issue_type = form.issue_type.saturating_sub(1)
                    } else {
                        form.priority = form.priority.saturating_sub(1)
                    }
                }
                Down => {
                    if which == 2 {
                        form.issue_type = (form.issue_type + 1).min(beads::BEAD_TYPES.len() - 1)
                    } else {
                        form.priority = (form.priority + 1).min(4)
                    }
                }
                Left | Right => {}
                Enter | Char(' ') => {
                    form.dropdown = None;
                    form.dropdown_original = None;
                }
                _ => {}
            }
            return false;
        }
        match key.code {
            Tab => form.focus = (form.focus + 1) % 6,
            BackTab => form.focus = (form.focus + 5) % 6,
            Enter if form.focus == 2 || form.focus == 3 => {
                form.dropdown = Some(form.focus);
                form.dropdown_original = Some(if form.focus == 2 {
                    form.issue_type
                } else {
                    form.priority
                });
            }
            Enter if form.focus == 4 => {
                if !form.title.trim().is_empty() {
                    let input = CreateBead {
                        title: form.title.trim().into(),
                        description: form.description.clone(),
                        issue_type: beads::BEAD_TYPES[form.issue_type].into(),
                        priority: form.priority as u8,
                    };
                    self.mutation_busy = true;
                    if self.requests.send(Request::Create(input)).is_err() {
                        self.mutation_busy = false;
                        self.notice = Some("Create could not be queued; try again".into());
                    }
                } else {
                    self.notice = Some("Title is required".into());
                }
            }
            Enter if form.focus == 5 => {
                self.modal = None;
            }
            Char(' ') if form.focus == 2 || form.focus == 3 => {
                form.dropdown = Some(form.focus);
                form.dropdown_original = Some(if form.focus == 2 {
                    form.issue_type
                } else {
                    form.priority
                });
            }
            Enter if form.focus == 0 => {
                form.focus = 1;
                form.description_cursor = form.description.len();
            }
            Enter if form.focus == 1 => {
                let i = form.description_cursor.min(form.description.len());
                form.description.insert(i, '\n');
                form.description_cursor = i + 1;
            }
            Up if form.focus == 1 => {
                form.description_cursor =
                    move_vertical(&form.description, form.description_cursor, false)
            }
            Down if form.focus == 1 => {
                form.description_cursor =
                    move_vertical(&form.description, form.description_cursor, true)
            }
            Left if form.focus == 0 => form.cursor = previous_boundary(&form.title, form.cursor),
            Right if form.focus == 0 => form.cursor = next_boundary(&form.title, form.cursor),
            Home if form.focus == 0 => form.cursor = 0,
            End if form.focus == 0 => form.cursor = form.title.len(),
            Left if form.focus == 1 => {
                form.description_cursor =
                    previous_boundary(&form.description, form.description_cursor)
            }
            Right if form.focus == 1 => {
                form.description_cursor = next_boundary(&form.description, form.description_cursor)
            }
            Home if form.focus == 1 => {
                form.description_cursor = form.description
                    [..form.description_cursor.min(form.description.len())]
                    .rfind('\n')
                    .map_or(0, |i| i + 1)
            }
            End if form.focus == 1 => {
                form.description_cursor = form.description
                    [form.description_cursor.min(form.description.len())..]
                    .find('\n')
                    .map_or(form.description.len(), |i| form.description_cursor + i)
            }
            Backspace if form.focus == 1 && form.description_cursor > 0 => {
                let p = previous_boundary(&form.description, form.description_cursor);
                form.description.drain(p..form.description_cursor);
                form.description_cursor = p;
            }
            Delete if form.focus == 1 && form.description_cursor < form.description.len() => {
                let n = next_boundary(&form.description, form.description_cursor);
                form.description.drain(form.description_cursor..n);
            }
            Backspace if form.focus == 0 && form.cursor > 0 => {
                let p = previous_boundary(&form.title, form.cursor);
                form.title.drain(p..form.cursor);
                form.cursor = p;
            }
            Delete if form.focus == 0 && form.cursor < form.title.len() => {
                let n = next_boundary(&form.title, form.cursor);
                form.title.drain(form.cursor..n);
            }
            Char(c)
                if !key.modifiers.intersects(
                    crossterm::event::KeyModifiers::CONTROL
                        | crossterm::event::KeyModifiers::ALT
                        | crossterm::event::KeyModifiers::SUPER,
                ) && form.focus == 0 =>
            {
                form.title.insert(form.cursor, c);
                form.cursor += c.len_utf8();
            }
            Char(c)
                if form.focus == 1
                    && !key.modifiers.intersects(
                        crossterm::event::KeyModifiers::CONTROL
                            | crossterm::event::KeyModifiers::ALT
                            | crossterm::event::KeyModifiers::SUPER,
                    ) =>
            {
                let i = form.description_cursor.min(form.description.len());
                form.description.insert(i, c);
                form.description_cursor = i + c.len_utf8();
            }
            _ => {}
        }
        false
    }
    fn mouse(&mut self, k: MouseEventKind, x: u16, y: u16) {
        if self.modal.is_some() {
            return;
        }
        match k {
            MouseEventKind::Down(MouseButton::Left)
                if Rect::new(
                    self.list_rect.x.saturating_add(1),
                    self.list_rect.y.saturating_add(1),
                    self.list_rect.width.saturating_sub(2),
                    self.list_rect.height.saturating_sub(2),
                )
                .contains((x, y).into()) =>
            {
                let inner_y = y - self.list_rect.y - 1;
                let inner_height = self.list_rect.height.saturating_sub(2);
                if inner_y >= inner_height || inner_y / 2 >= inner_height / 2 {
                    return;
                }
                let i = self.list_offset + (inner_y / 2) as usize;
                if i < self.beads.len() {
                    self.select(i);
                    self.focus_detail = false
                }
            }
            MouseEventKind::ScrollUp if self.list_rect.contains((x, y).into()) => self.step(-1),
            MouseEventKind::ScrollDown if self.list_rect.contains((x, y).into()) => self.step(1),
            MouseEventKind::ScrollUp if self.detail_rect.contains((x, y).into()) => {
                self.detail_offset = self.detail_offset.saturating_sub(3)
            }
            MouseEventKind::ScrollDown if self.detail_rect.contains((x, y).into()) => {
                self.detail_offset = self.detail_offset.saturating_add(3)
            }
            _ => {}
        }
    }
}
pub fn run(t: &mut Tui) -> std::io::Result<()> {
    let start = Instant::now();
    let mut splash = ui::new_splash();
    let mut app = App::new();
    loop {
        app.receive();
        t.draw(|f| ui::render(f, start.elapsed(), &mut splash, &mut app))?;
        if let Some(e) = event::poll_event(Duration::from_millis(35))? {
            match e {
                crossterm::event::Event::Key(k)
                    if matches!(
                        k.kind,
                        crossterm::event::KeyEventKind::Press
                            | crossterm::event::KeyEventKind::Repeat
                    ) =>
                {
                    if app.key(k) {
                        break;
                    }
                }
                crossterm::event::Event::Mouse(m) => app.mouse(m.kind, m.column, m.row),
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use serde_json::json;
    fn app() -> App {
        let (tx, _rx) = mpsc::channel();
        let (_out, responses) = mpsc::channel();
        App {
            beads: (0..4)
                .map(|i| Bead {
                    id: format!("b{i}"),
                    title: format!("issue {i}"),
                    status: "open".into(),
                    priority: 2,
                    data: json!({}),
                })
                .collect(),
            selected: Some(0),
            detail: Some(json!({})),
            focus_detail: false,
            list_offset: 0,
            detail_offset: 0,
            list_loading: false,
            detail_loading: false,
            list_error: None,
            detail_error: None,
            detail_seq: 0,
            list_rect: Rect::new(0, 0, 30, 8),
            detail_rect: Rect::new(30, 0, 50, 8),
            modal: None,
            notice: None,
            mutation_busy: false,
            modal_ready: true,
            preferred_id: None,
            requests: tx,
            responses,
        }
    }
    #[test]
    fn navigation_clamps_and_empty_is_safe() {
        let mut a = app();
        a.step(-1);
        assert_eq!(a.selected, Some(0));
        a.step(20);
        assert_eq!(a.selected, Some(3));
        a.beads.clear();
        a.step(1);
        assert_eq!(a.selected, None);
    }
    #[test]
    fn detail_focus_and_scroll_work() {
        let mut a = app();
        assert!(!a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)));
        assert!(a.focus_detail);
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(a.detail_offset, 1);
        a.key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(a.detail_offset, 7);
    }
    #[test]
    fn mouse_row_includes_list_offset_and_two_line_items() {
        let mut a = app();
        a.list_offset = 1;
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 5);
        assert_eq!(a.selected, Some(3));
    }
    #[test]
    fn stale_detail_response_cannot_replace_selection() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.responses = rx;
        let _ = tx.send(Response::Show("b0".into(), 0, Ok(json!({"old":true}))));
        a.selected = Some(1);
        a.receive();
        assert_eq!(a.detail, Some(json!({})));
    }
    #[test]
    fn stale_response_for_same_id_after_reselection_is_ignored() {
        let mut a = app();
        a.select(1);
        let old_seq = a.detail_seq;
        a.select(0);
        a.select(1);
        let (tx, rx) = mpsc::channel();
        a.responses = rx;
        let _ = tx.send(Response::Show(
            "b1".into(),
            old_seq,
            Ok(json!({"stale":true})),
        ));
        a.receive();
        assert_eq!(a.detail, Some(a.beads[1].data.clone()));
        assert!(a.detail_loading);
    }
    #[test]
    fn click_ignores_border_and_blank_rows() {
        let mut a = app();
        a.list_rect = Rect::new(0, 0, 30, 7);
        a.selected = Some(3);
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 0);
        a.mouse(MouseEventKind::Down(MouseButton::Left), 0, 2);
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 7);
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 5);
        assert_eq!(a.selected, Some(3));
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 1);
        assert_eq!(a.selected, Some(0));
        a.selected = Some(0);
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 7);
        assert_eq!(a.selected, Some(0));
    }

    #[test]
    fn successful_refresh_preserves_selection_and_clears_empty_details() {
        let mut a = app();
        a.selected = Some(2);
        a.detail_offset = 20;
        let (tx, rx) = mpsc::channel();
        a.responses = rx;
        tx.send(Response::List(Ok(vec![
            a.beads[2].clone(),
            a.beads[0].clone(),
        ])))
        .unwrap();
        a.receive();
        assert_eq!(a.selected, Some(0));
        assert_eq!(a.beads[0].id, "b2");
        assert_eq!(a.detail_offset, 0);

        a.detail_error = Some("old error".into());
        a.detail_loading = true;
        tx.send(Response::List(Ok(vec![]))).unwrap();
        a.receive();
        assert_eq!(a.selected, None);
        assert!(a.detail.is_none());
        assert!(!a.detail_loading);
        assert!(a.detail_error.is_none());
    }
    #[test]
    fn selecting_uses_cache_and_clears_selection_error() {
        let mut a = app();
        a.detail_error = Some("old error".into());
        a.select(1);
        assert_eq!(a.detail, Some(a.beads[1].data.clone()));
        assert!(a.detail_loading);
        assert!(a.detail_error.is_none());
    }
    #[test]
    fn create_form_defaults_text_and_quit_keys_do_not_leak() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!("form not opened")
        };
        assert_eq!((form.issue_type, form.priority), (0, 2));
        for c in ['q', 'r', 'n', 'd', '界'] {
            a.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!("form closed")
        };
        assert_eq!(form.title, "qrnd界");
        assert!(!a.key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)));
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 1);
        assert_eq!(a.selected, Some(0));
    }
    #[test]
    fn description_accepts_shortcut_letters_and_newlines_but_not_control_chars() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        press(&mut a, KeyCode::Tab);
        for c in ['q', 'r', 'n', 'd'] {
            press(&mut a, KeyCode::Char(c));
        }
        press(&mut a, KeyCode::Enter);
        press(&mut a, KeyCode::Char('界'));
        a.modal_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.description, "qrnd\n界");
    }
    #[test]
    fn dropdown_escape_closes_choice_before_form_and_delete_captures_selection() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Enter);
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Esc);
        assert!(matches!(a.modal, Some(Modal::Create(_))));
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.issue_type, 0);
        press(&mut a, KeyCode::Esc);
        press(&mut a, KeyCode::Char('d'));
        assert_eq!(
            a.modal,
            Some(Modal::ConfirmDelete {
                id: "b0".into(),
                title: "issue 0".into()
            })
        );
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(a.modal.is_none());
    }
    fn press(a: &mut App, code: KeyCode) {
        a.key(KeyEvent::new(code, KeyModifiers::NONE));
        // Test actions model the app's next render before the next input event.
        if a.modal.is_some() {
            a.modal_ready = true;
        }
    }
    #[test]
    fn create_submits_payload_once_and_locks_until_error_then_preserves_form() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        press(&mut a, KeyCode::Char('n'));
        for c in "q/rn界".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        for _ in 0..4 {
            press(&mut a, KeyCode::Tab);
        }
        press(&mut a, KeyCode::Enter);
        let Ok(Request::Create(input)) = rx.try_recv() else {
            panic!("missing create")
        };
        assert_eq!(input.title, "q/rn界");
        assert_eq!(input.description, "");
        assert_eq!(input.issue_type, "task");
        assert_eq!(input.priority, 2);
        assert!(a.mutation_busy);
        press(&mut a, KeyCode::Esc);
        press(&mut a, KeyCode::Char('n'));
        press(&mut a, KeyCode::Enter);
        assert!(matches!(a.modal, Some(Modal::Create(_))));
        assert!(rx.try_recv().is_err());
        let (out, responses) = mpsc::channel();
        a.responses = responses;
        out.send(Response::Created(Err(std::io::Error::other("offline"))))
            .unwrap();
        a.receive();
        assert!(!a.mutation_busy);
        assert!(matches!(a.modal, Some(Modal::Create(_))));
        assert!(a.notice.as_deref().unwrap().contains("offline"));
    }
    #[test]
    fn empty_title_reports_error_and_delete_uses_captured_id_once() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        press(&mut a, KeyCode::Char('n'));
        for _ in 0..4 {
            press(&mut a, KeyCode::Tab);
        }
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.notice.as_deref(), Some("Title is required"));
        assert!(rx.try_recv().is_err());
        a.modal = None;
        press(&mut a, KeyCode::Char('d'));
        a.selected = Some(2);
        assert_eq!(
            a.modal,
            Some(Modal::ConfirmDelete {
                id: "b0".into(),
                title: "issue 0".into()
            })
        );
        press(&mut a, KeyCode::Char('y'));
        assert!(matches!(rx.try_recv(),Ok(Request::Delete(id)) if id=="b0"));
        press(&mut a, KeyCode::Char('y'));
        assert!(rx.try_recv().is_err());
        assert!(a.mutation_busy);
    }
    #[test]
    fn unreadable_delete_dialog_cannot_be_confirmed() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.modal = Some(Modal::ConfirmDelete {
            id: "b0".into(),
            title: "issue 0".into(),
        });
        a.modal_ready = false;
        press(&mut a, KeyCode::Char('y'));
        assert!(rx.try_recv().is_err());
        assert!(a.modal.is_some());
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none());
    }
    #[test]
    fn newly_opened_delete_is_unconfirmable_until_first_render() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert!(!a.modal_ready);
        a.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn unicode_cursor_and_multiline_vertical_movement_stay_on_boundaries() {
        let s = "界a\nxy".to_string();
        let mut cursor = "界".len();
        assert_eq!(previous_boundary(&s, cursor), 0);
        cursor = next_boundary(&s, 0);
        assert_eq!(cursor, 3);
        let down = move_vertical(&s, 3, true);
        assert_eq!(&s[down..down + 1], "y");
        let mut a = app();
        a.modal = Some(Modal::Create(CreateForm {
            title: s.clone(),
            cursor,
            focus: 0,
            ..Default::default()
        }));
        press(&mut a, KeyCode::Backspace);
        let Some(Modal::Create(form)) = a.modal else {
            panic!()
        };
        assert_eq!(form.title, "a\nxy");
    }
    #[test]
    fn create_success_refreshes_and_selects_created_id() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        let (out, responses) = mpsc::channel();
        a.responses = responses;
        out.send(Response::Created(Ok("new-id".into()))).unwrap();
        a.receive();
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        out.send(Response::List(Ok(vec![Bead {
            id: "new-id".into(),
            title: "fresh".into(),
            status: "open".into(),
            priority: 2,
            data: json!({}),
        }])))
        .unwrap();
        a.receive();
        assert_eq!(a.selected, Some(0));
        assert_eq!(a.beads[0].id, "new-id");
        match rx.try_recv() {
            Ok(Request::Show(id, _)) => assert_eq!(id, "new-id"),
            _ => panic!("missing show follow-up"),
        }
    }
    #[test]
    fn dropdown_confirm_commits_previewed_option() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        for _ in 0..2 {
            press(&mut a, KeyCode::Tab);
        }
        press(&mut a, KeyCode::Enter);
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Enter);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.issue_type, 1);
        assert!(form.dropdown.is_none());
    }
}
