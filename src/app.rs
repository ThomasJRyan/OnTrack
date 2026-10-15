use crate::herdr::{Agent, LaunchContext, LaunchOutcome};
use crate::worktrees::{self, Action as WorktreeAction, CleanupPolicy, ManagedWorktree};
use crate::{
    Tui,
    beads::{self, Bead, CreateBead},
    event,
    settings::{self, Settings, Theme},
    ui,
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use std::{
    collections::HashSet,
    path::PathBuf,
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
    Discover(u64),
    Launch(u64, LaunchContext, Agent, Bead),
    ListWorktrees,
    WorktreeAction(String, WorktreeAction),
    ReconcileWorktrees(CleanupPolicy),
}
pub(super) enum Response {
    List(std::io::Result<Vec<Bead>>),
    Show(String, u64, std::io::Result<serde_json::Value>),
    Created(std::io::Result<String>),
    Deleted(std::io::Result<()>),
    Discovered(u64, std::io::Result<LaunchContext>),
    Launched(u64, std::io::Result<LaunchOutcome>),
    Worktrees(std::io::Result<Vec<ManagedWorktree>>),
    WorktreeChanged(std::io::Result<ManagedWorktree>),
}
pub struct App {
    pub beads: Vec<Bead>,
    pub collapsed: HashSet<String>,
    pub(crate) closed_default_pending: bool,
    pub collapse_context: Option<String>,
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
    pub(super) launch_seq: u64,
    pub list_rect: Rect,
    pub detail_rect: Rect,
    pub modal: Option<Modal>,
    pub settings: Settings,
    pub theme: Theme,
    pub settings_path: Option<PathBuf>,
    pub settings_load_error: Option<String>,
    pub worktrees: Vec<ManagedWorktree>,
    pub selected_worktree: usize,
    pub worktree_offset: usize,
    pub worktree_detail_offset: u16,
    pub worktrees_view: bool,
    pub worktrees_loading: bool,
    pub worktrees_error: Option<String>,
    pub worktrees_inflight: bool,
    pub worktrees_discovered: bool,
    pub worktrees_refresh_pending: bool,
    pub worktree_action_pending: bool,
    pub worktrees_next_refresh: Instant,
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
    ConfirmDelete {
        id: String,
        title: String,
    },
    Settings {
        draft: Settings,
        error: Option<String>,
        dropdown_open: bool,
        focus: usize,
    },
    Launch(LaunchModal),
    ConfirmWorktree {
        id: String,
        action: u8,
    },
}
#[derive(Debug, Clone)]
pub struct LaunchModal {
    pub bead: Bead,
    pub seq: u64,
    pub context: Option<LaunchContext>,
    pub selected_agent: usize,
    pub error: Option<String>,
    pub scroll: u16,
    pub finished: bool,
}
impl PartialEq for LaunchModal {
    fn eq(&self, other: &Self) -> bool {
        self.bead.id == other.bead.id
            && self.seq == other.seq
            && self.context == other.context
            && self.selected_agent == other.selected_agent
            && self.error == other.error
            && self.scroll == other.scroll
            && self.finished == other.finished
    }
}
impl Eq for LaunchModal {}
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

/// Return the next choice index, wrapping in either direction.
fn cycle_choice(index: usize, choices: usize, forward: bool) -> usize {
    assert!(
        choices > 0,
        "a choice control must have at least one option"
    );
    if forward {
        (index % choices + 1) % choices
    } else {
        (index % choices + choices - 1) % choices
    }
}

fn cycle_theme(theme: Theme, forward: bool) -> Theme {
    let index = Theme::ALL
        .iter()
        .position(|choice| *choice == theme)
        .unwrap_or(0);
    Theme::ALL[cycle_choice(index, Theme::ALL.len(), forward)]
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
                    Request::Discover(seq) => Response::Discovered(seq, crate::herdr::discover()),
                    Request::Launch(seq, context, agent, bead) => {
                        Response::Launched(seq, crate::herdr::launch(&context, &agent, &bead))
                    }
                    Request::ListWorktrees => Response::Worktrees(worktrees::list()),
                    Request::WorktreeAction(id, action) => {
                        Response::WorktreeChanged(worktrees::act(&id, action))
                    }
                    Request::ReconcileWorktrees(policy) => {
                        Response::Worktrees(worktrees::reconcile(policy))
                    }
                };
                if out.send(response).is_err() {
                    break;
                }
            }
        });
        let (settings, settings_path, settings_error) = match settings::config_path() {
            Ok(path) => match settings::load_from(&path) {
                Ok(value) => (value, Some(path), None),
                Err(error) => (
                    Settings::default(),
                    Some(path),
                    Some(format!("Settings could not be loaded: {error}")),
                ),
            },
            Err(error) => (
                Settings::default(),
                None,
                Some(format!("Settings location unavailable: {error}")),
            ),
        };
        let theme = settings.theme;
        let mut a = Self {
            beads: vec![],
            collapsed: HashSet::new(),
            closed_default_pending: true,
            collapse_context: None,
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
            launch_seq: 0,
            list_rect: Rect::default(),
            detail_rect: Rect::default(),
            modal: None,
            settings,
            theme,
            settings_path,
            settings_load_error: settings_error.clone(),
            worktrees: Vec::new(),
            selected_worktree: 0,
            worktree_offset: 0,
            worktree_detail_offset: 0,
            worktrees_view: false,
            worktrees_loading: false,
            worktrees_error: None,
            worktrees_inflight: false,
            worktrees_discovered: false,
            worktrees_refresh_pending: false,
            worktree_action_pending: false,
            worktrees_next_refresh: Instant::now(),
            notice: settings_error,
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
    fn poll_worktrees(&mut self, now: Instant) {
        let automatic = self.settings.auto_merge
            || self.settings.auto_close_workspace
            || self.settings.auto_remove_checkout;
        let should_poll = !self.worktrees_discovered
            || automatic
            || !self.worktrees.is_empty()
            || self.worktrees_view;
        if should_poll
            && !self.worktrees_inflight
            && !self.worktree_action_pending
            && now >= self.worktrees_next_refresh
        {
            self.worktrees_inflight = true;
            self.worktrees_loading = true;
            let policy = CleanupPolicy {
                auto_merge: self.settings.auto_merge,
                auto_close_workspace: self.settings.auto_close_workspace,
                auto_remove_checkout: self.settings.auto_remove_checkout,
            };
            let request = if !self.worktrees_discovered
                || (self.worktrees_view && self.worktrees.is_empty())
            {
                Request::ListWorktrees
            } else {
                Request::ReconcileWorktrees(policy)
            };
            if self.requests.send(request).is_err() {
                self.worktrees_inflight = false;
                self.worktrees_loading = false;
                self.worktrees_error = Some("Worktree refresh could not be queued".into());
                self.worktrees_next_refresh = now + REFRESH_INTERVAL;
            }
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
        self.collapse_context = None;
        if self.beads.is_empty() {
            self.clear_selection();
            return;
        }
        let index = index.min(self.beads.len() - 1);
        let status = normalized_status(&self.beads[index].status).to_owned();
        if self.collapsed.contains(&status) {
            self.clear_selection();
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
    fn step(&mut self, d: isize) {
        let targets = list_layout(&self.beads, &self.collapsed)
            .targets
            .into_iter()
            .filter_map(|target| match target.row {
                DisplayRow::Bead(index) => Some(DisplayRow::Bead(index)),
                DisplayRow::Group(status) if self.collapsed.contains(&status) => {
                    Some(DisplayRow::Group(status))
                }
                DisplayRow::Group(_) => None,
            })
            .collect::<Vec<_>>();
        if targets.is_empty() {
            self.clear_selection();
            self.collapse_context = None;
            return;
        }
        let current = self
            .selected
            .and_then(|index| {
                targets
                    .iter()
                    .position(|target| *target == DisplayRow::Bead(index))
            })
            .or_else(|| {
                self.collapse_context.as_ref().and_then(|status| {
                    targets
                        .iter()
                        .position(|target| target == &DisplayRow::Group(status.clone()))
                })
            });
        let next = current
            .map(|i| i.saturating_add_signed(d).min(targets.len() - 1))
            .unwrap_or(if d < 0 { targets.len() - 1 } else { 0 });
        match &targets[next] {
            DisplayRow::Bead(index) => self.select(*index),
            DisplayRow::Group(status) => self.focus_collapsed_group(status.clone()),
        }
    }
    fn toggle_selected_group(&mut self) {
        if let Some(status) = self.focused_status() {
            self.toggle_group(status);
        }
    }
    fn focused_status(&self) -> Option<String> {
        self.selected
            .and_then(|i| self.beads.get(i))
            .map(|b| normalized_status(&b.status).to_owned())
            .or_else(|| self.collapse_context.clone())
    }
    fn focus_collapsed_group(&mut self, status: String) {
        self.clear_selection();
        self.collapse_context = Some(status);
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
        let selected_in_group = self
            .selected
            .is_some_and(|i| normalized_status(&self.beads[i].status) == status);
        if collapsed {
            self.collapsed.insert(status.clone());
            if selected_in_group {
                self.focus_collapsed_group(status.clone());
            }
        } else {
            self.collapsed.remove(&status);
            if let Some(i) = self
                .beads
                .iter()
                .position(|b| normalized_status(&b.status) == status)
            {
                self.select(i);
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
            }
        } else {
            self.set_group_collapsed(status, true);
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
                            let collapse_context = self.collapse_context.clone();
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
                            if self.closed_default_pending
                                && self.beads.iter().any(|bead| bead.status == "closed")
                            {
                                self.collapsed.insert("closed".into());
                                self.closed_default_pending = false;
                            }
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
                                self.collapse_context = None;
                                self.collapsed.remove(&self.beads[i].status);
                                if Some(i) == same_id {
                                    self.selected = Some(i);
                                    self.refresh_detail();
                                } else {
                                    self.select(i);
                                }
                            } else if let Some(status) = collapse_context.filter(|status| {
                                self.collapsed.contains(status)
                                    && self.beads.iter().any(|bead| bead.status == *status)
                            }) {
                                self.focus_collapsed_group(status);
                            } else if let Some(i) = same_id {
                                self.collapse_context = None;
                                if self.collapsed.contains(&self.beads[i].status) {
                                    self.clear_selection();
                                    if let Some(visible) = list_layout(&self.beads, &self.collapsed)
                                        .targets
                                        .into_iter()
                                        .find_map(|target| match target.row {
                                            DisplayRow::Bead(index) => Some(index),
                                            DisplayRow::Group(_) => None,
                                        })
                                    {
                                        self.select(visible);
                                    }
                                } else {
                                    self.selected = Some(i);
                                    self.refresh_detail();
                                }
                            } else {
                                self.clear_selection();
                                self.collapse_context = None;
                                let targets = list_layout(&self.beads, &self.collapsed).targets;
                                if let Some(i) = targets.iter().find_map(|t| match t.row {
                                    DisplayRow::Bead(i) => Some(i),
                                    DisplayRow::Group(_) => None,
                                }) {
                                    self.select(i);
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
                Response::Discovered(seq, result) => {
                    if let Some(Modal::Launch(modal)) = self.modal.as_mut()
                        && modal.seq == seq
                    {
                        match result {
                            Ok(context) => {
                                modal.selected_agent = 0;
                                modal.error = None;
                                modal.context = Some(context);
                            }
                            Err(error) => modal.error = Some(error.to_string()),
                        }
                    }
                }
                Response::Launched(seq, result) => {
                    self.mutation_busy = false;
                    if seq == self.launch_seq {
                        match result {
                            Ok(outcome) => {
                                self.notice = Some(outcome.message);
                                // The launch journal is now active. Reload it even if the
                                // initial worktree scan found no managed handoffs.
                                if self.worktrees_inflight {
                                    self.worktrees_refresh_pending = true;
                                } else {
                                    self.worktrees_discovered = false;
                                    self.worktrees_next_refresh = now;
                                }
                                if let Some(Modal::Launch(modal)) = self.modal.as_mut() {
                                    modal.finished = true;
                                    modal.scroll = 0;
                                }
                            }
                            Err(error) => {
                                self.notice = Some(format!("Launch failed: {error}"));
                                if let Some(Modal::Launch(modal)) = self.modal.as_mut() {
                                    modal.finished = true;
                                    modal.scroll = 0;
                                }
                            }
                        }
                    }
                }
                Response::Worktrees(result) => {
                    self.worktrees_inflight = false;
                    self.worktrees_loading = false;
                    self.worktrees_next_refresh = now + REFRESH_INTERVAL;
                    self.worktrees_discovered = true;
                    if !self.worktree_action_pending {
                        match result {
                            Ok(items) => {
                                let selected_id = self
                                    .worktrees
                                    .get(self.selected_worktree)
                                    .map(|item| item.id.clone());
                                self.worktrees = items;
                                self.selected_worktree = selected_id
                                    .and_then(|id| {
                                        self.worktrees.iter().position(|item| item.id == id)
                                    })
                                    .unwrap_or(self.selected_worktree)
                                    .min(self.worktrees.len().saturating_sub(1));
                                self.worktrees_error = None;
                            }
                            Err(error) => self.worktrees_error = Some(error.to_string()),
                        }
                    }
                    if std::mem::take(&mut self.worktrees_refresh_pending) {
                        self.worktrees_discovered = false;
                        self.worktrees_next_refresh = now;
                    }
                }
                Response::WorktreeChanged(result) => {
                    self.mutation_busy = false;
                    self.modal = None;
                    self.worktree_action_pending = false;
                    match result {
                        Ok(item) => {
                            if let Some(index) = self
                                .worktrees
                                .iter()
                                .position(|existing| existing.id == item.id)
                            {
                                self.worktrees[index] = item.clone();
                            }
                            self.notice = Some(format!(
                                "{} · {}",
                                item.branch,
                                worktree_status_label(item.status)
                            ));
                            self.worktrees_next_refresh = now + REFRESH_INTERVAL;
                        }
                        Err(e) => {
                            self.worktrees_error = Some(e.to_string());
                            self.worktrees_next_refresh = now;
                        }
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
        if self.worktrees_view {
            match k.code {
                KeyCode::Char('w') | KeyCode::Esc | KeyCode::Backspace => {
                    self.worktrees_view = false
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected_worktree = self.selected_worktree.saturating_sub(1);
                    self.worktree_detail_offset = 0;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected_worktree =
                        (self.selected_worktree + 1).min(self.worktrees.len().saturating_sub(1));
                    self.worktree_detail_offset = 0;
                }
                KeyCode::PageUp => {
                    self.worktree_detail_offset = self
                        .worktree_detail_offset
                        .saturating_sub(self.detail_rect.height.saturating_sub(2));
                }
                KeyCode::PageDown => {
                    self.worktree_detail_offset = self
                        .worktree_detail_offset
                        .saturating_add(self.detail_rect.height.saturating_sub(2));
                }
                KeyCode::Char('r') => {
                    self.worktrees_next_refresh = Instant::now();
                    self.poll_worktrees(Instant::now());
                }
                KeyCode::Char('m') => self.confirm_worktree_action(0),
                KeyCode::Char('c') => self.confirm_worktree_action(1),
                KeyCode::Char('x') => self.confirm_worktree_action(2),
                _ => {}
            }
            return false;
        }
        match k.code {
            KeyCode::Char('w') => {
                self.worktrees_view = true;
                self.worktrees_next_refresh = Instant::now();
                self.poll_worktrees(Instant::now());
            }
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Char('s') => {
                let load_error = self
                    .settings_load_error
                    .as_ref()
                    .map(|error| format!("Settings were not loaded; defaults are active. {error}"));
                self.modal = Some(Modal::Settings {
                    draft: self.settings,
                    error: load_error,
                    dropdown_open: false,
                    focus: 0,
                });
                self.modal_ready = false;
            }
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
            KeyCode::Char('h') => self.open_launch(),
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
    fn confirm_worktree_action(&mut self, action: u8) {
        let Some(item) = self.worktrees.get(self.selected_worktree) else {
            self.worktrees_error = Some("Select a managed worktree first".into());
            return;
        };
        if action == 1 && item.status != worktrees::WorktreeStatus::Integrated {
            self.worktrees_error =
                Some("Cleanup is available only after verified integration".into());
            return;
        }
        if action == 2
            && !matches!(
                item.status,
                worktrees::WorktreeStatus::Integrated | worktrees::WorktreeStatus::WorkspaceClosed
            )
        {
            self.worktrees_error = Some("Checkout removal requires verified integration".into());
            return;
        }
        if action == 0
            && !matches!(
                item.status,
                worktrees::WorktreeStatus::Unmerged | worktrees::WorktreeStatus::NeedsResolution
            )
        {
            self.worktrees_error = Some("This worktree is not ready for a merge request".into());
            return;
        }
        self.modal = Some(Modal::ConfirmWorktree {
            id: item.id.clone(),
            action,
        });
        self.modal_ready = false;
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
            if matches!(self.modal, Some(Modal::Launch(_))) {
                self.launch_seq = self.launch_seq.wrapping_add(1);
                self.modal = None;
                return false;
            }
            if let Some(Modal::Settings { dropdown_open, .. }) = self.modal.as_mut() {
                if *dropdown_open {
                    *dropdown_open = false;
                } else {
                    self.modal = None;
                }
                return false;
            }
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
        if let Some(Modal::ConfirmWorktree { id, action }) = self.modal.as_ref() {
            if matches!(key.code, Char('y') | Char('Y')) {
                let id = id.clone();
                let action = match action {
                    0 => WorktreeAction::RequestMerge,
                    1 => WorktreeAction::CloseWorkspace,
                    _ => WorktreeAction::RemoveCheckout,
                };
                self.mutation_busy = true;
                if self
                    .requests
                    .send(Request::WorktreeAction(id, action))
                    .is_err()
                {
                    self.mutation_busy = false;
                    self.worktrees_error = Some("Action could not be queued".into());
                } else {
                    self.worktree_action_pending = true;
                }
            } else if matches!(key.code, Char('n') | Char('N') | Enter) {
                self.modal = None;
            }
            return false;
        }
        if matches!(self.modal, Some(Modal::Settings { .. })) {
            let mut save_settings = None;
            if let Some(Modal::Settings {
                draft,
                dropdown_open,
                focus,
                ..
            }) = self.modal.as_mut()
            {
                match key.code {
                    Tab | BackTab => {
                        *dropdown_open = false;
                        *focus = (*focus + if key.code == Tab { 1 } else { 4 }) % 5;
                    }
                    Up | Down => {
                        *focus = (*focus + if key.code == Down { 1 } else { 4 }) % 5;
                        *dropdown_open = false;
                    }
                    Left | Right if *focus == 0 => {
                        draft.theme = cycle_theme(draft.theme, key.code == Right);
                        save_settings = Some(*draft);
                    }
                    Enter | Char(' ') if *focus == 0 && *dropdown_open => {
                        *dropdown_open = false;
                        save_settings = Some(*draft);
                    }
                    Enter | Char(' ') if *focus == 0 => {
                        *dropdown_open = true;
                    }
                    Enter | Char(' ') if (1..=3).contains(focus) => {
                        match *focus {
                            1 => draft.auto_merge = !draft.auto_merge,
                            2 => draft.auto_close_workspace = !draft.auto_close_workspace,
                            _ => draft.auto_remove_checkout = !draft.auto_remove_checkout,
                        }
                        save_settings = Some(*draft);
                    }
                    Enter | Char(' ') if *focus == 4 => {
                        self.modal = None;
                    }
                    _ => {}
                }
            }
            if let Some(settings) = save_settings {
                self.save_settings(settings);
            }
            return false;
        }
        if let Some(Modal::Launch(modal)) = self.modal.as_mut() {
            match key.code {
                PageUp => modal.scroll = modal.scroll.saturating_sub(6),
                PageDown => modal.scroll = modal.scroll.saturating_add(6),
                Enter if modal.finished => {}
                Up | Char('k') if modal.context.as_ref().is_some_and(|c| !c.agents.is_empty()) => {
                    modal.selected_agent = modal.selected_agent.saturating_sub(1);
                }
                Down | Char('j')
                    if modal.context.as_ref().is_some_and(|c| !c.agents.is_empty()) =>
                {
                    let len = modal.context.as_ref().unwrap().agents.len();
                    modal.selected_agent = (modal.selected_agent + 1).min(len - 1);
                }
                Enter => {
                    let Some(context) = modal.context.clone() else {
                        return false;
                    };
                    let Some(agent) = context.agents.get(modal.selected_agent).cloned() else {
                        self.notice = Some("No agents are available in this workspace".into());
                        return false;
                    };
                    let seq = modal.seq;
                    let bead = modal.bead.clone();
                    self.mutation_busy = true;
                    if self
                        .requests
                        .send(Request::Launch(seq, context, agent, bead))
                        .is_err()
                    {
                        self.mutation_busy = false;
                        self.notice = Some("Launch could not be queued; try again".into());
                    }
                }
                Char('h') if modal.error.is_some() => {
                    let bead = modal.bead.clone();
                    self.launch_seq = self.launch_seq.wrapping_add(1);
                    let seq = self.launch_seq;
                    self.modal = Some(Modal::Launch(LaunchModal {
                        bead,
                        seq,
                        context: None,
                        selected_agent: 0,
                        error: None,
                        scroll: 0,
                        finished: false,
                    }));
                    if self.requests.send(Request::Discover(seq)).is_err()
                        && let Some(Modal::Launch(modal)) = self.modal.as_mut()
                    {
                        modal.error =
                            Some("Workspace discovery could not be queued; try again".into());
                    }
                }
                _ => {}
            }
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
                Left | Right => {
                    if which == 2 {
                        form.issue_type = cycle_choice(
                            form.issue_type,
                            beads::BEAD_TYPES.len(),
                            key.code == Right,
                        );
                    } else {
                        form.priority = cycle_choice(form.priority, 5, key.code == Right);
                    }
                }
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
            Left | Right if form.focus == 2 => {
                form.issue_type =
                    cycle_choice(form.issue_type, beads::BEAD_TYPES.len(), key.code == Right);
            }
            Left | Right if form.focus == 3 => {
                form.priority = cycle_choice(form.priority, 5, key.code == Right);
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
            Enter if form.focus == 1 && key.modifiers == crossterm::event::KeyModifiers::SHIFT => {
                let i = form.description_cursor.min(form.description.len());
                form.description.insert(i, '\n');
                form.description_cursor = i + 1;
            }
            Enter if form.focus == 1 && key.modifiers.is_empty() => form.focus = 4,
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
    fn save_settings(&mut self, proposed: Settings) {
        let result = self
            .settings_path
            .as_deref()
            .ok_or_else(|| "Settings location is unavailable; cannot save.".to_owned())
            .and_then(|path| settings::save_to(path, &proposed).map_err(|error| error.to_string()));
        match result {
            Ok(()) => {
                self.settings = proposed;
                self.theme = proposed.theme;
                self.settings_load_error = None;
                self.notice = Some("Settings saved".into());
                if let Some(Modal::Settings { error, .. }) = self.modal.as_mut() {
                    *error = None;
                }
            }
            Err(error) => {
                if let Some(Modal::Settings { error: message, .. }) = self.modal.as_mut() {
                    *message = Some(format!("Could not save settings: {error}"));
                }
            }
        }
    }
    fn open_launch(&mut self) {
        let Some(bead) = self
            .selected
            .and_then(|index| self.beads.get(index))
            .cloned()
        else {
            self.notice = Some("Select a bead before handing it to an agent".into());
            return;
        };
        self.launch_seq = self.launch_seq.wrapping_add(1);
        let seq = self.launch_seq;
        self.modal = Some(Modal::Launch(LaunchModal {
            bead,
            seq,
            context: None,
            selected_agent: 0,
            error: None,
            scroll: 0,
            finished: false,
        }));
        self.notice = None;
        if self.requests.send(Request::Discover(seq)).is_err()
            && let Some(Modal::Launch(modal)) = self.modal.as_mut()
        {
            modal.error = Some("Workspace discovery could not be queued; try again".into());
        }
    }
    fn mouse(&mut self, k: MouseEventKind, x: u16, y: u16) {
        if self.modal.is_some() {
            return;
        }
        if self.worktrees_view {
            match k {
                MouseEventKind::Down(MouseButton::Left)
                    if self.list_rect.contains((x, y).into()) && y > self.list_rect.y =>
                {
                    let row = usize::from(y - self.list_rect.y - 1) / 2 + self.worktree_offset;
                    if row < self.worktrees.len() {
                        self.selected_worktree = row;
                        self.worktree_detail_offset = 0;
                    }
                }
                MouseEventKind::ScrollUp if self.detail_rect.contains((x, y).into()) => {
                    self.worktree_detail_offset = self.worktree_detail_offset.saturating_sub(3);
                }
                MouseEventKind::ScrollDown if self.detail_rect.contains((x, y).into()) => {
                    self.worktree_detail_offset = self.worktree_detail_offset.saturating_add(3);
                }
                _ => {}
            }
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
pub(super) fn worktree_status_label(status: worktrees::WorktreeStatus) -> &'static str {
    use worktrees::WorktreeStatus::*;
    match status {
        Unmerged => "UNMERGED",
        MergeRequested => "MERGE PENDING",
        NeedsResolution => "NEEDS RESOLUTION",
        Integrated => "INTEGRATED",
        WorkspaceClosed => "WORKSPACE CLOSED",
        CheckoutRemoved => "CHECKOUT REMOVED",
        NeedsAttention => "NEEDS ATTENTION",
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
        app.poll_worktrees(Instant::now());
        t.draw(|f| ui::render(f, start.elapsed(), &mut app))?;
        if let Some(e) = event::poll_event(Duration::from_millis(35))? {
            match e {
                crossterm::event::Event::Key(k)
                    if matches!(
                        k.kind,
                        crossterm::event::KeyEventKind::Press
                            | crossterm::event::KeyEventKind::Repeat
                    ) && app.key(k) =>
                {
                    break;
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
            closed_default_pending: false,
            collapse_context: None,
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
            launch_seq: 0,
            list_rect: Rect::new(0, 0, 30, 8),
            detail_rect: Rect::new(30, 0, 50, 8),
            modal: None,
            settings: Settings::default(),
            theme: Theme::Dark,
            settings_path: None,
            settings_load_error: None,
            worktrees: Vec::new(),
            selected_worktree: 0,
            worktree_offset: 0,
            worktree_detail_offset: 0,
            worktrees_view: false,
            worktrees_loading: false,
            worktrees_error: None,
            worktrees_inflight: false,
            worktrees_discovered: true,
            worktrees_refresh_pending: false,
            worktree_action_pending: false,
            worktrees_next_refresh: Instant::now(),
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
    fn assert_collapsed_header_selected(a: &App, status: &str) {
        assert_eq!(a.collapse_context.as_deref(), Some(status));
        assert!(a.collapsed.contains(status));
        assert!(a.selected.is_none());
        assert!(a.detail.is_none());
    }
    fn assert_bead(a: &App, id: &str) {
        let bead = &a.beads[a.selected.expect("selected bead")];
        assert_eq!(bead.id, id);
        assert_eq!(a.detail.as_ref(), Some(&bead.data));
    }
    fn unloaded_app() -> App {
        let mut a = app();
        a.beads.clear();
        a.clear_selection();
        a.closed_default_pending = true;
        a
    }
    fn launch_context() -> LaunchContext {
        LaunchContext {
            workspace: crate::herdr::Workspace {
                id: "ws".into(),
                label: "ws".into(),
            },
            agents: vec![
                Agent {
                    id: "codex".into(),
                    label: "Codex".into(),
                },
                Agent {
                    id: "opencode".into(),
                    label: "OpenCode".into(),
                },
            ],
        }
    }
    #[test]
    fn launch_discovery_is_queued_without_launch_until_explicit_confirmation() {
        let (mut a, rx, tx) = connected_app();
        a.beads[0].data["description"] = json!("Preserve full bead context for the worker");
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => panic!("expected discovery"),
        };
        assert!(rx.try_recv().is_err());
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        press(&mut a, KeyCode::Down);
        assert!(rx.try_recv().is_err());
        press(&mut a, KeyCode::Enter);
        assert!(
            matches!(rx.try_recv(), Ok(Request::Launch(_, context, agent, bead)) if context.workspace.id == "ws" && agent.id == "opencode" && bead.id == "b0" && bead.title == "issue 0" && bead.data["description"] == "Preserve full bead context for the worker")
        );
    }
    #[test]
    fn handoff_gates_selection_and_captures_selected_bead_across_discovery() {
        let (mut a, rx, tx) = connected_app();
        a.clear_selection();
        press(&mut a, KeyCode::Char('h'));
        assert!(rx.try_recv().is_err());
        a.collapsed.insert("open".into());
        a.focus_collapsed_group("open".into());
        press(&mut a, KeyCode::Char('h'));
        assert!(rx.try_recv().is_err());
        assert!(a.modal.is_none());

        a.collapsed.clear();
        a.select(0);
        let _ = rx.try_recv(); // selection detail request
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        a.select(1);
        let _ = rx.try_recv(); // changed selection detail request
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        press(&mut a, KeyCode::Enter);
        assert!(
            matches!(rx.try_recv(), Ok(Request::Launch(_, _, _, bead)) if bead.id == "b0" && bead.title == "issue 0")
        );
    }
    #[test]
    fn empty_agents_prevent_launch_and_duplicate_confirmation_is_ignored() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(
            seq,
            Ok(LaunchContext {
                workspace: crate::herdr::Workspace {
                    id: "ws".into(),
                    label: "ws".into(),
                },
                agents: vec![],
            }),
        ))
        .unwrap();
        a.receive();
        press(&mut a, KeyCode::Enter);
        assert!(!a.mutation_busy);
        assert!(rx.try_recv().is_err());

        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        press(&mut a, KeyCode::Enter);
        assert!(matches!(rx.try_recv(), Ok(Request::Launch(..))));
        press(&mut a, KeyCode::Enter);
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn disconnected_discovery_and_launch_queues_do_not_lock_the_ui() {
        let mut a = app();
        let (requests, rx) = mpsc::channel();
        drop(rx);
        a.requests = requests;
        press(&mut a, KeyCode::Char('h'));
        assert!(!a.mutation_busy);
        assert!(
            matches!(&a.modal, Some(Modal::Launch(m)) if m.error.as_deref().is_some_and(|e| e.contains("could not be queued")))
        );

        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        let (requests, receiver) = mpsc::channel();
        drop(receiver);
        a.requests = requests;
        press(&mut a, KeyCode::Enter);
        assert!(!a.mutation_busy);
        assert!(
            a.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("Launch could not be queued"))
        );
    }
    #[test]
    fn launch_cancel_ignores_stale_discovery_and_busy_guard_blocks_input() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        press(&mut a, KeyCode::Esc);
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(a.modal.is_none());
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        a.mutation_busy = true;
        press(&mut a, KeyCode::Down);
        assert!(matches!(
            a.modal,
            Some(Modal::Launch(LaunchModal {
                selected_agent: 0,
                ..
            }))
        ));
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn discovery_failure_is_reported_and_retryable() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(
            seq,
            Err(std::io::Error::other("workspace unavailable")),
        ))
        .unwrap();
        a.receive();
        assert!(
            matches!(&a.modal, Some(Modal::Launch(m)) if m.error.as_deref() == Some("workspace unavailable"))
        );
        press(&mut a, KeyCode::Char('h'));
        assert!(matches!(rx.try_recv(), Ok(Request::Discover(_))));
    }
    #[test]
    fn completed_launch_result_scrolls_without_relaunch_and_can_be_dismissed() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        press(&mut a, KeyCode::Enter);
        assert!(matches!(rx.try_recv(), Ok(Request::Launch(..))));
        // A snapshot that began before launch must not hide the new handoff.
        a.worktrees_inflight = true;
        tx.send(Response::Launched(
            seq,
            Ok(LaunchOutcome {
                message: "Prompt sent · workspace ws-new · pane p1 · branch ontrack/b0".into(),
            }),
        ))
        .unwrap();
        a.receive();
        assert!(!a.mutation_busy);
        assert!(a.worktrees_refresh_pending);
        tx.send(Response::Worktrees(Ok(vec![]))).unwrap();
        a.receive();
        a.poll_worktrees(Instant::now());
        assert!(matches!(rx.try_recv(), Ok(Request::ListWorktrees)));
        assert_eq!(
            a.notice.as_deref(),
            Some("Prompt sent · workspace ws-new · pane p1 · branch ontrack/b0")
        );
        assert!(matches!(&a.modal, Some(Modal::Launch(m)) if m.finished));
        press(&mut a, KeyCode::Enter);
        assert!(
            rx.try_recv().is_err(),
            "Enter cannot repeat a completed launch"
        );
        press(&mut a, KeyCode::PageDown);
        assert!(matches!(&a.modal, Some(Modal::Launch(m)) if m.scroll == 6));
        press(&mut a, KeyCode::PageUp);
        assert!(matches!(&a.modal, Some(Modal::Launch(m)) if m.scroll == 0));
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none());
    }
    #[test]
    fn failed_launch_clears_busy_and_keeps_actionable_result_open() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        press(&mut a, KeyCode::Enter);
        assert!(matches!(rx.try_recv(), Ok(Request::Launch(..))));
        press(&mut a, KeyCode::Enter);
        assert!(rx.try_recv().is_err());
        tx.send(Response::Launched(
            seq,
            Err(std::io::Error::other(
                "worktree may exist; inspect before retrying",
            )),
        ))
        .unwrap();
        a.receive();
        assert!(!a.mutation_busy);
        assert!(matches!(&a.modal, Some(Modal::Launch(modal)) if modal.finished));
        assert!(
            a.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("inspect before retrying"))
        );
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
    fn refresh_preserves_surviving_collapsed_header_selection() {
        let (mut a, rx, tx) = connected_app();
        a.collapsed.insert("open".into());
        a.focus_collapsed_group("open".into());
        a.focus_detail = true;
        a.list_offset = 4;
        let now = Instant::now();
        a.poll_refresh(now);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        tx.send(Response::List(Ok(a.beads.clone()))).unwrap();
        a.receive_at(now);
        assert!(a.selected.is_none());
        assert_eq!(a.collapse_context.as_deref(), Some("open"));
        assert!(a.detail.is_none());
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
        assert!(a.collapse_context.is_none());
        assert_eq!(a.beads[a.selected.unwrap()].id, "b2");
        press(&mut a, KeyCode::Char('d'));
        assert!(matches!(&a.modal, Some(Modal::ConfirmDelete { id, .. }) if id == "b2"));
        press(&mut a, KeyCode::Esc);
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
        a.select(1);
        assert_bead(&a, "b1");
    }
    #[test]
    fn settings_change_autosaves_and_close_keeps_committed_theme() {
        let mut a = app();
        let dir = std::env::temp_dir().join(format!("ontrack-app-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        a.settings_path = Some(dir.join("ontrack/settings.json"));
        a.settings_load_error = Some("startup settings failed".into());
        a.notice = Some("startup settings failed".into());
        press(&mut a, KeyCode::Char('s'));
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                draft: Settings {
                    theme: Theme::Dark,
                    ..
                },
                ..
            })
        ));
        press(&mut a, KeyCode::Char('n'));
        assert!(
            matches!(a.modal, Some(Modal::Settings { .. })),
            "modal keys must not leak to the browser"
        );
        assert!(!a.key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)));
        assert!(matches!(a.modal, Some(Modal::Settings { .. })));
        press(&mut a, KeyCode::Char(' '));
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                dropdown_open: true,
                ..
            })
        ));
        press(&mut a, KeyCode::Right);
        assert_eq!(
            a.theme,
            Theme::Light,
            "successful change applies immediately"
        );
        assert_eq!(a.settings.theme, Theme::Light);
        assert!(a.settings_load_error.is_none());
        assert!(matches!(a.modal, Some(Modal::Settings { error: None, .. })));
        assert_eq!(
            settings::load_from(a.settings_path.as_deref().unwrap())
                .unwrap()
                .theme,
            Theme::Light
        );
        press(&mut a, KeyCode::Left);
        assert_eq!(a.theme, Theme::Dark, "left returns to the first choice");
        assert_eq!(
            settings::load_from(a.settings_path.as_deref().unwrap())
                .unwrap()
                .theme,
            Theme::Dark
        );
        press(&mut a, KeyCode::Right);
        assert_eq!(
            a.theme,
            Theme::Light,
            "right selects and saves the second choice"
        );
        press(&mut a, KeyCode::Enter);
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                dropdown_open: false,
                ..
            })
        ));
        assert_eq!(
            a.theme,
            Theme::Light,
            "Enter retries/saves the selected option and closes the dropdown"
        );
        press(&mut a, KeyCode::Left);
        assert_eq!(a.theme, Theme::Dark);
        assert_eq!(
            settings::load_from(a.settings_path.as_deref().unwrap())
                .unwrap()
                .theme,
            Theme::Dark
        );
        press(&mut a, KeyCode::Right);
        assert_eq!(a.theme, Theme::Light);
        assert_eq!(
            settings::load_from(a.settings_path.as_deref().unwrap())
                .unwrap()
                .theme,
            Theme::Light
        );
        press(&mut a, KeyCode::Tab);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 1, .. })));
        assert_eq!(a.theme, Theme::Light, "Tab only focuses Close");
        press(&mut a, KeyCode::BackTab);
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none(), "Esc closes Settings without rollback");
        assert_eq!(a.theme, Theme::Light);
        assert_eq!(a.settings.theme, Theme::Light);
        assert_eq!(
            settings::load_from(a.settings_path.as_deref().unwrap())
                .unwrap()
                .theme,
            Theme::Light
        );
        press(&mut a, KeyCode::Char('s'));
        assert_eq!(
            a.theme,
            Theme::Light,
            "reopening does not change the saved theme"
        );
        assert!(matches!(a.modal, Some(Modal::Settings { .. })));
        for _ in 0..4 {
            press(&mut a, KeyCode::Tab);
        }
        press(&mut a, KeyCode::Enter);
        assert!(a.modal.is_none());
        assert_eq!(
            settings::load_from(a.settings_path.as_deref().unwrap())
                .unwrap()
                .theme,
            Theme::Light
        );
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn settings_dialog_surfaces_startup_load_failure_without_dismissing_notice() {
        let mut a = app();
        let startup_error = "Settings could not be loaded: invalid JSON at /home/test/.config/ontrack/settings.json";
        a.settings_load_error = Some(startup_error.into());
        a.notice = Some(startup_error.into());
        press(&mut a, KeyCode::Char('s'));
        assert!(
            matches!(&a.modal, Some(Modal::Settings { error: Some(message), .. }) if message.contains("defaults are active") && message.contains("settings.json"))
        );
        assert_eq!(a.notice.as_deref(), Some(startup_error));
        press(&mut a, KeyCode::Enter);
        press(&mut a, KeyCode::Down);
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                error: Some(_),
                dropdown_open: false,
                ..
            })
        ));
        assert_eq!(
            a.notice.as_deref(),
            Some(startup_error),
            "row navigation must not clear startup errors"
        );
        press(&mut a, KeyCode::Esc);
        assert_eq!(a.notice.as_deref(), Some(startup_error));
        assert_eq!(a.theme, Theme::Dark);
    }
    #[test]
    fn settings_arrows_wrap_rows_and_reach_cleanup_toggles() {
        let dir = std::env::temp_dir().join(format!("ontrack-settings-nav-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        let mut a = app();
        a.settings_path = Some(path.clone());
        press(&mut a, KeyCode::Char('s'));
        press(&mut a, KeyCode::Up);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 4, .. })));
        press(&mut a, KeyCode::Down);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 0, .. })));
        press(&mut a, KeyCode::Down);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 1, .. })));
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.theme, Theme::Dark);
        assert!(a.settings.auto_merge);
        assert!(settings::load_from(&path).unwrap().auto_merge);
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none());
        assert!(a.settings.auto_merge);
        press(&mut a, KeyCode::Char('s'));
        press(&mut a, KeyCode::Up);
        press(&mut a, KeyCode::Down);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 0, .. })));
        assert_eq!(a.theme, Theme::Dark);
        assert!(a.settings.auto_merge);
        press(&mut a, KeyCode::Esc);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn settings_selector_cycles_all_catalog_entries_and_wraps_both_directions() {
        let dir =
            std::env::temp_dir().join(format!("ontrack-settings-cycle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = app();
        a.settings_path = Some(dir.join("settings.json"));
        press(&mut a, KeyCode::Char('s'));
        press(&mut a, KeyCode::Enter);
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                dropdown_open: true,
                ..
            })
        ));
        for theme in Theme::ALL
            .into_iter()
            .cycle()
            .skip(1)
            .take(Theme::ALL.len() - 1)
        {
            press(&mut a, KeyCode::Right);
            assert_eq!(a.theme, theme);
        }
        assert_eq!(a.theme, *Theme::ALL.last().unwrap());
        press(&mut a, KeyCode::Right);
        assert_eq!(
            a.theme,
            Theme::Dark,
            "forward wraps from the last catalog entry"
        );
        press(&mut a, KeyCode::Left);
        assert_eq!(
            a.theme,
            *Theme::ALL.last().unwrap(),
            "backward wraps from the first entry"
        );
        press(&mut a, KeyCode::Esc);
        assert_eq!(
            a.theme,
            *Theme::ALL.last().unwrap(),
            "closing the dropdown retains the last successfully saved theme"
        );
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                dropdown_open: false,
                ..
            })
        ));
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none());
        let reloaded = settings::load_from(a.settings_path.as_deref().unwrap()).unwrap();
        assert_eq!(
            reloaded.theme, a.settings.theme,
            "reopening reloads the committed theme"
        );
        press(&mut a, KeyCode::Char('s'));
        assert!(matches!(a.modal, Some(Modal::Settings { draft, .. }) if draft == reloaded));
        press(&mut a, KeyCode::Esc);
        let _ = std::fs::remove_dir_all(dir);
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
        assert_collapsed_header_selected(&a, "closed");
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
        assert_collapsed_header_selected(&a, "open");
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
        assert_eq!(a.selected, Some(0));
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
    fn keyboard_steps_between_expanded_rows_and_collapsed_headers() {
        let mut a = app();
        a.beads[0].status = "open".into();
        a.beads[1].status = "closed".into();
        a.beads[2].status = "closed".into();
        a.beads[3].status = "open".into();
        press(&mut a, KeyCode::Char(' '));
        assert_collapsed_header_selected(&a, "open");
        press(&mut a, KeyCode::Down);
        assert_bead(&a, "b1");
        press(&mut a, KeyCode::Char(' '));
        assert_eq!(a.collapsed.len(), 2);
        assert_collapsed_header_selected(&a, "closed");
        press(&mut a, KeyCode::Up);
        assert_collapsed_header_selected(&a, "open");
        press(&mut a, KeyCode::Char(' '));
        assert_bead(&a, "b0");
        press(&mut a, KeyCode::Down);
        assert_bead(&a, "b3");
        press(&mut a, KeyCode::Down);
        assert_collapsed_header_selected(&a, "closed");
        press(&mut a, KeyCode::Right);
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
        assert_collapsed_header_selected(&a, "open");
        assert!(a.selected.is_none());
        press(&mut a, KeyCode::Left);
        assert_collapsed_header_selected(&a, "open");
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
    fn vertical_navigation_crosses_visible_groups_without_selecting_headers() {
        let mut a = app();
        a.beads[2].status = "blocked".into();
        a.beads[3].status = "closed".into();
        a.select(1);
        press(&mut a, KeyCode::Down);
        assert_bead(&a, "b2");
        press(&mut a, KeyCode::Up);
        assert_bead(&a, "b1");
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Down);
        assert_bead(&a, "b3");
        press(&mut a, KeyCode::Down);
        assert_bead(&a, "b3");
        press(&mut a, KeyCode::Up);
        assert_bead(&a, "b2");
        press(&mut a, KeyCode::Up);
        press(&mut a, KeyCode::Up);
        assert_bead(&a, "b0");
        press(&mut a, KeyCode::Up);
        assert_bead(&a, "b0");
        assert!(a.collapse_context.is_none());
    }
    #[test]
    fn collapsed_headers_are_keyboard_targets_and_mouse_can_also_reopen() {
        let mut a = app();
        a.beads[2].status = "blocked".into();
        a.beads[3].status = "closed".into();
        a.collapsed.insert("blocked".into());
        a.select(1);
        press(&mut a, KeyCode::Down);
        assert_collapsed_header_selected(&a, "blocked");
        press(&mut a, KeyCode::Right);
        assert_bead(&a, "b2");

        a.collapsed
            .extend(["open".into(), "blocked".into(), "closed".into()]);
        a.focus_collapsed_group("open".into());
        press(&mut a, KeyCode::Up);
        assert_collapsed_header_selected(&a, "open");
        press(&mut a, KeyCode::Down);
        assert_collapsed_header_selected(&a, "blocked");
        press(&mut a, KeyCode::Down);
        assert_collapsed_header_selected(&a, "closed");
        press(&mut a, KeyCode::Down);
        assert_collapsed_header_selected(&a, "closed");
        press(&mut a, KeyCode::Up);
        assert_collapsed_header_selected(&a, "blocked");
        press(&mut a, KeyCode::Right);
        assert_bead(&a, "b2");
        assert!(!a.collapsed.contains("blocked"));
        // Mouse title clicks and keyboard controls share the same expansion transition.
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 0);
        assert_bead(&a, "b0");
        assert!(!a.collapsed.contains("open"));
    }
    #[test]
    fn details_focused_arrows_scroll_without_changing_issue_selection() {
        let mut a = app();
        a.focus_detail = true;
        a.detail_offset = 2;
        press(&mut a, KeyCode::Up);
        assert_eq!(a.detail_offset, 1);
        assert_bead(&a, "b0");
        press(&mut a, KeyCode::Down);
        assert_eq!(a.detail_offset, 2);
        assert_bead(&a, "b0");
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
        assert_eq!(a.collapse_context.as_deref(), Some("open"));
        assert!(a.selected.is_none());
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
        assert!(a.collapse_context.is_none());
    }
    #[test]
    fn closed_group_starts_collapsed_and_user_expansion_survives_refresh() {
        let mut a = unloaded_app();
        let mut items = app().beads;
        items.truncate(2);
        items[1].status = " closed ".into();
        respond(&mut a, Response::List(Ok(items)));
        assert_bead(&a, "b0");
        let layout = list_layout(&a.beads, &a.collapsed);
        assert_eq!(layout.groups.len(), 2);
        assert!(!layout.groups[0].collapsed);
        assert!(layout.groups[1].collapsed);
        assert_eq!(layout.groups[1].indices, vec![1]);
        assert!(
            !layout
                .targets
                .iter()
                .any(|target| target.row == DisplayRow::Bead(1))
        );

        press(&mut a, KeyCode::Down);
        assert_collapsed_header_selected(&a, "closed");
        press(&mut a, KeyCode::Right);
        assert_bead(&a, "b1");
        let refreshed = a.beads.clone();
        respond(&mut a, Response::List(Ok(refreshed)));
        assert_bead(&a, "b1");
        assert!(!a.collapsed.contains("closed"));
        press(&mut a, KeyCode::Left);
        assert_collapsed_header_selected(&a, "closed");
    }
    #[test]
    fn closed_group_defaults_when_it_first_appears_after_an_empty_list() {
        let mut a = unloaded_app();
        respond(&mut a, Response::List(Ok(vec![])));
        assert!(a.closed_default_pending);
        assert!(a.collapsed.is_empty());

        let mut items = app().beads;
        items.truncate(2);
        respond(&mut a, Response::List(Ok(items.clone())));
        assert!(a.closed_default_pending);
        assert_bead(&a, "b0");

        items[1].status = "closed".into();
        respond(&mut a, Response::List(Ok(items)));
        assert!(!a.closed_default_pending);
        assert!(a.collapsed.contains("closed"));
        assert_bead(&a, "b0");
    }
    #[test]
    fn closed_only_workspace_keeps_header_keyboard_accessible() {
        let mut a = unloaded_app();
        let mut items = app().beads;
        items.truncate(1);
        items[0].status = "closed".into();
        respond(&mut a, Response::List(Ok(items)));
        assert!(a.collapsed.contains("closed"));
        assert!(a.selected.is_none());
        press(&mut a, KeyCode::Down);
        assert_collapsed_header_selected(&a, "closed");
        press(&mut a, KeyCode::Right);
        assert_bead(&a, "b0");
    }
    #[test]
    fn refresh_never_selects_group_headers_or_makes_them_delete_targets() {
        let mut a = app();
        a.beads[1].status = "closed".into();
        let mut items = a.beads.clone();
        items.reverse();
        respond(&mut a, Response::List(Ok(items)));
        assert!(a.collapse_context.is_none());
        assert!(a.selected.is_some());
        press(&mut a, KeyCode::Char('d'));
        assert!(matches!(a.modal, Some(Modal::ConfirmDelete { .. })));
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
        assert!(a.collapse_context.is_none());
        let selected = a.selected.expect("refresh keeps a visible issue selected");
        assert_ne!(a.beads[selected].id, "b2");
        assert_eq!(a.detail.as_ref(), Some(&a.beads[selected].data));
        assert!(!a.collapsed.contains("closed"));
        press(&mut a, KeyCode::Char('d'));
        assert!(
            matches!(&a.modal, Some(Modal::ConfirmDelete { id, .. }) if id == &a.beads[selected].id)
        );
    }
    #[test]
    fn refresh_missing_bead_selects_visible_issue_or_leaves_no_selection() {
        let mut a = app();
        a.beads[0].status = "closed".into();
        a.beads[1].status = "blocked".into();
        a.collapsed.insert("open".into());
        a.select(0);
        let items = a.beads.iter().filter(|b| b.id != "b0").cloned().collect();
        respond(&mut a, Response::List(Ok(items)));
        assert_bead(&a, "b1");
        let mut items = a.beads.clone();
        items.iter_mut().find(|b| b.id == "b1").unwrap().status = "closed".into();
        respond(&mut a, Response::List(Ok(items)));
        assert_bead(&a, "b1");
        a.collapsed.insert("closed".into());
        let items = a
            .beads
            .iter()
            .filter(|b| b.status != "closed")
            .cloned()
            .collect();
        respond(&mut a, Response::List(Ok(items)));
        assert!(a.collapse_context.is_none());
        assert!(a.selected.is_none());
        respond(&mut a, Response::List(Ok(vec![])));
        assert!(a.collapse_context.is_none());
        assert!(a.selected.is_none());
        assert!(a.detail.is_none());
        assert!(a.collapsed.is_empty());
    }
    #[test]
    fn preferred_id_expands_destination_and_selects_created_bead() {
        {
            let mut a = app();
            a.beads[1].status = "closed".into();
            a.collapsed.insert("closed".into());
            a.preferred_id = Some("b1".into());
            let items = a.beads.clone();
            respond(&mut a, Response::List(Ok(items)));
            assert_bead(&a, "b1");
            assert!(!a.collapsed.contains("closed"));
            assert!(a.preferred_id.is_none());
        }
    }
    #[test]
    fn missing_preferred_id_falls_back_to_visible_issue_not_header() {
        let mut a = app();
        a.preferred_id = Some("missing".into());
        let items = a.beads.clone();
        respond(&mut a, Response::List(Ok(items)));
        assert_bead(&a, "b0");
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
    fn description_accepts_shortcut_letters_and_shift_enter_but_not_control_chars() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Tab);
        for c in ['q', 'r', 'n', 'd'] {
            press(&mut a, KeyCode::Char(c));
        }
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
        press(&mut a, KeyCode::Char('界'));
        a.modal_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.description, "qrnd\n界");
    }
    #[test]
    fn enter_from_description_focuses_create_without_submitting() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        press(&mut a, KeyCode::Char('n'));
        for c in "Title".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Tab);
        for c in "two words".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        press(&mut a, KeyCode::Left);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!("form closed")
        };
        assert_eq!(form.description, "two word\ns");
        assert_eq!(form.focus, 1);
        press(&mut a, KeyCode::Enter);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!("form closed")
        };
        assert_eq!(form.focus, 4);
        assert_eq!(form.description, "two word\ns");
        assert!(rx.try_recv().is_err());
        press(&mut a, KeyCode::Enter);
        let Ok(Request::Create(input)) = rx.try_recv() else {
            panic!("missing create")
        };
        assert_eq!(input.description, "two word\ns");
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
    fn escape_navigates_back_without_quitting() {
        let mut a = app();
        assert!(!a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(!a.worktrees_view);
        assert!(a.modal.is_none());

        a.worktrees_view = true;
        assert!(!a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(!a.worktrees_view);

        a.modal = Some(Modal::Settings {
            draft: a.settings,
            error: None,
            dropdown_open: true,
            focus: 0,
        });
        assert!(!a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                dropdown_open: false,
                ..
            })
        ));
        assert!(!a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(a.modal.is_none());

        a.modal = Some(Modal::ConfirmDelete {
            id: "b0".into(),
            title: "issue 0".into(),
        });
        a.mutation_busy = true;
        assert!(!a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(a.modal.is_some());
        a.mutation_busy = false;
        assert!(!a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(a.modal.is_none());

        assert!(a.key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)));
        assert!(a.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)));
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

    #[test]
    fn create_type_and_priority_cycle_with_left_right_and_enter_picker_remains_available() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Left); // Type wraps from Task to the last option.
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.issue_type, beads::BEAD_TYPES.len() - 1);
        assert!(form.dropdown.is_none());
        press(&mut a, KeyCode::Right);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.issue_type, 0);
        press(&mut a, KeyCode::Enter);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.dropdown, Some(2));
        press(&mut a, KeyCode::Left);
        assert!(
            matches!(a.modal, Some(Modal::Create(ref f)) if f.issue_type == beads::BEAD_TYPES.len() - 1)
        );
        press(&mut a, KeyCode::Esc);
        assert!(matches!(a.modal, Some(Modal::Create(ref f)) if f.issue_type == 0));

        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Right); // P2 -> P3
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.priority, 3);
        press(&mut a, KeyCode::Right);
        press(&mut a, KeyCode::Right);
        assert!(matches!(a.modal, Some(Modal::Create(ref f)) if f.priority == 0));
        press(&mut a, KeyCode::Left);
        assert!(matches!(a.modal, Some(Modal::Create(ref f)) if f.priority == 4));
        press(&mut a, KeyCode::Enter);
        assert!(matches!(a.modal, Some(Modal::Create(ref f)) if f.dropdown == Some(3)));
        press(&mut a, KeyCode::Right);
        assert!(matches!(a.modal, Some(Modal::Create(ref f)) if f.priority == 0));
        press(&mut a, KeyCode::Enter);
        assert!(
            matches!(a.modal, Some(Modal::Create(ref f)) if f.priority == 0 && f.dropdown.is_none())
        );
    }

    #[test]
    fn choice_cycling_wraps_both_directions() {
        assert_eq!(cycle_choice(0, 3, false), 2);
        assert_eq!(cycle_choice(2, 3, true), 0);
    }

    #[test]
    fn settings_autosave_failure_preserves_commit_and_same_choice_can_retry() {
        use std::fs;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "ontrack-app-settings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("settings.json");
        let mut a = app();
        a.settings_path = Some(path.clone());
        settings::save_to(
            &path,
            &Settings {
                theme: Theme::Dark,
                ..Settings::default()
            },
        )
        .unwrap();
        press(&mut a, KeyCode::Char('s'));
        // A regular file where the parent directory should be makes saving fail.
        let blocker = root.join("blocked");
        fs::write(&blocker, "preserve").unwrap();
        a.settings_path = Some(blocker.join("settings.json"));
        press(&mut a, KeyCode::Right);
        assert_eq!(
            a.settings.theme,
            Theme::Dark,
            "failed save keeps committed value"
        );
        assert_eq!(
            a.theme,
            Theme::Dark,
            "failed save leaves the active theme unchanged"
        );
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                draft: Settings {
                    theme: Theme::Light,
                    ..
                },
                error: Some(_),
                ..
            })
        ));
        assert_eq!(settings::load_from(&path).unwrap().theme, Theme::Dark);
        assert_eq!(fs::read_to_string(blocker).unwrap(), "preserve");

        press(&mut a, KeyCode::Enter);
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                dropdown_open: true,
                error: Some(_),
                ..
            })
        ));
        assert_eq!(a.theme, Theme::Dark);
        assert_eq!(settings::load_from(&path).unwrap().theme, Theme::Dark);
        press(&mut a, KeyCode::Enter);
        assert!(
            matches!(a.modal, Some(Modal::Settings { error: Some(_), .. })),
            "retry on the same candidate still reports failure"
        );

        a.settings_path = Some(path.clone());
        press(&mut a, KeyCode::Enter);
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.theme, Theme::Light);
        assert_eq!(a.settings.theme, Theme::Light);
        assert!(matches!(a.modal, Some(Modal::Settings { error: None, .. })));
        assert_eq!(settings::load_from(&path).unwrap().theme, Theme::Light);
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none());
        assert_eq!(
            a.theme,
            Theme::Light,
            "closing retains the successful choice"
        );
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn worktree_merge_requires_confirmation_and_cancel_has_no_side_effect() {
        let (mut a, rx, _) = connected_app();
        a.worktrees_view = true;
        a.worktrees.push(ManagedWorktree {
            id: "wt-1".into(),
            bead_id: "b1".into(),
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/b1".into(),
            target_branch: "main".into(),
            workspace_id: "ws1".into(),
            status: worktrees::WorktreeStatus::Unmerged,
            detail: None,
        });
        press(&mut a, KeyCode::Char('m'));
        assert!(matches!(
            a.modal,
            Some(Modal::ConfirmWorktree { action: 0, .. })
        ));
        assert!(rx.try_recv().is_err());
        press(&mut a, KeyCode::Esc);
        assert!(rx.try_recv().is_err());
        press(&mut a, KeyCode::Char('m'));
        press(&mut a, KeyCode::Char('y'));
        assert!(
            matches!(rx.try_recv(), Ok(Request::WorktreeAction(id, WorktreeAction::RequestMerge)) if id == "wt-1")
        );
    }
    #[test]
    fn worktree_worker_polls_without_queue_growth_and_surfaces_errors() {
        let (mut a, rx, tx) = connected_app();
        a.worktrees_view = true;
        let now = Instant::now();
        a.poll_worktrees(now);
        assert!(matches!(rx.try_recv(), Ok(Request::ListWorktrees)));
        a.poll_worktrees(now + REFRESH_INTERVAL * 10);
        assert!(rx.try_recv().is_err(), "one in-flight request is enough");
        tx.send(Response::Worktrees(Err(std::io::Error::other(
            "registry unavailable",
        ))))
        .unwrap();
        a.receive_at(now);
        assert_eq!(a.worktrees_error.as_deref(), Some("registry unavailable"));
        assert!(!a.worktrees_loading);
        a.poll_worktrees(now + REFRESH_INTERVAL);
        assert!(matches!(rx.try_recv(), Ok(Request::ListWorktrees)));
    }
    #[test]
    fn worktree_detail_scroll_is_separate_and_resets_on_selection() {
        let (mut app, _, _) = connected_app();
        app.worktrees_view = true;
        app.detail_rect = Rect::new(40, 2, 40, 12);
        app.worktrees = (0..2)
            .map(|index| ManagedWorktree {
                id: format!("wt-{index}"),
                bead_id: format!("bead-{index}"),
                handoff_accepted: true,
                bead_title: Some(format!("Title {index}")),
                bead_description: Some("A long description".into()),
                branch: format!("ontrack/bead-{index}"),
                target_branch: "main".into(),
                workspace_id: format!("ws-{index}"),
                status: worktrees::WorktreeStatus::Unmerged,
                detail: None,
            })
            .collect();
        press(&mut app, KeyCode::PageDown);
        assert_eq!(app.worktree_detail_offset, 10);
        assert_eq!(app.detail_offset, 0);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.selected_worktree, 1);
        assert_eq!(app.worktree_detail_offset, 0);
    }
    #[test]
    fn worktree_policy_reconciles_outside_worktrees_view() {
        let (mut a, rx, _) = connected_app();
        a.settings.auto_merge = true;
        a.poll_worktrees(Instant::now());
        assert!(matches!(
            rx.try_recv(),
            Ok(Request::ReconcileWorktrees(CleanupPolicy {
                auto_merge: true,
                ..
            }))
        ));
        assert!(!a.worktrees_view);
    }
    #[test]
    fn closed_bead_does_not_remove_its_managed_worktree() {
        let (mut a, _, tx) = connected_app();
        a.beads[0].status = "closed".into();
        let item = ManagedWorktree {
            id: "tracked-closed".into(),
            bead_id: a.beads[0].id.clone(),
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/b0".into(),
            target_branch: "main".into(),
            workspace_id: "ws".into(),
            status: worktrees::WorktreeStatus::Unmerged,
            detail: None,
        };
        tx.send(Response::Worktrees(Ok(vec![item.clone()])))
            .unwrap();
        a.receive_at(Instant::now());
        assert_eq!(a.worktrees.len(), 1);
        assert_eq!(a.worktrees[0].id, item.id);
        assert_eq!(a.beads[0].status, "closed");
    }
    #[test]
    fn older_worktree_snapshot_cannot_overwrite_action_result() {
        let (mut a, _, tx) = connected_app();
        let before = ManagedWorktree {
            id: "wt".into(),
            bead_id: "b0".into(),
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/b0".into(),
            target_branch: "main".into(),
            workspace_id: "ws".into(),
            status: worktrees::WorktreeStatus::Unmerged,
            detail: None,
        };
        a.worktrees = vec![before.clone()];
        a.worktrees_inflight = true;
        a.worktree_action_pending = true;
        let after = ManagedWorktree {
            status: worktrees::WorktreeStatus::Integrated,
            detail: None,
            ..before
        };
        tx.send(Response::Worktrees(Ok(vec![a.worktrees[0].clone()])))
            .unwrap();
        tx.send(Response::WorktreeChanged(Ok(after.clone())))
            .unwrap();
        a.receive_at(Instant::now());
        assert_eq!(a.worktrees.len(), 1);
        assert_eq!(a.worktrees[0].status, worktrees::WorktreeStatus::Integrated);
    }
}
