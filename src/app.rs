use crate::{
    Tui,
    beads::{self, Bead, CreateBead},
    event, ui,
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use std::{
    collections::HashSet,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

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
    pub collapsed: HashSet<String>,
    pub focused_group: Option<String>,
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
    pub(super) refresh_inflight: bool,
    pub(super) refresh_pending: bool,
    pub(super) next_refresh: Instant,
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
            collapsed: HashSet::new(),
            focused_group: None,
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
            refresh_inflight: false,
            refresh_pending: false,
            next_refresh: Instant::now(),
            requests: tx,
            responses,
        };
        a.refresh();
        a
    }
    fn refresh(&mut self) {
        self.refresh_at(Instant::now());
    }
    fn refresh_at(&mut self, now: Instant) {
        if self.refresh_inflight {
            // Explicit refreshes and mutation invalidations need one newer snapshot.
            self.refresh_pending = true;
        } else {
            self.start_refresh(now);
        }
    }
    fn poll_refresh(&mut self, now: Instant) {
        if !self.refresh_inflight && now >= self.next_refresh {
            self.start_refresh(now);
        }
    }
    fn start_refresh(&mut self, now: Instant) {
        self.refresh_inflight = true;
        if self.requests.send(Request::List).is_err() {
            self.refresh_inflight = false;
            self.list_loading = false;
            self.list_error = Some("Refresh could not be queued; retrying".into());
            self.next_refresh = now + REFRESH_INTERVAL;
        }
    }
    fn refresh_detail(&mut self) {
        let Some(bead) = self.selected.and_then(|i| self.beads.get(i)) else {
            return;
        };
        // Every snapshot invalidates older shows, even when the list is unchanged:
        // comments and other show-only fields can change independently.
        self.detail_seq = self.detail_seq.wrapping_add(1);
        if self
            .requests
            .send(Request::Show(bead.id.clone(), self.detail_seq))
            .is_err()
        {
            self.detail_loading = false;
            self.detail_error = Some("Details could not be queued; retrying".into());
        }
    }
    fn select(&mut self, index: usize) {
        self.focused_group = None;
        if self.beads.is_empty() {
            self.clear_selection();
            return;
        }
        let index = index.min(self.beads.len() - 1);
        let status = normalized_status(&self.beads[index].status).to_owned();
        if self.collapsed.contains(&status) {
            self.focus_group(status);
            return;
        }
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
    fn clear_selection(&mut self) {
        self.selected = None;
        self.detail = None;
        self.detail_loading = false;
        self.detail_error = None;
        self.detail_offset = 0;
    }
    fn focus_group(&mut self, status: String) {
        self.clear_selection();
        self.focused_group = Some(status);
    }
    fn step(&mut self, d: isize) {
        let targets = list_layout(&self.beads, &self.collapsed).targets;
        if targets.is_empty() {
            self.clear_selection();
            self.focused_group = None;
            return;
        }
        let current = self
            .focused_group
            .as_ref()
            .and_then(|s| {
                targets
                    .iter()
                    .position(|t| matches!(&t.row, DisplayRow::Group(g) if g == s))
            })
            .or_else(|| {
                self.selected
                    .and_then(|i| targets.iter().position(|t| t.row == DisplayRow::Bead(i)))
            })
            .unwrap_or(0);
        let next = current.saturating_add_signed(d).min(targets.len() - 1);
        match &targets[next].row {
            DisplayRow::Group(status) => self.focus_group(status.clone()),
            DisplayRow::Bead(i) => self.select(*i),
        }
    }
    fn toggle_selected_group(&mut self) {
        if let Some(status) = self.focused_status() {
            self.toggle_group(status);
        }
    }
    fn focused_status(&self) -> Option<String> {
        self.focused_group.clone().or_else(|| {
            self.selected
                .and_then(|i| self.beads.get(i))
                .map(|b| normalized_status(&b.status).to_owned())
        })
    }
    fn set_group_collapsed(&mut self, status: String, collapsed: bool) {
        if !self
            .beads
            .iter()
            .any(|b| normalized_status(&b.status) == status)
        {
            return;
        }
        let is_collapsed = self.collapsed.contains(&status);
        if is_collapsed == collapsed {
            return;
        }
        if collapsed {
            self.collapsed.insert(status.clone());
            self.focus_group(status);
        } else {
            self.collapsed.remove(&status);
            if let Some(i) = self
                .beads
                .iter()
                .position(|b| normalized_status(&b.status) == status)
            {
                self.select(i);
            } else {
                self.focus_group(status);
            }
        }
    }
    fn toggle_group(&mut self, status: String) {
        if self.collapsed.remove(&status) {
            if let Some(i) = self
                .beads
                .iter()
                .position(|b| normalized_status(&b.status) == status)
            {
                self.select(i);
            } else {
                self.focus_group(status);
            }
        } else {
            self.collapsed.insert(status.clone());
            self.focus_group(status);
        }
    }
    fn receive(&mut self) {
        self.receive_at(Instant::now());
    }
    fn receive_at(&mut self, now: Instant) {
        while let Ok(r) = self.responses.try_recv() {
            match r {
                Response::List(res) => {
                    self.refresh_inflight = false;
                    self.next_refresh = now + REFRESH_INTERVAL;
                    self.list_loading = false;
                    match res {
                        Ok(items) => {
                            let id = self
                                .selected
                                .and_then(|i| self.beads.get(i))
                                .map(|b| b.id.clone());
                            let group = self.focused_group.clone();
                            self.beads = items
                                .into_iter()
                                .map(|mut b| {
                                    b.status = normalized_status(&b.status).to_owned();
                                    b
                                })
                                .collect();
                            self.beads.sort_by(|a, b| {
                                status_order(&a.status)
                                    .cmp(&status_order(&b.status))
                                    .then_with(|| a.status.cmp(&b.status))
                                    .then_with(|| a.id.cmp(&b.id))
                            });
                            self.collapsed
                                .retain(|s| self.beads.iter().any(|b| &b.status == s));
                            // An in-flight pre-mutation snapshot must not consume
                            // the new ID before the trailing refresh can see it.
                            let preferred = if self.refresh_pending {
                                None
                            } else {
                                self.preferred_id
                                    .take()
                                    .and_then(|id| self.beads.iter().position(|b| b.id == id))
                            };
                            let same_id = id
                                .as_ref()
                                .and_then(|id| self.beads.iter().position(|b| &b.id == id));
                            // Indices may now refer to different beads. Only the
                            // surviving ID can reuse the existing detail and scroll.
                            self.selected = None;
                            if let Some(i) = preferred {
                                self.focused_group = None;
                                self.collapsed.remove(&self.beads[i].status);
                                if Some(i) == same_id {
                                    self.selected = Some(i);
                                    self.refresh_detail();
                                } else {
                                    self.select(i);
                                }
                            } else if let Some(status) =
                                group.filter(|s| self.beads.iter().any(|b| &b.status == s))
                            {
                                self.focus_group(status);
                            } else if let Some(i) = same_id {
                                self.focused_group = None;
                                if self.collapsed.contains(&self.beads[i].status) {
                                    self.focus_group(self.beads[i].status.clone());
                                } else {
                                    self.selected = Some(i);
                                    self.refresh_detail();
                                }
                            } else {
                                self.clear_selection();
                                self.focused_group = None;
                                let targets = list_layout(&self.beads, &self.collapsed).targets;
                                if let Some(i) = targets.iter().find_map(|t| match t.row {
                                    DisplayRow::Bead(i) => Some(i),
                                    DisplayRow::Group(_) => None,
                                }) {
                                    self.select(i);
                                } else if let Some(ListTarget {
                                    row: DisplayRow::Group(status),
                                    ..
                                }) = targets.into_iter().next()
                                {
                                    self.focus_group(status);
                                }
                            }
                            self.list_error = None;
                        }
                        Err(e) => self.list_error = Some(e.to_string()),
                    }
                    if std::mem::take(&mut self.refresh_pending) {
                        self.start_refresh(now);
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
                            self.refresh_at(now);
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
                            self.refresh_at(now);
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
            KeyCode::Char(' ') => self.toggle_selected_group(),
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
            KeyCode::Left if !self.focus_detail => {
                if let Some(status) = self.focused_status() {
                    self.set_group_collapsed(status, true);
                }
            }
            KeyCode::Right if !self.focus_detail => {
                if let Some(status) = self.focused_status() {
                    self.set_group_collapsed(status, false);
                }
            }
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
            Tab => {
                form.focus = match form.focus {
                    0 => 2,
                    2 => 3,
                    3 => 1,
                    1 => 4,
                    4 => 5,
                    _ => 0,
                }
            }
            BackTab => {
                form.focus = match form.focus {
                    0 => 5,
                    5 => 4,
                    4 => 1,
                    1 => 3,
                    3 => 2,
                    _ => 0,
                }
            }
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
                if self.list_rect.contains((x, y).into())
                    && x > self.list_rect.x
                    && x < self.list_rect.x + self.list_rect.width.saturating_sub(1) =>
            {
                let row = usize::from(y - self.list_rect.y) + self.list_offset;
                if let Some(target) = list_layout(&self.beads, &self.collapsed)
                    .targets
                    .into_iter()
                    .find(|t| row >= t.y && row < t.y + t.height)
                {
                    match target.row {
                        DisplayRow::Group(status) => self.toggle_group(status),
                        DisplayRow::Bead(i) => self.select(i),
                    }
                    self.focus_detail = false;
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
fn status_order(status: &str) -> (u8, &str) {
    match status {
        "open" => (0, status),
        "closed" => (2, status),
        _ => (1, status),
    }
}
fn normalized_status(status: &str) -> &str {
    match status.trim() {
        "" => "unknown",
        status => status,
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayRow {
    Group(String),
    Bead(usize),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListLayout {
    pub groups: Vec<GroupLayout>,
    pub targets: Vec<ListTarget>,
    pub height: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupLayout {
    pub status: String,
    pub indices: Vec<usize>,
    pub y: usize,
    pub height: usize,
    pub collapsed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListTarget {
    pub row: DisplayRow,
    pub y: usize,
    pub height: usize,
}
pub fn group_statuses(beads: &[Bead]) -> Vec<String> {
    let mut groups = beads
        .iter()
        .map(|b| normalized_status(&b.status).to_owned())
        .collect::<Vec<_>>();
    groups.sort_by(|a, b| status_order(a).cmp(&status_order(b)).then_with(|| a.cmp(b)));
    groups.dedup();
    groups
}
pub fn list_layout(beads: &[Bead], collapsed: &HashSet<String>) -> ListLayout {
    let mut layout = ListLayout {
        groups: Vec::new(),
        targets: Vec::new(),
        height: 0,
    };
    for status in group_statuses(beads) {
        let indices = beads
            .iter()
            .enumerate()
            .filter(|(_, b)| normalized_status(&b.status) == status)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        let is_collapsed = collapsed.contains(&status);
        let y = layout.height;
        let height = if is_collapsed {
            2
        } else {
            2 + 2 * indices.len()
        };
        layout.targets.push(ListTarget {
            row: DisplayRow::Group(status.clone()),
            y,
            height: 1,
        });
        if !is_collapsed {
            layout
                .targets
                .extend(indices.iter().enumerate().map(|(n, &i)| ListTarget {
                    row: DisplayRow::Bead(i),
                    y: y + 1 + 2 * n,
                    height: 2,
                }));
        }
        layout.groups.push(GroupLayout {
            status,
            indices,
            y,
            height,
            collapsed: is_collapsed,
        });
        layout.height += height;
    }
    layout
}
pub fn run(t: &mut Tui) -> std::io::Result<()> {
    let start = Instant::now();
    let mut app = App::new();
    loop {
        app.receive();
        app.poll_refresh(Instant::now());
        t.draw(|f| ui::render(f, start.elapsed(), &mut app))?;
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
                    data: json!({"id": format!("b{i}"), "description": format!("detail {i}")}),
                })
                .collect(),
            collapsed: HashSet::new(),
            focused_group: None,
            selected: Some(0),
            detail: Some(json!({"id": "b0", "description": "detail 0"})),
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
            refresh_inflight: false,
            refresh_pending: false,
            next_refresh: Instant::now(),
            requests: tx,
            responses,
        }
    }
    fn assert_header(a: &App, status: &str) {
        assert_eq!(a.focused_group.as_deref(), Some(status));
        assert!(a.selected.is_none());
        assert!(a.detail.is_none());
        assert!(!a.detail_loading);
        assert!(a.detail_error.is_none());
        assert_eq!(a.detail_offset, 0);
    }
    fn assert_bead(a: &App, id: &str) {
        assert!(a.focused_group.is_none());
        let bead = &a.beads[a.selected.expect("selected bead")];
        assert_eq!(bead.id, id);
        assert_eq!(a.detail.as_ref(), Some(&bead.data));
    }
    fn respond(a: &mut App, response: Response) {
        let (tx, rx) = mpsc::channel();
        a.responses = rx;
        tx.send(response).unwrap();
        a.receive();
    }
    fn connected_app() -> (App, mpsc::Receiver<Request>, mpsc::Sender<Response>) {
        let mut a = app();
        let (requests, rx) = mpsc::channel();
        let (tx, responses) = mpsc::channel();
        a.requests = requests;
        a.responses = responses;
        (a, rx, tx)
    }
    fn take_show(rx: &mpsc::Receiver<Request>, expected_id: &str) -> u64 {
        match rx.try_recv() {
            Ok(Request::Show(id, seq)) => {
                assert_eq!(id, expected_id);
                seq
            }
            _ => panic!("missing show for {expected_id}"),
        }
    }
    #[test]
    fn startup_and_interval_polling_wait_for_completion_without_queue_growth() {
        let (mut a, rx, tx) = connected_app();
        a.beads.clear();
        a.clear_selection();
        a.list_loading = true;
        let now = Instant::now();
        a.refresh_at(now);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        assert!(a.list_loading);
        for elapsed in [0, 1, 10, 100] {
            a.poll_refresh(now + Duration::from_secs(elapsed));
        }
        assert!(rx.try_recv().is_err());
        assert!(!a.refresh_pending);
        let completed = now + Duration::from_secs(100);
        tx.send(Response::List(Ok(vec![]))).unwrap();
        a.receive_at(completed);
        assert!(!a.list_loading);
        assert!(!a.refresh_inflight);
        a.poll_refresh(completed + REFRESH_INTERVAL - Duration::from_nanos(1));
        assert!(rx.try_recv().is_err());
        a.poll_refresh(completed + REFRESH_INTERVAL);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        assert!(!a.list_loading, "empty background polls must not flicker");
        a.poll_refresh(completed + REFRESH_INTERVAL * 10);
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn repeated_manual_refreshes_coalesce_into_one_trailing_snapshot() {
        let (mut a, rx, tx) = connected_app();
        let now = Instant::now();
        a.refresh_at(now);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        for _ in 0..20 {
            press(&mut a, KeyCode::Char('r'));
        }
        assert!(rx.try_recv().is_err());
        assert!(a.refresh_pending);
        tx.send(Response::List(Ok(a.beads.clone()))).unwrap();
        a.receive_at(now);
        take_show(&rx, "b0");
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        assert!(!a.refresh_pending);
        a.poll_refresh(now + REFRESH_INTERVAL * 20);
        assert!(rx.try_recv().is_err());
        tx.send(Response::List(Ok(a.beads.clone()))).unwrap();
        a.receive_at(now + REFRESH_INTERVAL * 20);
        take_show(&rx, "b0");
        assert!(rx.try_recv().is_err());
        assert!(!a.refresh_inflight);
    }
    #[test]
    fn mutations_invalidate_older_inflight_snapshot_and_keep_created_id_for_followup() {
        for create in [true, false] {
            let (mut a, rx, tx) = connected_app();
            let now = Instant::now();
            a.refresh_at(now);
            assert!(matches!(rx.try_recv(), Ok(Request::List)));
            tx.send(if create {
                Response::Created(Ok("new-id".into()))
            } else {
                Response::Deleted(Ok(()))
            })
            .unwrap();
            a.receive_at(now);
            assert!(a.refresh_pending);
            assert!(rx.try_recv().is_err());
            tx.send(Response::List(Ok(a.beads.clone()))).unwrap();
            a.receive_at(now);
            take_show(&rx, "b0");
            assert!(matches!(rx.try_recv(), Ok(Request::List)));
            assert_eq!(a.preferred_id.as_deref(), create.then_some("new-id"));
            let mut items = a.beads.clone();
            if create {
                let mut new = items[0].clone();
                new.id = "new-id".into();
                items.push(new);
            } else {
                items.remove(0);
            }
            tx.send(Response::List(Ok(items))).unwrap();
            a.receive_at(now);
            take_show(&rx, if create { "new-id" } else { "b1" });
            assert!(rx.try_recv().is_err());
            assert!(a.preferred_id.is_none());
        }
    }
    #[test]
    fn unchanged_polls_preserve_detail_scroll_focus_collapsed_groups_and_modals() {
        for modal in [
            None,
            Some(Modal::Create(CreateForm {
                title: "unfinished draft".into(),
                description: "keep this".into(),
                cursor: 3,
                dropdown: Some(2),
                ..Default::default()
            })),
            Some(Modal::ConfirmDelete {
                id: "b0".into(),
                title: "issue 0".into(),
            }),
        ] {
            let (mut a, rx, tx) = connected_app();
            a.beads[3].status = "closed".into();
            a.collapsed.insert("closed".into());
            a.focus_detail = true;
            a.list_offset = 7;
            a.detail_offset = 23;
            a.detail = Some(json!({"id":"b0", "comments":[{"body":"keep me"}]}));
            a.modal = modal.clone();
            a.modal_ready = false;
            a.notice = Some("keep notice".into());
            let detail = a.detail.clone();
            let now = Instant::now();
            a.poll_refresh(now);
            assert!(matches!(rx.try_recv(), Ok(Request::List)));
            assert!(!a.list_loading);
            assert!(!a.detail_loading);
            tx.send(Response::List(Ok(a.beads.clone()))).unwrap();
            a.receive_at(now);
            let seq = take_show(&rx, "b0");
            assert!(!a.detail_loading);
            assert_eq!(a.detail, detail);
            assert_eq!(a.selected, Some(0));
            assert_eq!(a.collapsed, HashSet::from(["closed".into()]));
            assert!(a.focus_detail);
            assert_eq!((a.list_offset, a.detail_offset), (7, 23));
            assert_eq!(a.modal, modal);
            assert!(!a.modal_ready);
            assert_eq!(a.notice.as_deref(), Some("keep notice"));
            // Show-only changes must be observed even though the list was identical.
            let updated = json!({"id":"b0", "comments":[{"body":"new comment"}]});
            tx.send(Response::Show("b0".into(), seq, Ok(updated.clone())))
                .unwrap();
            a.receive_at(now);
            assert_eq!(a.detail, Some(updated));
            assert_eq!((a.list_offset, a.detail_offset), (7, 23));
            assert_eq!(a.modal, modal);
        }
    }
    #[test]
    fn unchanged_poll_preserves_focused_group_and_does_not_request_details() {
        let (mut a, rx, tx) = connected_app();
        a.focus_group("open".into());
        a.collapsed.insert("open".into());
        a.focus_detail = true;
        a.list_offset = 4;
        let now = Instant::now();
        a.poll_refresh(now);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        tx.send(Response::List(Ok(a.beads.clone()))).unwrap();
        a.receive_at(now);
        assert_header(&a, "open");
        assert!(a.collapsed.contains("open"));
        assert!(a.focus_detail);
        assert_eq!(a.list_offset, 4);
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn polling_applies_external_metadata_additions_and_removals_by_id() {
        let (mut a, rx, tx) = connected_app();
        a.select(2);
        take_show(&rx, "b2");
        a.detail_offset = 12;
        a.list_offset = 3;
        let now = Instant::now();
        a.poll_refresh(now);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        let mut items = a.beads.clone();
        items[2].title = "externally renamed".into();
        items[2].priority = 0;
        items[2].status = "blocked".into();
        items[2].data = json!({"id":"b2", "labels":["external"], "assignee":"someone"});
        let mut added = items[0].clone();
        added.id = "added".into();
        items.push(added);
        items.retain(|b| b.id != "b1");
        items.reverse();
        tx.send(Response::List(Ok(items))).unwrap();
        a.receive_at(now);
        let selected = &a.beads[a.selected.unwrap()];
        assert_eq!(selected.id, "b2");
        assert_eq!(selected.title, "externally renamed");
        assert_eq!(selected.priority, 0);
        assert_eq!(selected.status, "blocked");
        assert_eq!(selected.data["assignee"], "someone");
        assert!(a.beads.iter().any(|b| b.id == "added"));
        assert!(!a.beads.iter().any(|b| b.id == "b1"));
        assert_eq!((a.list_offset, a.detail_offset), (3, 12));
        let seq = take_show(&rx, "b2");
        let updated = a.beads[a.selected.unwrap()].data.clone();
        tx.send(Response::Show("b2".into(), seq, Ok(updated.clone())))
            .unwrap();
        a.receive_at(now);
        assert_eq!(a.detail, Some(updated));
        a.poll_refresh(now + REFRESH_INTERVAL);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        let items = a.beads.iter().filter(|b| b.id != "b2").cloned().collect();
        tx.send(Response::List(Ok(items))).unwrap();
        a.receive_at(now + REFRESH_INTERVAL);
        assert_eq!(a.beads[a.selected.unwrap()].id, "added");
        take_show(&rx, "added");
    }
    #[test]
    fn transient_list_and_show_errors_keep_data_and_retry_until_success() {
        let (mut a, rx, tx) = connected_app();
        let now = Instant::now();
        let old_data = a.detail.clone();
        let old_ids = a.beads.iter().map(|b| b.id.clone()).collect::<Vec<_>>();
        a.detail_offset = 9;
        a.poll_refresh(now);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        tx.send(Response::List(Err(std::io::Error::other("list offline"))))
            .unwrap();
        a.receive_at(now);
        assert_eq!(a.list_error.as_deref(), Some("list offline"));
        assert_eq!(a.detail, old_data);
        assert_eq!(
            a.beads.iter().map(|b| b.id.clone()).collect::<Vec<_>>(),
            old_ids
        );
        a.poll_refresh(now + REFRESH_INTERVAL);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        assert_eq!(a.list_error.as_deref(), Some("list offline"));
        tx.send(Response::List(Ok(a.beads.clone()))).unwrap();
        a.receive_at(now + REFRESH_INTERVAL);
        assert!(a.list_error.is_none());
        let seq = take_show(&rx, "b0");
        tx.send(Response::Show(
            "b0".into(),
            seq,
            Err(std::io::Error::other("show offline")),
        ))
        .unwrap();
        a.receive_at(now + REFRESH_INTERVAL);
        assert_eq!(a.detail_error.as_deref(), Some("show offline"));
        assert_eq!(a.detail, old_data);
        assert_eq!(a.detail_offset, 9);
        a.poll_refresh(now + REFRESH_INTERVAL * 2);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        tx.send(Response::List(Ok(a.beads.clone()))).unwrap();
        a.receive_at(now + REFRESH_INTERVAL * 2);
        let seq = take_show(&rx, "b0");
        assert_eq!(a.detail_error.as_deref(), Some("show offline"));
        let recovered = json!({"id":"b0", "comments":["recovered"]});
        tx.send(Response::Show("b0".into(), seq, Ok(recovered.clone())))
            .unwrap();
        a.receive_at(now + REFRESH_INTERVAL * 2);
        assert_eq!(a.detail, Some(recovered));
        assert!(a.detail_error.is_none());
        assert_eq!(a.detail_offset, 9);
    }
    #[test]
    fn newer_snapshots_reject_older_detail_successes_and_errors_for_same_id() {
        let (mut a, rx, tx) = connected_app();
        let now = Instant::now();
        a.poll_refresh(now);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        tx.send(Response::List(Ok(a.beads.clone()))).unwrap();
        a.receive_at(now);
        let old_seq = take_show(&rx, "b0");
        a.poll_refresh(now + REFRESH_INTERVAL);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        let mut items = a.beads.clone();
        items[0].title = "new snapshot".into();
        tx.send(Response::List(Ok(items))).unwrap();
        a.receive_at(now + REFRESH_INTERVAL);
        let new_seq = take_show(&rx, "b0");
        assert_ne!(old_seq, new_seq);
        let current = json!({"id":"b0", "description":"new detail"});
        tx.send(Response::Show("b0".into(), new_seq, Ok(current.clone())))
            .unwrap();
        tx.send(Response::Show(
            "b0".into(),
            old_seq,
            Ok(json!({"stale":true})),
        ))
        .unwrap();
        tx.send(Response::Show(
            "b0".into(),
            old_seq,
            Err(std::io::Error::other("stale error")),
        ))
        .unwrap();
        a.receive_at(now + REFRESH_INTERVAL);
        assert_eq!(a.detail, Some(current));
        assert!(a.detail_error.is_none());
    }
    #[test]
    fn disconnected_request_channel_does_not_latch_refresh_inflight() {
        let mut a = app();
        let now = Instant::now();
        a.refresh_at(now);
        assert!(!a.refresh_inflight);
        assert!(!a.list_loading);
        assert!(a.list_error.is_some());
        let (requests, rx) = mpsc::channel();
        a.requests = requests;
        a.poll_refresh(now + REFRESH_INTERVAL - Duration::from_nanos(1));
        assert!(rx.try_recv().is_err());
        a.poll_refresh(now + REFRESH_INTERVAL);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        assert!(a.refresh_inflight);
    }
    #[test]
    fn layout_has_physical_geometry_and_only_interactive_targets() {
        let mut a = app();
        a.beads[0].status = " closed ".into();
        a.beads[1].status = "blocked".into();
        a.beads[2].status = "open".into();
        a.beads[3].status = " \t ".into();
        let layout = list_layout(&a.beads, &HashSet::from(["blocked".into()]));
        assert_eq!(layout.height, 14);
        assert_eq!(
            layout
                .groups
                .iter()
                .map(|g| (&*g.status, g.y, g.height, g.collapsed))
                .collect::<Vec<_>>(),
            [
                ("open", 0, 4, false),
                ("blocked", 4, 2, true),
                ("unknown", 6, 4, false),
                ("closed", 10, 4, false)
            ]
        );
        assert_eq!(
            layout
                .groups
                .iter()
                .map(|g| g.indices.clone())
                .collect::<Vec<_>>(),
            [vec![2], vec![1], vec![3], vec![0]]
        );
        assert_eq!(
            layout.targets,
            vec![
                ListTarget {
                    row: DisplayRow::Group("open".into()),
                    y: 0,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Bead(2),
                    y: 1,
                    height: 2
                },
                ListTarget {
                    row: DisplayRow::Group("blocked".into()),
                    y: 4,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Group("unknown".into()),
                    y: 6,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Bead(3),
                    y: 7,
                    height: 2
                },
                ListTarget {
                    row: DisplayRow::Group("closed".into()),
                    y: 10,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Bead(0),
                    y: 11,
                    height: 2
                },
            ]
        );
        assert_eq!(
            list_layout(&[], &HashSet::new()),
            ListLayout {
                groups: vec![],
                targets: vec![],
                height: 0
            }
        );
        a.beads[1].status = "unknown".into();
        assert_eq!(
            list_layout(&a.beads, &HashSet::new()).groups[1].indices,
            [1, 3]
        );
    }
    #[test]
    fn mouse_cross_group_toggle_clears_detail_and_delete_identity() {
        let mut a = app();
        a.list_rect = Rect::new(5, 7, 30, 30);
        a.beads[1].status = "closed".into();
        a.select(2);
        a.detail_error = Some("stale".into());
        a.detail_offset = 8;
        let layout = list_layout(&a.beads, &a.collapsed);
        let closed_y = layout
            .groups
            .iter()
            .find(|g| g.status == "closed")
            .unwrap()
            .y;
        a.mouse(
            MouseEventKind::Down(MouseButton::Left),
            6,
            7 + closed_y as u16,
        );
        assert_header(&a, "closed");
        press(&mut a, KeyCode::Char('d'));
        assert!(a.modal.is_none());
        // Reopening uses the same toggle path as Space, not the old open selection.
        a.mouse(
            MouseEventKind::Down(MouseButton::Left),
            6,
            7 + closed_y as u16,
        );
        assert_bead(&a, "b1");
        press(&mut a, KeyCode::Char('d'));
        assert!(matches!(&a.modal, Some(Modal::ConfirmDelete { id, .. }) if id == "b1"));
        press(&mut a, KeyCode::Esc);
        // Selecting the same index must still remove conflicting header focus.
        a.focused_group = Some("open".into());
        a.select(1);
        assert_bead(&a, "b1");
    }
    #[test]
    fn mouse_targets_follow_scrolled_natural_lines_without_relocated_headers() {
        let mut a = app();
        a.list_rect = Rect::new(5, 7, 30, 20);
        a.beads[1].status = "closed".into();
        a.select(2);
        let layout = list_layout(&a.beads, &a.collapsed);
        for target in layout
            .targets
            .iter()
            .filter(|t| matches!(t.row, DisplayRow::Bead(_)))
        {
            let DisplayRow::Bead(i) = target.row else {
                unreachable!()
            };
            for line in 0..target.height {
                a.list_offset = target.y + line;
                a.mouse(MouseEventKind::Down(MouseButton::Left), 6, 7);
                assert_bead(&a, &a.beads[i].id);
            }
        }
        a.list_offset = 1;
        let closed_bead = layout
            .targets
            .iter()
            .find(|t| t.row == DisplayRow::Bead(1))
            .unwrap();
        let screen_y = a.list_rect.y + (closed_bead.y - a.list_offset) as u16;
        a.mouse(MouseEventKind::Down(MouseButton::Left), 6, screen_y);
        assert_bead(&a, "b1");
        assert!(a.collapsed.is_empty());
        let closed = layout.groups.iter().find(|g| g.status == "closed").unwrap();
        a.list_offset = closed.y;
        a.mouse(MouseEventKind::Down(MouseButton::Left), 6, 7);
        assert_header(&a, "closed");
    }
    #[test]
    fn collapse_and_reselect_reject_stale_detail_success_and_failure() {
        let mut a = app();
        a.select(1);
        let old_seq = a.detail_seq;
        press(&mut a, KeyCode::Char(' '));
        respond(
            &mut a,
            Response::Show("b1".into(), old_seq, Ok(json!({"stale": true}))),
        );
        assert_header(&a, "open");
        press(&mut a, KeyCode::Char(' '));
        a.select(1);
        assert_ne!(a.detail_seq, old_seq);
        respond(
            &mut a,
            Response::Show("b1".into(), old_seq, Err(std::io::Error::other("stale"))),
        );
        assert_bead(&a, "b1");
        assert!(a.detail_loading);
        assert!(a.detail_error.is_none());
        respond(
            &mut a,
            Response::Show("b1".into(), old_seq, Ok(json!({"stale": true}))),
        );
        assert_bead(&a, "b1");
        let seq = a.detail_seq;
        respond(
            &mut a,
            Response::Show("b1".into(), seq, Ok(json!({"fresh": "b1"}))),
        );
        assert_eq!(a.detail, Some(json!({"fresh": "b1"})));
        assert!(!a.detail_loading);
    }
    #[test]
    fn navigation_clamps_and_empty_is_safe() {
        let mut a = app();
        a.step(-1);
        assert_eq!(a.focused_group.as_deref(), Some("open"));
        a.step(20);
        assert_eq!(a.selected, Some(3));
        a.beads.clear();
        a.step(1);
        assert_eq!(a.selected, None);
    }
    #[test]
    fn status_groups_sort_open_middle_closed_and_unknown() {
        let mut a = app();
        a.beads[0].status = "closed".into();
        a.beads[1].status = "blocked".into();
        a.beads[2].status = "open".into();
        a.beads[3].status = "".into();
        assert_eq!(
            group_statuses(&a.beads),
            ["open", "blocked", "unknown", "closed"]
        );
    }
    #[test]
    fn every_collapsed_group_can_be_reopened_and_selection_detail_stay_aligned() {
        let mut a = app();
        a.beads[0].status = "open".into();
        a.beads[1].status = "closed".into();
        a.beads[2].status = "closed".into();
        a.beads[3].status = "open".into();
        press(&mut a, KeyCode::Char(' '));
        assert_header(&a, "open");
        press(&mut a, KeyCode::Down);
        assert_header(&a, "closed");
        press(&mut a, KeyCode::Char(' '));
        assert_eq!(a.collapsed.len(), 2);
        press(&mut a, KeyCode::Up);
        assert_header(&a, "open");
        press(&mut a, KeyCode::Char(' '));
        assert!(!a.collapsed.contains("open"));
        assert_bead(&a, "b0");
        press(&mut a, KeyCode::Down);
        assert_bead(&a, "b3");
        press(&mut a, KeyCode::Down);
        assert_header(&a, "closed");
        press(&mut a, KeyCode::Char(' '));
        assert_bead(&a, "b1");
        assert!(a.collapsed.is_empty());
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
    fn left_right_collapse_expand_idempotently_without_losing_live_selection() {
        let mut a = app();
        a.select(2);
        press(&mut a, KeyCode::Left);
        assert!(a.collapsed.contains("open"));
        assert_header(&a, "open");
        press(&mut a, KeyCode::Left);
        assert_header(&a, "open");
        press(&mut a, KeyCode::Right);
        assert!(!a.collapsed.contains("open"));
        assert_bead(&a, "b0");
        a.select(2);
        let detail = a.detail.clone();
        press(&mut a, KeyCode::Right);
        assert_bead(&a, "b2");
        assert_eq!(a.detail, detail);
        assert!(detail.is_some());
    }
    #[test]
    fn details_focus_keeps_arrows_out_of_group_state_and_tab_alone_switches_panes() {
        let mut a = app();
        let selected = a.selected;
        let detail = a.detail.clone();
        press(&mut a, KeyCode::Tab);
        assert!(a.focus_detail);
        press(&mut a, KeyCode::Left);
        press(&mut a, KeyCode::Right);
        assert!(a.focus_detail);
        assert!(a.collapsed.is_empty());
        assert_eq!(a.selected, selected);
        assert_eq!(a.detail, detail);
        press(&mut a, KeyCode::Tab);
        assert!(!a.focus_detail);
        press(&mut a, KeyCode::Tab);
        assert!(a.focus_detail);
    }
    #[test]
    fn list_arrows_are_safe_when_empty_and_modal_arrows_remain_form_controls() {
        let mut a = app();
        a.beads.clear();
        a.clear_selection();
        press(&mut a, KeyCode::Left);
        press(&mut a, KeyCode::Right);
        assert!(a.selected.is_none());
        assert!(a.collapsed.is_empty());
        a.beads = app().beads;
        a.modal = Some(Modal::Create(CreateForm {
            title: "xy".into(),
            cursor: 1,
            ..Default::default()
        }));
        press(&mut a, KeyCode::Right);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!("form closed")
        };
        assert_eq!(form.cursor, 2);
        press(&mut a, KeyCode::Left);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!("form closed")
        };
        assert_eq!(form.cursor, 1);
        assert!(!a.focus_detail);
        assert!(a.collapsed.is_empty());
    }
    #[test]
    fn mouse_row_includes_list_offset_and_two_line_items() {
        let mut a = app();
        a.list_offset = 1;
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 3);
        assert_eq!(a.selected, Some(1));
    }
    #[test]
    fn stale_detail_response_cannot_replace_selection() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.responses = rx;
        let _ = tx.send(Response::Show("b0".into(), 0, Ok(json!({"old":true}))));
        a.select(1);
        a.receive();
        assert_eq!(a.detail, Some(a.beads[1].data.clone()));
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
        a.list_rect = Rect::new(0, 0, 30, 16);
        a.select(3);
        for (x, y) in [(0, 0), (0, 3), (29, 3), (4, 9), (4, 10), (4, 15), (4, 16)] {
            a.mouse(MouseEventKind::Down(MouseButton::Left), x, y);
            assert_bead(&a, "b3");
            assert!(a.collapsed.is_empty());
        }
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 3);
        assert_bead(&a, "b1");
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 4);
        assert_bead(&a, "b1");
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 0);
        assert!(a.collapsed.contains("open"));
        assert_header(&a, "open");
        for y in 1..16 {
            a.mouse(MouseEventKind::Down(MouseButton::Left), 4, y);
            assert_header(&a, "open");
            assert!(a.collapsed.contains("open"));
        }
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 0);
        assert_bead(&a, "b0");
    }

    #[test]
    fn successful_refresh_preserves_selection_and_clears_empty_details() {
        let mut a = app();
        a.selected = Some(2);
        a.detail = Some(a.beads[2].data.clone());
        a.detail_offset = 20;
        let (tx, rx) = mpsc::channel();
        a.responses = rx;
        tx.send(Response::List(Ok(vec![
            a.beads[2].clone(),
            a.beads[0].clone(),
        ])))
        .unwrap();
        a.receive();
        assert_eq!(a.beads[a.selected.unwrap()].id, "b2");
        assert_eq!(a.detail_offset, 20);

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
    fn refresh_keeps_existing_group_collapse_and_drops_stale_group_state() {
        let mut a = app();
        a.collapsed.extend(["closed".into(), "removed".into()]);
        let (tx, rx) = mpsc::channel();
        a.responses = rx;
        tx.send(Response::List(Ok(vec![Bead {
            id: "c".into(),
            title: "closed issue".into(),
            status: "closed".into(),
            priority: 2,
            data: json!({}),
        }])))
        .unwrap();
        a.receive();
        assert_eq!(a.collapsed, HashSet::from(["closed".to_owned()]));
        assert_eq!(a.selected, None);
        assert!(a.detail.is_none());
        assert_eq!(a.focused_group.as_deref(), Some("closed"));
    }
    #[test]
    fn refresh_preserves_expanded_and_collapsed_headers_without_selecting_beads() {
        let mut a = app();
        a.beads[1].status = "closed".into();
        a.focus_group("closed".into());
        for collapsed in [false, true] {
            if collapsed {
                a.collapsed.insert("closed".into());
            }
            let mut items = a.beads.clone();
            items.reverse();
            respond(&mut a, Response::List(Ok(items)));
            assert_header(&a, "closed");
            assert_eq!(a.collapsed.contains("closed"), collapsed);
            press(&mut a, KeyCode::Char('d'));
            assert!(a.modal.is_none());
        }
    }
    #[test]
    fn refresh_tracks_bead_id_across_reorder_status_moves_and_collapsed_destination() {
        let mut a = app();
        a.select(2);
        let old_seq = a.detail_seq;
        let old_detail = a.detail.clone();
        let mut items = a.beads.clone();
        items[2].status = "closed".into();
        items[2].data = json!({"id": "b2", "description": "updated detail"});
        items.reverse();
        respond(&mut a, Response::List(Ok(items)));
        assert_eq!(a.beads[a.selected.unwrap()].id, "b2");
        assert_eq!(a.detail, old_detail);
        assert_eq!(a.selected, Some(3));
        assert_eq!(a.detail_offset, 0);
        respond(
            &mut a,
            Response::Show("b2".into(), old_seq, Ok(json!({"stale": true}))),
        );
        assert_eq!(a.detail, old_detail);
        press(&mut a, KeyCode::Char('d'));
        assert!(matches!(&a.modal, Some(Modal::ConfirmDelete { id, .. }) if id == "b2"));
        press(&mut a, KeyCode::Esc);
        a.collapsed.insert("blocked".into());
        let mut items = a.beads.clone();
        items.iter_mut().find(|b| b.id == "b2").unwrap().status = "blocked".into();
        respond(&mut a, Response::List(Ok(items)));
        assert_header(&a, "blocked");
        assert!(!a.collapsed.contains("closed"));
        press(&mut a, KeyCode::Char(' '));
        assert_bead(&a, "b2");
    }
    #[test]
    fn refresh_missing_bead_or_header_chooses_first_visible_bead_then_first_header() {
        let mut a = app();
        a.beads[0].status = "closed".into();
        a.beads[1].status = "blocked".into();
        a.collapsed.insert("open".into());
        a.select(0);
        let items = a.beads.iter().filter(|b| b.id != "b0").cloned().collect();
        respond(&mut a, Response::List(Ok(items)));
        assert_bead(&a, "b1");
        a.focus_group("blocked".into());
        let mut items = a.beads.clone();
        items.iter_mut().find(|b| b.id == "b1").unwrap().status = "closed".into();
        respond(&mut a, Response::List(Ok(items)));
        assert_bead(&a, "b1");
        a.collapsed.insert("closed".into());
        a.focus_group("closed".into());
        let items = a
            .beads
            .iter()
            .filter(|b| b.status != "closed")
            .cloned()
            .collect();
        respond(&mut a, Response::List(Ok(items)));
        assert_header(&a, "open");
        assert_eq!(a.collapsed, HashSet::from(["open".into()]));
        respond(&mut a, Response::List(Ok(vec![])));
        assert!(a.focused_group.is_none());
        assert!(a.selected.is_none());
        assert!(a.detail.is_none());
        assert!(a.collapsed.is_empty());
    }
    #[test]
    fn preferred_id_overrides_header_or_bead_focus_and_expands_destination() {
        for header in [false, true] {
            let mut a = app();
            a.beads[1].status = "closed".into();
            a.collapsed.insert("closed".into());
            if header {
                a.focus_group("open".into());
            }
            a.preferred_id = Some("b1".into());
            let items = a.beads.clone();
            respond(&mut a, Response::List(Ok(items)));
            assert_bead(&a, "b1");
            assert!(!a.collapsed.contains("closed"));
            assert!(a.preferred_id.is_none());
        }
    }
    #[test]
    fn missing_preferred_id_does_not_discard_existing_header_focus() {
        let mut a = app();
        a.focus_group("open".into());
        a.preferred_id = Some("missing".into());
        let items = a.beads.clone();
        respond(&mut a, Response::List(Ok(items)));
        assert_header(&a, "open");
        assert!(a.preferred_id.is_none());
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
        press(&mut a, KeyCode::Tab);
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
    fn form_tab_order_follows_visual_layout_and_wraps_both_directions() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        let mut forward = Vec::new();
        for _ in 0..6 {
            press(&mut a, KeyCode::Tab);
            if let Some(Modal::Create(f)) = &a.modal {
                forward.push(f.focus)
            }
        }
        assert_eq!(forward, [2, 3, 1, 4, 5, 0]);
        let mut reverse = Vec::new();
        for _ in 0..6 {
            press(&mut a, KeyCode::BackTab);
            if let Some(Modal::Create(f)) = &a.modal {
                reverse.push(f.focus)
            }
        }
        assert_eq!(reverse, [5, 4, 1, 3, 2, 0]);
    }
    #[test]
    fn dropdown_escape_closes_choice_before_form_and_delete_captures_selection() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
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
        press(&mut a, KeyCode::Tab);
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
