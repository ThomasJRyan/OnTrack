use crate::herdr::{Agent, LaunchContext, LaunchOutcome};
use crate::worktrees::{self, Action as WorktreeAction, CleanupPolicy, ManagedWorktree};
use crate::{
    Tui,
    beads::{self, Bead, CreateBead, UpdateBead},
    event,
    hotkeys::{self, Help},
    screenshots,
    settings::{self, DefaultAgent, Settings, Theme},
    ui,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn spawn_epic_planning(id: String, replies: mpsc::Sender<Response>) {
    thread::spawn(move || {
        let session = match crate::epic::launch(&id) {
            Ok(session) => session,
            Err(error) => {
                let _ = replies.send(Response::EpicLaunched(id, Err(error)));
                return;
            }
        };
        if replies
            .send(Response::EpicLaunched(id.clone(), Ok(session.clone())))
            .is_ok()
        {
            let result = crate::epic::finish(&session, &id);
            let _ = replies.send(Response::EpicFinished(id, result));
        }
    });
}

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub(super) enum Request {
    List,
    Show(String, u64),
    LoadEdit(String, u64),
    Create(CreateBead),
    CreateEpic(CreateBead, Option<String>),
    GenerateEpic(String),
    CreateChild(CreateBead, String),
    Update(UpdateBead),
    SetReady(String, bool),
    SetEpicReady(String, bool, HashSet<String>, HashSet<String>),
    Delete(String),
    DiscoverHarnesses(u64),
    Discover(u64),
    Launch(u64, LaunchContext, Agent, Bead),
    LaunchReady(u64, LaunchContext, Agent, Bead),
    ListWorktrees,
    FocusWorkspace(String),
    WorktreeAction(String, WorktreeAction),
    RestartWorktree(String),
    ContinueWorktree(String),
    ReconcileWorktrees(CleanupPolicy),
    ObserveAgents,
    ReadScreenshot(u64),
    PlanLaunch(u64, String, String),
    PlanRead(u64, crate::epic::Session, String),
    PlanReply(u64, crate::epic::Session, String, String),
    PlanCreate(u64, crate::planning::Plan),
}
pub(super) enum Response {
    List(std::io::Result<Vec<Bead>>),
    Show(String, u64, std::io::Result<serde_json::Value>),
    EditLoaded(String, u64, std::io::Result<serde_json::Value>),
    Created(std::io::Result<String>),
    EpicLaunched(String, std::io::Result<crate::epic::Session>),
    EpicFinished(String, std::io::Result<Vec<String>>),
    Updated(String, std::io::Result<()>),
    ReadySet(String, bool, std::io::Result<Vec<String>>),
    Deleted(std::io::Result<()>),
    HarnessesDiscovered(u64, std::io::Result<Vec<String>>),
    Discovered(u64, std::io::Result<LaunchContext>),
    Launched(u64, std::io::Result<LaunchOutcome>),
    Worktrees(std::io::Result<Vec<ManagedWorktree>>),
    WorkspaceFocused(String, std::io::Result<()>),
    WorktreeChanged(std::io::Result<ManagedWorktree>),
    WorktreeRestarted(std::io::Result<String>),
    WorktreeContinued(std::io::Result<String>),
    AgentsObserved(std::io::Result<Vec<worktrees::AgentDisplay>>),
    ScreenshotRead(u64, std::io::Result<Vec<u8>>),
    PlanLaunched(u64, std::io::Result<crate::epic::Session>),
    PlanRead(
        u64,
        String,
        std::io::Result<Option<crate::planning::Message>>,
    ),
    PlanReplied(u64, String, std::io::Result<()>),
    PlanCreated(u64, Result<Vec<String>, String>),
}
pub(super) enum WorktreeFocusFeedback {
    Success(String),
    Error(String),
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
    pub(super) edit_seq: u64,
    pub(super) harness_seq: u64,
    pub(super) launch_seq: u64,
    pub(super) handoff: HashMap<u64, PendingHandoff>,
    /// Accepted agent starts not yet acknowledged by a fresh durable scan.
    pub(crate) handoff_capacity: HashSet<String>,
    pub toast: Option<Toast>,
    pub list_rect: Rect,
    pub detail_rect: Rect,
    pub modal: Option<Modal>,
    pub help: Option<Help>,
    pub settings: Settings,
    pub theme: Theme,
    pub settings_path: Option<PathBuf>,
    pub settings_load_error: Option<String>,
    pub worktrees: Vec<ManagedWorktree>,
    /// Accepted handoffs include removed checkouts, which are hidden from `worktrees`.
    pub accepted_handoffs: HashSet<String>,
    pub(crate) known_handoffs: HashSet<String>,
    pub(crate) scheduler_attempted: HashSet<String>,
    pub selected_worktree: usize,
    pub worktree_offset: usize,
    pub worktree_detail_offset: u16,
    pub worktrees_view: bool,
    pub worktrees_loading: bool,
    pub worktrees_error: Option<String>,
    pub(super) worktrees_focus_feedback: Option<WorktreeFocusFeedback>,
    pub(super) worktrees_focus_pending: bool,
    pub worktrees_inflight: bool,
    pub worktrees_discovered: bool,
    pub worktrees_refresh_pending: bool,
    pub worktree_action_pending: bool,
    pub worktrees_next_refresh: Instant,
    pub(crate) agent_observation_inflight: bool,
    pub(crate) next_agent_observation: Instant,
    pub notice: Option<String>,
    pub mutation_busy: bool,
    /// Ready keeps mutation exclusion without locking browser input.
    pub ready_pending: bool,
    pub planning_epics: HashSet<String>,
    pub(super) planning_seq: u64,
    pub modal_ready: bool,
    pub(crate) paste_seq: u64,
    pub(crate) paste_pending: bool,
    pub(crate) created_screenshots: Vec<PathBuf>,
    pub preferred_id: Option<String>,
    pub(super) refresh_inflight: bool,
    pub(super) refresh_pending: bool,
    pub(super) next_refresh: Instant,
    pub(super) requests: mpsc::Sender<Request>,
    pub(super) responses: mpsc::Receiver<Response>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Planning(PlanningForm),
    Create(CreateForm),
    EditLoading {
        id: String,
        seq: u64,
        error: Option<String>,
    },
    Edit {
        id: String,
        form: CreateForm,
    },
    ConfirmGenerateEpic {
        id: String,
        title: String,
    },
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
    ConfirmWorktree {
        id: String,
        action: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanningStage {
    Draft,
    Waiting,
    Question { text: String, options: Vec<String> },
    Review(crate::planning::Plan),
    Revision,
    Creating,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningForm {
    pub seq: u64,
    pub proposal: String,
    pub input: String,
    pub marker: String,
    pub round: u64,
    pub session: Option<crate::epic::Session>,
    pub stage: PlanningStage,
    pub selected: usize,
    pub poll_inflight: bool,
    pub next_poll: Instant,
    pub scroll: u16,
}

impl PlanningForm {
    fn new(seq: u64) -> Self {
        Self {
            seq,
            proposal: String::new(),
            input: String::new(),
            marker: format!(
                "ONTRACK_PLAN_{seq}_{:x}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
            round: 0,
            session: None,
            stage: PlanningStage::Draft,
            selected: 0,
            poll_inflight: false,
            next_poll: Instant::now(),
            scroll: 0,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub error: bool,
    pub expires_at: Option<Instant>,
}
impl Toast {
    fn progress(message: String) -> Self {
        Self {
            message,
            error: false,
            expires_at: None,
        }
    }
    fn result(message: String, error: bool) -> Self {
        Self {
            message,
            error,
            expires_at: Some(Instant::now() + Duration::from_secs(if error { 15 } else { 7 })),
        }
    }
    pub fn visible(&self, now: Instant) -> bool {
        self.expires_at.is_none_or(|expires| now < expires)
    }
}
#[derive(Debug, Clone)]
pub(super) struct PendingHandoff {
    seq: u64,
    bead: Bead,
    launch_queued: bool,
    scheduled: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateForm {
    pub parent_id: Option<String>,
    pub title: String,
    pub description: String,
    pub issue_type: usize,
    pub priority: usize,
    pub harness: usize,
    pub harnesses: Vec<String>,
    pub harness_loading: bool,
    pub harness_error: Option<String>,
    pub harness_seq: u64,
    pub previous_harness: Option<String>,
    pub generate_children: bool,
    pub focus: usize,
    pub cursor: usize,
    pub description_cursor: usize,
    pub description_preferred_col: Option<usize>,
    pub dropdown: Option<usize>,
    pub dropdown_original: Option<usize>,
}
impl Default for CreateForm {
    fn default() -> Self {
        Self {
            parent_id: None,
            title: String::new(),
            description: String::new(),
            issue_type: 0,
            priority: 2,
            harness: 0,
            harnesses: Vec::new(),
            harness_loading: false,
            harness_error: None,
            harness_seq: 0,
            previous_harness: None,
            generate_children: false,
            focus: 0,
            cursor: 0,
            description_cursor: 0,
            description_preferred_col: None,
            dropdown: None,
            dropdown_original: None,
        }
    }
}

impl CreateForm {
    fn insert_screenshot(&mut self, path: &std::path::Path) {
        let reference = screenshots::markdown_reference(path);
        let cursor = self.description_cursor.min(self.description.len());
        self.description.insert_str(cursor, &reference);
        self.description_cursor = cursor + reference.len();
        self.description_preferred_col = None;
    }

    fn set_harnesses(&mut self, harnesses: Vec<String>, preferred: DefaultAgent) {
        self.harnesses = harnesses;
        self.harness = match preferred {
            DefaultAgent::Unassigned => 0,
            DefaultAgent::Kind(kind) => self
                .harnesses
                .iter()
                .position(|available| available == kind)
                .map(|index| index + 1)
                .unwrap_or_else(|| self.automatic_harness()),
            DefaultAgent::Automatic => self.automatic_harness(),
        };
        self.harness_loading = false;
        self.harness_error = None;
        self.dropdown = None;
        self.dropdown_original = None;
    }

    fn automatic_harness(&self) -> usize {
        self.harnesses
            .iter()
            .position(|kind| kind == "codex")
            .map_or_else(
                || usize::from(!self.harnesses.is_empty()),
                |index| index + 1,
            )
    }
}

fn edit_form(record: &serde_json::Value) -> Result<CreateForm, String> {
    let title = record
        .get("title")
        .and_then(serde_json::Value::as_str)
        .filter(|title| !title.trim().is_empty())
        .ok_or("Bead has no editable title")?;
    let issue_type = record
        .get("issue_type")
        .and_then(serde_json::Value::as_str)
        .and_then(|kind| beads::BEAD_TYPES.iter().position(|known| *known == kind))
        .ok_or("Bead type is missing or unsupported by the edit form")?;
    let priority = record
        .get("priority")
        .and_then(serde_json::Value::as_u64)
        .filter(|priority| *priority <= 4)
        .ok_or("Bead priority is missing or outside P0–P4")? as usize;
    let description = match record.get("description") {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(value)) => value.clone(),
        _ => return Err("Bead description is not text".into()),
    };
    Ok(CreateForm {
        title: title.into(),
        cursor: title.len(),
        description_cursor: description.len(),
        description,
        issue_type,
        priority,
        harness: beads::harness(record)
            .and_then(|kind| beads::HARNESSES.iter().position(|known| *known == kind))
            .map_or(0, |index| index + 1),
        harnesses: beads::HARNESSES
            .iter()
            .map(|kind| (*kind).to_owned())
            .collect(),
        previous_harness: beads::harness(record).map(str::to_owned),
        ..Default::default()
    })
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
fn execute_request(request: Request, out: &mpsc::Sender<Response>) {
    let response = match request {
        Request::List => Response::List(beads::list_beads()),
        Request::Show(id, seq) => Response::Show(id.clone(), seq, beads::show_bead(&id)),
        Request::LoadEdit(id, seq) => Response::EditLoaded(id.clone(), seq, beads::show_bead(&id)),
        Request::Create(input) => Response::Created(beads::create_bead(&input)),
        Request::CreateEpic(input, parent) => match parent.as_deref().map_or_else(
            || beads::create_bead(&input),
            |id| beads::create_child(&input, id),
        ) {
            Ok(id) => {
                if out.send(Response::Created(Ok(id.clone()))).is_err() {
                    return;
                }
                spawn_epic_planning(id, out.clone());
                return;
            }
            Err(error) => Response::Created(Err(error)),
        },
        Request::GenerateEpic(id) => {
            spawn_epic_planning(id, out.clone());
            return;
        }
        Request::CreateChild(input, parent) => {
            Response::Created(beads::create_child(&input, &parent))
        }
        Request::Update(input) => Response::Updated(input.id.clone(), beads::update_bead(&input)),
        Request::ReadScreenshot(seq) => {
            Response::ScreenshotRead(seq, screenshots::read_clipboard())
        }
        Request::PlanLaunch(seq, proposal, marker) => {
            Response::PlanLaunched(seq, crate::planning::launch(&proposal, &marker))
        }
        Request::PlanRead(seq, session, marker) => {
            let result = crate::planning::read(&session, &marker);
            Response::PlanRead(seq, marker, result)
        }
        Request::PlanReply(seq, session, message, marker) => {
            let result = crate::planning::reply(&session, &message, &marker);
            Response::PlanReplied(seq, marker, result)
        }
        Request::PlanCreate(seq, plan) => {
            Response::PlanCreated(seq, crate::planning::create(&plan))
        }
        Request::SetReady(id, ready) => Response::ReadySet(
            id.clone(),
            ready,
            beads::set_ready(&id, ready).map(|()| vec![id]),
        ),
        Request::SetEpicReady(id, ready, known_handoffs, protected_handoffs) => Response::ReadySet(
            id.clone(),
            ready,
            beads::set_epic_ready(&id, ready, &known_handoffs, &protected_handoffs),
        ),
        Request::Delete(id) => Response::Deleted(beads::delete_bead(&id)),
        Request::DiscoverHarnesses(seq) => {
            Response::HarnessesDiscovered(seq, crate::herdr::installed_harnesses())
        }
        Request::Discover(seq) => Response::Discovered(seq, crate::herdr::discover()),
        Request::Launch(seq, context, agent, bead) => {
            Response::Launched(seq, crate::herdr::launch(&context, &agent, &bead))
        }
        Request::LaunchReady(seq, context, agent, bead) => {
            Response::Launched(seq, crate::herdr::launch_ready(&context, &agent, &bead))
        }
        Request::ListWorktrees => Response::Worktrees(worktrees::list()),
        Request::FocusWorkspace(id) => {
            Response::WorkspaceFocused(id.clone(), crate::herdr::focus_workspace(&id))
        }
        Request::WorktreeAction(id, action) => {
            Response::WorktreeChanged(worktrees::act(&id, action))
        }
        Request::RestartWorktree(id) => Response::WorktreeRestarted(worktrees::restart(&id)),
        Request::ContinueWorktree(id) => Response::WorktreeContinued(worktrees::continue_task(&id)),
        Request::ReconcileWorktrees(policy) => Response::Worktrees(worktrees::reconcile(policy)),
        Request::ObserveAgents => Response::AgentsObserved(worktrees::observe_agents()),
    };
    let _ = out.send(response);
}

/// Browser bd, managed work, and independent Herdr jobs never share a FIFO.
/// Detached jobs are not joined or cancelled when the terminal app exits.
fn start_workers(
    execute: impl Fn(Request, &mpsc::Sender<Response>) + Send + Sync + 'static,
) -> (
    mpsc::Sender<Request>,
    mpsc::Receiver<Response>,
    mpsc::Sender<Response>,
) {
    let execute = Arc::new(execute);
    let (requests, incoming) = mpsc::channel();
    let (out, responses) = mpsc::channel();
    let (browser, browser_in) = mpsc::channel();
    let (managed, managed_in) = mpsc::channel();
    let (ready, ready_in) = mpsc::channel();
    for lane in [browser_in, managed_in, ready_in] {
        let execute = execute.clone();
        let out = out.clone();
        thread::spawn(move || {
            while let Ok(request) = lane.recv() {
                execute(request, &out);
            }
        });
    }
    let replies = out.clone();
    thread::spawn(move || {
        while let Ok(request) = incoming.recv() {
            if matches!(
                request,
                Request::Discover(_)
                    | Request::PlanLaunch(..)
                    | Request::PlanRead(..)
                    | Request::PlanReply(..)
                    | Request::DiscoverHarnesses(_)
                    | Request::Launch(..)
                    | Request::LaunchReady(..)
                    | Request::FocusWorkspace(_)
                    | Request::ReadScreenshot(_)
                    | Request::ObserveAgents
            ) {
                let execute = execute.clone();
                let out = replies.clone();
                thread::spawn(move || execute(request, &out));
            } else if matches!(
                request,
                Request::ListWorktrees
                    | Request::WorktreeAction(..)
                    | Request::RestartWorktree(_)
                    | Request::ContinueWorktree(_)
                    | Request::ReconcileWorktrees(_)
            ) {
                if managed.send(request).is_err() {
                    break;
                }
            } else if matches!(request, Request::SetReady(..) | Request::SetEpicReady(..)) {
                if ready.send(request).is_err() {
                    break;
                }
            } else if browser.send(request).is_err() {
                break;
            }
        }
    });
    (requests, responses, out)
}

impl App {
    fn new() -> Self {
        let (tx, responses, _) = start_workers(execute_request);
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
            edit_seq: 0,
            harness_seq: 0,
            launch_seq: 0,
            handoff: HashMap::new(),
            handoff_capacity: HashSet::new(),
            toast: None,
            list_rect: Rect::default(),
            detail_rect: Rect::default(),
            modal: None,
            help: None,
            settings,
            theme,
            settings_path,
            settings_load_error: settings_error.clone(),
            worktrees: Vec::new(),
            accepted_handoffs: HashSet::new(),
            known_handoffs: HashSet::new(),
            scheduler_attempted: HashSet::new(),
            selected_worktree: 0,
            worktree_offset: 0,
            worktree_detail_offset: 0,
            worktrees_view: false,
            worktrees_loading: false,
            worktrees_error: None,
            worktrees_focus_feedback: None,
            worktrees_focus_pending: false,
            worktrees_inflight: false,
            worktrees_discovered: false,
            worktrees_refresh_pending: false,
            worktree_action_pending: false,
            worktrees_next_refresh: Instant::now(),
            agent_observation_inflight: false,
            next_agent_observation: Instant::now(),
            notice: settings_error,
            mutation_busy: false,
            ready_pending: false,
            planning_epics: HashSet::new(),
            planning_seq: 0,
            modal_ready: true,
            paste_seq: 0,
            paste_pending: false,
            created_screenshots: Vec::new(),
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
    fn poll_planning(&mut self, now: Instant) {
        let Some(Modal::Planning(form)) = self.modal.as_mut() else {
            return;
        };
        if !matches!(form.stage, PlanningStage::Waiting)
            || form.poll_inflight
            || now < form.next_poll
        {
            return;
        }
        let Some(session) = form.session.clone() else {
            return;
        };
        form.poll_inflight = true;
        form.next_poll = now + Duration::from_secs(2);
        if self
            .requests
            .send(Request::PlanRead(form.seq, session, form.marker.clone()))
            .is_err()
        {
            form.poll_inflight = false;
            form.stage = PlanningStage::Failed("Agent output read could not be queued".into());
        }
    }
    fn poll_scheduler(&mut self) {
        if !self.worktrees_discovered || self.mutation_busy {
            return;
        }
        // A child must branch from its managed parent checkout. Preparing an
        // Epic only reserves that checkout; it does not occupy an agent slot.
        if let Some(epic) = self
            .beads
            .iter()
            .find(|bead| {
                if !beads::is_ready(bead)
                    || bead.data["issue_type"] != "epic"
                    || self.scheduler_attempted.contains(&bead.id)
                    || self.known_handoffs.contains(&bead.id)
                    || self.worktrees.iter().any(|item| item.bead_id == bead.id)
                    || self
                        .handoff
                        .values()
                        .any(|pending| pending.bead.id == bead.id)
                {
                    return false;
                }
                let descendants = beads::open_descendants_in(&self.beads, &bead.id);
                self.beads.iter().any(|child| {
                    beads::is_ready(child)
                        && child.data["issue_type"] != "epic"
                        && !beads::explicitly_unassigned(&child.data)
                        && (!beads::has_harness_label(&child.data)
                            || beads::harness(&child.data).is_some())
                        && !self.known_handoffs.contains(&child.id)
                        && descendants.contains(&child.id)
                })
            })
            .cloned()
        {
            self.scheduler_attempted.insert(epic.id.clone());
            self.start_launch(epic, false);
            return;
        }
        let active_agents =
            self.worktrees
                .iter()
                .filter(|item| {
                    item.handoff_accepted
                        && matches!(
                            item.status,
                            worktrees::WorktreeStatus::InProgress
                                | worktrees::WorktreeStatus::DirtyCheckout
                        )
                })
                .count()
                + self
                    .handoff_capacity
                    .iter()
                    .filter(|id| {
                        !self
                            .worktrees
                            .iter()
                            .any(|item| &item.bead_id == *id && item.handoff_accepted)
                    })
                    .count()
                + self
                    .handoff
                    .values()
                    .filter(|pending| {
                        // Reserving a parent checkout for open descendants starts no
                        // agent. Resuming that reserved parent after its children does.
                        !self.reserves_parent(&pending.bead)
                            && !self.worktrees.iter().any(|item| {
                                item.bead_id == pending.bead.id && item.handoff_accepted
                            })
                    })
                    .count();
        if !self.settings.agent_limit.allows(active_agents) {
            return;
        }
        let next = self
            .beads
            .iter()
            .filter(|bead| {
                beads::is_ready(bead)
                    && bead.data["issue_type"] != "epic"
                    && self.scheduler_eligible(bead)
                    && !self.scheduler_attempted.contains(&bead.id)
            })
            .min_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.id.cmp(&b.id)))
            .cloned();
        if let Some(bead) = next {
            self.scheduler_attempted.insert(bead.id.clone());
            if beads::explicitly_unassigned(&bead.data) {
                let message = format!(
                    "{} is Ready but explicitly Unassigned; edit it to select an agent",
                    bead.id
                );
                self.notice = Some(message.clone());
                self.toast = Some(Toast::result(message, true));
            } else if beads::has_harness_label(&bead.data) && beads::harness(&bead.data).is_none() {
                let message = format!(
                    "{} has an unsupported agent harness; edit it to select one",
                    bead.id
                );
                self.notice = Some(message.clone());
                self.toast = Some(Toast::result(message, true));
            } else {
                self.start_launch(bead, true);
            }
        }
    }
    fn scheduler_eligible(&self, bead: &Bead) -> bool {
        if self
            .handoff
            .values()
            .any(|pending| pending.bead.id == bead.id)
        {
            return false;
        }
        if let Some(parent) = bead.data["parent"].as_str()
            && !self.worktrees.iter().any(|item| {
                item.bead_id == parent
                    && matches!(
                        item.status,
                        worktrees::WorktreeStatus::Reserved | worktrees::WorktreeStatus::InProgress
                    )
            })
        {
            return false;
        }
        if !self.known_handoffs.contains(&bead.id) {
            return true;
        }
        let Some(reserved) = self.worktrees.iter().find(|item| {
            item.bead_id == bead.id && item.status == worktrees::WorktreeStatus::Reserved
        }) else {
            return false;
        };
        beads::open_descendants_in(&self.beads, &bead.id).is_empty()
            && !self.worktrees.iter().any(|item| {
                item.parent_bead_id.as_deref() == Some(bead.id.as_str())
                    && !matches!(
                        item.status,
                        worktrees::WorktreeStatus::Integrated
                            | worktrees::WorktreeStatus::WorkspaceClosed
                            | worktrees::WorktreeStatus::CheckoutRemoved
                    )
            })
            && !reserved.workspace_id.is_empty()
    }
    fn reserves_parent(&self, bead: &Bead) -> bool {
        !beads::open_descendants_in(&self.beads, &bead.id).is_empty()
            || self.worktrees.iter().any(|item| {
                item.parent_bead_id.as_deref() == Some(bead.id.as_str())
                    && !matches!(
                        item.status,
                        worktrees::WorktreeStatus::Integrated
                            | worktrees::WorktreeStatus::WorkspaceClosed
                            | worktrees::WorktreeStatus::CheckoutRemoved
                    )
            })
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
            // Only the initial scan hides empty-state guidance. A refresh or
            // retry keeps the settled view visible while the request is in flight.
            self.worktrees_loading = !self.worktrees_discovered && self.worktrees_error.is_none();
            let policy = CleanupPolicy {
                auto_merge: self.settings.auto_merge,
                auto_merge_children: self.settings.auto_merge_children,
                auto_close_workspace: self.settings.auto_close_workspace,
                auto_remove_checkout: self.settings.auto_remove_checkout,
            };
            let request = if !self.handoff.is_empty()
                || !self.worktrees_discovered
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
    fn poll_agent_observations(&mut self, now: Instant) {
        if self.agent_observation_inflight
            || self.worktrees.is_empty()
            || now < self.next_agent_observation
        {
            return;
        }
        self.agent_observation_inflight = true;
        if self.requests.send(Request::ObserveAgents).is_err() {
            self.agent_observation_inflight = false;
            self.next_agent_observation = now + Duration::from_secs(2);
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
        let status = display_status(&self.beads[index]).to_owned();
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
                DisplayRow::Group(status)
                    if self.collapsed.contains(&status)
                        || !self.beads.iter().any(|bead| display_status(bead) == status) =>
                {
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
            .map(|b| display_status(b).to_owned())
            .or_else(|| self.collapse_context.clone())
    }
    fn focus_collapsed_group(&mut self, status: String) {
        self.clear_selection();
        self.collapse_context = Some(status);
    }
    fn set_group_collapsed(&mut self, status: String, collapsed: bool) {
        if !group_statuses(&self.beads).contains(&status) {
            return;
        }
        let is_collapsed = self.collapsed.contains(&status);
        if is_collapsed == collapsed {
            return;
        }
        let selected_in_group = self
            .selected
            .is_some_and(|i| display_status(&self.beads[i]) == status);
        if collapsed {
            self.collapsed.insert(status.clone());
            if selected_in_group {
                self.focus_collapsed_group(status.clone());
            }
        } else {
            self.collapsed.remove(&status);
            if let Some(i) = self.beads.iter().position(|b| display_status(b) == status) {
                self.select(i);
            } else {
                self.focus_collapsed_group(status);
            }
        }
    }
    fn toggle_group(&mut self, status: String) {
        if self.collapsed.remove(&status) {
            if let Some(i) = self.beads.iter().position(|b| display_status(b) == status) {
                self.select(i);
            } else {
                self.focus_collapsed_group(status);
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
                                status_order(display_status(a))
                                    .cmp(&status_order(display_status(b)))
                                    .then_with(|| a.status.cmp(&b.status))
                                    .then_with(|| a.id.cmp(&b.id))
                            });
                            if self.closed_default_pending {
                                self.collapsed.insert("closed".into());
                                self.closed_default_pending = false;
                            }
                            let groups = group_statuses(&self.beads);
                            self.collapsed.retain(|s| groups.contains(s));
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
                                self.collapsed.remove(display_status(&self.beads[i]));
                                if Some(i) == same_id {
                                    self.selected = Some(i);
                                    self.refresh_detail();
                                } else {
                                    self.select(i);
                                }
                            } else if let Some(status) = collapse_context.filter(|status| {
                                groups.contains(status)
                                    && (self.collapsed.contains(status)
                                        || !self
                                            .beads
                                            .iter()
                                            .any(|bead| display_status(bead) == status))
                            }) {
                                self.focus_collapsed_group(status);
                            } else if let Some(i) = same_id {
                                self.collapse_context = None;
                                if self.collapsed.contains(display_status(&self.beads[i])) {
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
                Response::EditLoaded(id, seq, result) => {
                    if let Some(Modal::EditLoading {
                        id: pending,
                        seq: active,
                        error,
                    }) = self.modal.as_mut()
                        && *pending == id
                        && *active == seq
                    {
                        match result
                            .map_err(|e| e.to_string())
                            .and_then(|v| edit_form(&v))
                        {
                            Ok(form) => self.modal = Some(Modal::Edit { id, form }),
                            Err(message) => *error = Some(message),
                        }
                    }
                }
                Response::Created(result) => {
                    self.mutation_busy = false;
                    match result {
                        Ok(id) => {
                            if matches!(&self.modal, Some(Modal::Create(form)) if form.generate_children && beads::BEAD_TYPES[form.issue_type] == "epic")
                            {
                                self.planning_epics.insert(id.clone());
                                self.toast = Some(Toast::progress(format!(
                                    "Created Epic {id}; starting sub-task agent…"
                                )));
                            }
                            self.notice = Some(format!("Created {id}"));
                            self.created_screenshots.clear();
                            self.modal = None;
                            self.preferred_id = Some(id);
                            self.refresh_at(now);
                        }
                        Err(e) => {
                            self.notice = Some(format!("Create failed: {e}"));
                        }
                    }
                }
                Response::EpicLaunched(id, result) => match result {
                    Ok(session) => {
                        self.planning_epics.insert(id.clone());
                        let message = format!(
                            "Planning {id} in Herdr workspace {} tab {}…",
                            session.workspace_id, session.tab_id
                        );
                        self.notice = Some(message.clone());
                        self.toast = Some(Toast::progress(message));
                    }
                    Err(error) => {
                        self.planning_epics.remove(&id);
                        let message = format!("Epic {id}: planning failed: {error}");
                        self.notice = Some(message.clone());
                        self.toast = Some(Toast::result(message, true));
                    }
                },
                Response::EpicFinished(id, result) => {
                    self.planning_epics.remove(&id);
                    let (message, error) = match result {
                        Ok(children) => (
                            format!(
                                "Epic {id}: {} child Beads created; planning tab closed",
                                children.len()
                            ),
                            false,
                        ),
                        Err(error) => (
                            format!("Epic {id}: planning needs attention: {error}"),
                            true,
                        ),
                    };
                    self.notice = Some(message.clone());
                    self.toast = Some(Toast::result(message, error));
                    self.refresh_at(now);
                }
                Response::PlanLaunched(seq, result) => {
                    if let Some(Modal::Planning(form)) = self.modal.as_mut()
                        && form.seq == seq
                    {
                        match result {
                            Ok(session) => {
                                form.session = Some(session);
                                form.next_poll = now;
                            }
                            Err(error) => form.stage = PlanningStage::Failed(error.to_string()),
                        }
                    } else if let Ok(session) = result {
                        self.notice = Some(format!(
                            "Cancelled planning started in Herdr tab {}; inspect or close it there",
                            session.tab_id
                        ));
                    }
                }
                Response::PlanRead(seq, marker, result) => {
                    if let Some(Modal::Planning(form)) = self.modal.as_mut()
                        && form.seq == seq
                        && form.marker == marker
                    {
                        form.poll_inflight = false;
                        match result {
                            Ok(Some(crate::planning::Message::Question { text, options })) => {
                                form.stage = PlanningStage::Question { text, options };
                                form.selected = 0;
                                form.input.clear();
                                form.scroll = 0;
                            }
                            Ok(Some(crate::planning::Message::Plan(plan))) => {
                                form.stage = PlanningStage::Review(plan);
                                form.scroll = 0;
                            }
                            Ok(None) => {}
                            Err(error) => form.stage = PlanningStage::Failed(error.to_string()),
                        }
                    }
                }
                Response::PlanReplied(seq, marker, result) => {
                    if let Some(Modal::Planning(form)) = self.modal.as_mut()
                        && form.seq == seq
                        && form.marker == marker
                    {
                        if let Err(error) = result {
                            form.stage = PlanningStage::Failed(error.to_string());
                        } else {
                            form.next_poll = now;
                        }
                    }
                }
                Response::PlanCreated(seq, result) => {
                    if !matches!(self.modal, Some(Modal::Planning(ref form)) if form.seq == seq) {
                        continue;
                    }
                    self.mutation_busy = false;
                    match result {
                        Ok(ids) => {
                            self.notice =
                                Some(format!("Created planned Beads: {}", ids.join(", ")));
                            self.preferred_id = ids.first().cloned();
                            self.modal = None;
                        }
                        Err(error) => {
                            self.notice = Some(format!("Planning creation failed: {error}"));
                            if let Some(Modal::Planning(form)) = self.modal.as_mut() {
                                form.stage = PlanningStage::Failed(error);
                            }
                        }
                    }
                    self.refresh_at(now);
                }
                Response::Updated(id, result) => {
                    self.mutation_busy = false;
                    match result {
                        Ok(()) => {
                            self.notice = Some(format!("Updated {id}"));
                            self.created_screenshots.clear();
                            self.modal = None;
                            self.preferred_id = Some(id);
                            self.refresh_at(now);
                        }
                        Err(e) => self.notice = Some(format!("Update failed: {e}")),
                    }
                }
                Response::ScreenshotRead(seq, result) => {
                    if seq != self.paste_seq {
                        continue;
                    }
                    self.paste_pending = false;
                    if let Some(Modal::Create(form) | Modal::Edit { form, .. }) =
                        self.modal.as_mut()
                    {
                        match result.and_then(|bytes| screenshots::save(&bytes)) {
                            Ok(path) => {
                                form.insert_screenshot(&path);
                                self.notice = Some(format!("Screenshot saved: {}", path.display()));
                                self.created_screenshots.push(path);
                            }
                            Err(error) => {
                                self.notice = Some(format!("Screenshot paste failed: {error}"))
                            }
                        }
                    }
                }
                Response::ReadySet(id, ready, result) => {
                    self.mutation_busy = false;
                    self.ready_pending = false;
                    match result {
                        Ok(ids) => {
                            for changed_id in &ids {
                                if ready {
                                    self.scheduler_attempted.remove(changed_id);
                                } else {
                                    self.scheduler_attempted.insert(changed_id.clone());
                                }
                            }
                            for bead in self.beads.iter_mut().filter(|bead| ids.contains(&bead.id))
                            {
                                let labels = bead.data["labels"].as_array_mut();
                                if ready {
                                    if let Some(labels) = labels {
                                        if !labels.iter().any(|label| label == beads::READY_LABEL) {
                                            labels.push(serde_json::Value::String(
                                                beads::READY_LABEL.into(),
                                            ));
                                        }
                                    } else {
                                        bead.data["labels"] =
                                            serde_json::json!([beads::READY_LABEL]);
                                    }
                                } else if let Some(labels) = labels {
                                    labels.retain(|label| label != beads::READY_LABEL);
                                }
                            }
                            self.notice = Some(format!(
                                "{id} {} Ready",
                                if ready { "marked" } else { "removed from" }
                            ));
                            if self
                                .selected
                                .and_then(|i| self.beads.get(i))
                                .is_some_and(|bead| bead.id == id)
                            {
                                self.preferred_id = Some(id.clone());
                                self.collapsed.remove(if ready { "ready" } else { "open" });
                            }
                            self.refresh_at(now);
                        }
                        Err(error) => {
                            self.notice = Some(format!("Could not update Ready for {id}: {error}"));
                            self.refresh_at(now);
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
                Response::HarnessesDiscovered(seq, result) => {
                    if let Some(Modal::Create(form)) = self.modal.as_mut()
                        && form.harness_seq == seq
                    {
                        match result {
                            Ok(harnesses) => {
                                form.set_harnesses(harnesses, self.settings.default_agent)
                            }
                            Err(error) => {
                                form.set_harnesses(Vec::new(), self.settings.default_agent);
                                form.harness_error =
                                    Some(format!("Herdr integration discovery failed: {error}"));
                            }
                        }
                        if self.notice.as_deref().is_some_and(|message| {
                            message.starts_with("Checking Herdr integrations;")
                        }) {
                            self.notice = None;
                        }
                    }
                }
                Response::Discovered(seq, result) => {
                    if let Some(pending) = self
                        .handoff
                        .get(&seq)
                        .filter(|item| item.seq == seq && !item.launch_queued)
                        .cloned()
                    {
                        match result {
                            Ok(context) => {
                                let chosen = beads::harness(&pending.bead.data);
                                let agent = if let Some(kind) = chosen {
                                    context.agents.iter().find(|agent| agent.id == kind)
                                } else if pending.scheduled {
                                    match self.settings.default_agent {
                                        DefaultAgent::Kind(kind) => context
                                            .agents
                                            .iter()
                                            .find(|agent| agent.id == kind)
                                            .or_else(|| {
                                                context
                                                    .agents
                                                    .iter()
                                                    .find(|agent| agent.id == "codex")
                                            })
                                            .or_else(|| context.agents.first()),
                                        DefaultAgent::Automatic | DefaultAgent::Unassigned => {
                                            context
                                                .agents
                                                .iter()
                                                .find(|agent| agent.id == "codex")
                                                .or_else(|| context.agents.first())
                                        }
                                    }
                                } else {
                                    context
                                        .agents
                                        .iter()
                                        .find(|agent| agent.id == "codex")
                                        .or_else(|| context.agents.first())
                                }
                                .cloned();
                                if let Some(agent) = agent {
                                    let bead = pending.bead.clone();
                                    self.handoff.get_mut(&seq).unwrap().launch_queued = true;
                                    if seq == self.launch_seq {
                                        self.toast = Some(Toast::progress(format!(
                                            "Handing {} to {} in Herdr…",
                                            bead.id, agent.label
                                        )));
                                    }
                                    let request = if pending.scheduled {
                                        Request::LaunchReady(seq, context, agent, bead)
                                    } else {
                                        Request::Launch(seq, context, agent, bead)
                                    };
                                    if self.requests.send(request).is_err() {
                                        self.handoff.remove(&seq);
                                        let message =
                                            "Handoff could not be queued; try again".to_owned();
                                        self.handoff_feedback(seq, message, true);
                                    }
                                } else {
                                    self.handoff.remove(&seq);
                                    let message = if let Some(kind) = chosen {
                                        format!(
                                            "Harness {kind} for {} is unavailable; change it or toggle Ready to retry",
                                            pending.bead.id
                                        )
                                    } else if pending.scheduled {
                                        format!(
                                            "Selected harness for {} is unavailable; change it or toggle Ready to retry",
                                            pending.bead.id
                                        )
                                    } else {
                                        "No Herdr agents are available in this workspace".to_owned()
                                    };
                                    self.handoff_feedback(seq, message, true);
                                }
                            }
                            Err(error) => {
                                self.handoff.remove(&seq);
                                let message = format!("Handoff discovery failed: {error}");
                                self.handoff_feedback(seq, message, true);
                            }
                        }
                    }
                }
                Response::Launched(seq, result) => {
                    if self
                        .handoff
                        .get(&seq)
                        .is_some_and(|item| item.launch_queued)
                    {
                        let ready_id = self
                            .handoff
                            .get(&seq)
                            .filter(|item| beads::is_ready(&item.bead))
                            .map(|item| item.bead.id.clone());
                        let bead_id = self.handoff[&seq].bead.id.clone();
                        self.handoff.remove(&seq);
                        match result {
                            Ok(outcome) => {
                                self.known_handoffs.insert(bead_id.clone());
                                if !outcome.reserved {
                                    self.handoff_capacity.insert(bead_id);
                                }
                                if let Some(id) = ready_id {
                                    self.known_handoffs.insert(id.clone());
                                    if outcome.reserved {
                                        self.scheduler_attempted.remove(&id);
                                    } else {
                                        self.accepted_handoffs.insert(id);
                                    }
                                }
                                self.handoff_feedback(seq, outcome.message, false);
                                // The launch journal is now active. Reload it even if the
                                // initial worktree scan found no managed handoffs.
                                if self.worktrees_inflight {
                                    self.worktrees_refresh_pending = true;
                                } else {
                                    self.worktrees_discovered = false;
                                    self.worktrees_next_refresh = now;
                                }
                            }
                            Err(error) => {
                                let message = format!("Handoff failed: {error}");
                                self.handoff_feedback(seq, message, true);
                                // A failed launch can still have a durable partial
                                // intent/remote worktree. Always inspect it again.
                                if self.worktrees_inflight {
                                    self.worktrees_refresh_pending = true;
                                } else {
                                    self.worktrees_discovered = false;
                                    self.worktrees_next_refresh = now;
                                }
                            }
                        }
                    }
                }
                Response::Worktrees(result) => {
                    self.worktrees_inflight = false;
                    self.worktrees_loading = false;
                    self.worktrees_next_refresh = now + REFRESH_INTERVAL;
                    self.worktrees_discovered = result.is_ok();
                    if !self.worktree_action_pending {
                        match result {
                            Ok(mut items) => {
                                for item in &mut items {
                                    if matches!(
                                        item.status,
                                        worktrees::WorktreeStatus::Reserved
                                            | worktrees::WorktreeStatus::WorkspaceClosed
                                            | worktrees::WorktreeStatus::CheckoutRemoved
                                    ) {
                                        continue;
                                    }
                                    if let Some(previous) = self.worktrees.iter().find(|previous| {
                                        previous.id == item.id
                                            && previous.workspace_id == item.workspace_id
                                    }) && let Some(line) =
                                        previous.detail.as_deref().and_then(|detail| {
                                            detail.lines().find(|line| {
                                                line.starts_with(
                                                    "Agent observation (display only):",
                                                )
                                            })
                                        })
                                    {
                                        if item.status == worktrees::WorktreeStatus::InProgress
                                            && previous.status == worktrees::WorktreeStatus::Blocked
                                        {
                                            item.status = worktrees::WorktreeStatus::Blocked;
                                        }
                                        let base = item.detail.as_deref().unwrap_or("");
                                        item.detail = Some(if base.is_empty() {
                                            line.into()
                                        } else {
                                            format!("{base}\n{line}")
                                        });
                                    }
                                }
                                if !self.worktrees_refresh_pending {
                                    self.handoff_capacity
                                        .retain(|id| !items.iter().any(|item| &item.bead_id == id));
                                }
                                let selected_id = self
                                    .worktrees
                                    .get(self.selected_worktree)
                                    .map(|item| item.id.clone());
                                let accepted_handoffs: HashSet<_> = items
                                    .iter()
                                    .filter(|item| item.handoff_accepted)
                                    .map(|item| item.bead_id.clone())
                                    .collect();
                                if self.worktrees_refresh_pending {
                                    // This scan started before the successful handoff.
                                    self.accepted_handoffs.extend(accepted_handoffs);
                                } else {
                                    self.accepted_handoffs = accepted_handoffs;
                                }
                                self.known_handoffs
                                    .extend(items.iter().map(|item| item.bead_id.clone()));
                                items.retain(|item| {
                                    item.status != worktrees::WorktreeStatus::CheckoutRemoved
                                });
                                order_worktrees(&mut items);
                                self.worktrees = items;
                                self.selected_worktree = selected_id
                                    .as_deref()
                                    .and_then(|id| {
                                        self.worktrees.iter().position(|item| item.id == id)
                                    })
                                    .unwrap_or(self.selected_worktree)
                                    .min(self.worktrees.len().saturating_sub(1));
                                if selected_id.as_deref()
                                    != self
                                        .worktrees
                                        .get(self.selected_worktree)
                                        .map(|item| item.id.as_str())
                                {
                                    self.worktree_detail_offset = 0;
                                }
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
                Response::AgentsObserved(result) => {
                    if !self.agent_observation_inflight {
                        continue;
                    }
                    self.agent_observation_inflight = false;
                    self.next_agent_observation = now + Duration::from_secs(2);
                    for item in &mut self.worktrees {
                        if matches!(
                            item.status,
                            worktrees::WorktreeStatus::Reserved
                                | worktrees::WorktreeStatus::WorkspaceClosed
                                | worktrees::WorktreeStatus::CheckoutRemoved
                        ) {
                            continue;
                        }
                        let observation = match &result {
                            Ok(observations) => observations.iter().find(|observation| {
                                observation.id == item.id
                                    && observation.workspace_id == item.workspace_id
                            }),
                            Err(_) => None,
                        };
                        if matches!(
                            item.status,
                            worktrees::WorktreeStatus::InProgress
                                | worktrees::WorktreeStatus::Blocked
                        ) {
                            item.status =
                                if observation.is_some_and(|observation| observation.blocked) {
                                    worktrees::WorktreeStatus::Blocked
                                } else {
                                    worktrees::WorktreeStatus::InProgress
                                };
                        }
                        let line = match &result {
                            Ok(_) => observation.map(|observation| observation.detail.clone()),
                            Err(error) => Some(format!(
                                "Agent observation (display only): unknown — poll failed: {error}"
                            )),
                        };
                        if let Some(line) = line {
                            let existing = item.detail.as_deref().unwrap_or("");
                            let base = existing
                                .split("\nAgent observation (display only):")
                                .next()
                                .unwrap_or("");
                            let base = if base.starts_with("Agent observation (display only):") {
                                ""
                            } else {
                                base
                            };
                            item.detail = Some(if base.is_empty() {
                                line
                            } else {
                                format!("{base}\n{line}")
                            });
                        }
                    }
                }
                Response::WorkspaceFocused(id, result) => {
                    self.worktrees_focus_pending = false;
                    self.worktrees_focus_feedback = Some(match result {
                        Ok(()) => WorktreeFocusFeedback::Success(format!(
                            "Switched to Herdr workspace {id}"
                        )),
                        Err(error) => WorktreeFocusFeedback::Error(format!(
                            "Could not switch to Herdr workspace {id}: {error}"
                        )),
                    });
                }
                Response::WorktreeChanged(result) => {
                    self.mutation_busy = false;
                    self.modal = None;
                    self.worktree_action_pending = false;
                    match result {
                        Ok(item) => {
                            if item.handoff_accepted {
                                self.accepted_handoffs.insert(item.bead_id.clone());
                            }
                            if let Some(index) = self
                                .worktrees
                                .iter()
                                .position(|existing| existing.id == item.id)
                            {
                                if item.status == worktrees::WorktreeStatus::CheckoutRemoved {
                                    self.worktrees.remove(index);
                                    self.selected_worktree = self
                                        .selected_worktree
                                        .min(self.worktrees.len().saturating_sub(1));
                                    if self.selected_worktree >= index {
                                        self.worktree_detail_offset = 0;
                                    }
                                } else {
                                    self.worktrees[index] = item.clone();
                                }
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
                Response::WorktreeRestarted(result) => {
                    self.mutation_busy = false;
                    self.modal = None;
                    self.worktree_action_pending = false;
                    self.worktrees_focus_feedback = Some(match result {
                        Ok(message) => WorktreeFocusFeedback::Success(message),
                        Err(error) => {
                            WorktreeFocusFeedback::Error(format!("Restart failed: {error}"))
                        }
                    });
                    if self.worktrees_inflight {
                        self.worktrees_refresh_pending = true;
                    } else {
                        self.worktrees_discovered = false;
                        self.worktrees_next_refresh = now;
                    }
                }
                Response::WorktreeContinued(result) => {
                    self.mutation_busy = false;
                    self.modal = None;
                    self.worktree_action_pending = false;
                    self.worktrees_focus_feedback = Some(match result {
                        Ok(message) => WorktreeFocusFeedback::Success(message),
                        Err(error) => {
                            WorktreeFocusFeedback::Error(format!("Continue failed: {error}"))
                        }
                    });
                    if self.worktrees_inflight {
                        self.worktrees_refresh_pending = true;
                    } else {
                        self.worktrees_discovered = false;
                        self.worktrees_next_refresh = now;
                    }
                }
            }
        }
    }
    fn key(&mut self, k: KeyEvent) -> bool {
        if self.mutation_busy && !self.ready_pending {
            return false;
        }
        if let Some(help) = self.help.as_mut() {
            if hotkeys::is_help_key(k) || k.code == KeyCode::Esc {
                self.help = None;
            } else {
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => help.offset = help.offset.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => {
                        help.offset = help.offset.saturating_add(1)
                    }
                    KeyCode::PageUp => help.offset = help.offset.saturating_sub(10),
                    KeyCode::PageDown => help.offset = help.offset.saturating_add(10),
                    KeyCode::Home => help.offset = 0,
                    KeyCode::End => help.offset = usize::MAX,
                    _ => {}
                }
            }
            return false;
        }
        if hotkeys::is_help_key(k) {
            self.help = Some(Help {
                context: hotkeys::Context::current(self),
                offset: 0,
            });
            return false;
        }
        if self.modal.is_some() {
            return self.modal_key(k);
        }
        if event::is_quit(k) {
            return true;
        }
        // Keep browsing available during Ready, but do not open mutation dialogs
        // or start handoffs against a partially updated cascade.
        if self.ready_pending
            && matches!(
                k.code,
                KeyCode::Char(
                    'n' | 'N'
                        | 'e'
                        | 'd'
                        | 'g'
                        | 'h'
                        | 'p'
                        | 's'
                        | 'u'
                        | 'm'
                        | 'c'
                        | 'x'
                        | 'R'
                        | 'a'
                )
            )
        {
            return false;
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
                KeyCode::Enter => self.focus_selected_worktree(),
                KeyCode::Char('m') => self.confirm_worktree_action(0),
                KeyCode::Char('c') => self.confirm_worktree_action(1),
                KeyCode::Char('x') => self.confirm_worktree_action(2),
                KeyCode::Char('R') => self.confirm_worktree_action(3),
                KeyCode::Char('a') => self.confirm_worktree_action(4),
                _ => {}
            }
            return false;
        }
        match k.code {
            KeyCode::Char('p') => {
                self.planning_seq = self.planning_seq.wrapping_add(1);
                self.modal = Some(Modal::Planning(PlanningForm::new(self.planning_seq)));
                self.modal_ready = false;
                self.notice = None;
            }
            KeyCode::Char('w') => {
                self.worktrees_view = true;
                self.worktrees_focus_feedback = None;
                self.worktrees_next_refresh = Instant::now();
                self.poll_worktrees(Instant::now());
            }
            KeyCode::Char('r' | 'R') => self.toggle_ready(),
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
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let child = k.code == KeyCode::Char('N')
                    || (k.code == KeyCode::Char('n')
                        && k.modifiers.contains(crossterm::event::KeyModifiers::SHIFT));
                let parent_id = if child {
                    match self.selected.and_then(|i| self.beads.get(i)) {
                        Some(bead)
                            if bead.data["issue_type"] == "task" && bead.status != "closed" =>
                        {
                            Some(bead.id.clone())
                        }
                        _ => {
                            self.notice = Some("Select an open task to create a child".into());
                            return false;
                        }
                    }
                } else {
                    None
                };
                self.harness_seq = self.harness_seq.wrapping_add(1);
                let seq = self.harness_seq;
                self.modal = Some(Modal::Create(CreateForm {
                    parent_id,
                    harness_loading: true,
                    harness_seq: seq,
                    ..CreateForm::default()
                }));
                self.modal_ready = false;
                self.notice = None;
                if self.requests.send(Request::DiscoverHarnesses(seq)).is_err()
                    && let Some(Modal::Create(form)) = self.modal.as_mut()
                {
                    form.harness_loading = false;
                    form.harness_error =
                        Some("Herdr integration discovery could not be queued".into());
                }
            }
            KeyCode::Char('e') => {
                if let Some(b) = self.selected.and_then(|i| self.beads.get(i)) {
                    let id = b.id.clone();
                    self.edit_seq = self.edit_seq.wrapping_add(1);
                    let seq = self.edit_seq;
                    self.modal = Some(Modal::EditLoading {
                        id: id.clone(),
                        seq,
                        error: None,
                    });
                    self.modal_ready = false;
                    self.notice = None;
                    if self.requests.send(Request::LoadEdit(id, seq)).is_err()
                        && let Some(Modal::EditLoading { error, .. }) = self.modal.as_mut()
                    {
                        *error = Some("Edit could not be queued; try again".into());
                    }
                }
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
            KeyCode::Char('g') => self.open_epic_generation(),
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
    fn focus_selected_worktree(&mut self) {
        if self.worktrees_focus_pending {
            return;
        }
        let Some(item) = self.worktrees.get(self.selected_worktree) else {
            self.worktrees_focus_feedback = Some(WorktreeFocusFeedback::Error(
                "Select a managed worktree first".into(),
            ));
            return;
        };
        if matches!(
            item.status,
            worktrees::WorktreeStatus::WorkspaceClosed | worktrees::WorktreeStatus::CheckoutRemoved
        ) {
            self.worktrees_focus_feedback = Some(WorktreeFocusFeedback::Error(
                "This worktree's Herdr workspace is closed".into(),
            ));
            return;
        }
        self.worktrees_focus_feedback = None;
        self.worktrees_focus_pending = true;
        if self
            .requests
            .send(Request::FocusWorkspace(item.workspace_id.clone()))
            .is_err()
        {
            self.worktrees_focus_pending = false;
            self.worktrees_focus_feedback = Some(WorktreeFocusFeedback::Error(
                "Workspace switch could not be queued".into(),
            ));
        }
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
                worktrees::WorktreeStatus::InProgress
                    | worktrees::WorktreeStatus::Blocked
                    | worktrees::WorktreeStatus::Unmerged
                    | worktrees::WorktreeStatus::MergeQueued
                    | worktrees::WorktreeStatus::NeedsResolution
            )
        {
            self.worktrees_error = Some("This worktree is not ready for a merge request".into());
            return;
        }
        if action == 3
            && matches!(
                item.status,
                worktrees::WorktreeStatus::CheckoutRemoved
                    | worktrees::WorktreeStatus::WorkspaceClosed
            )
        {
            self.worktrees_error = Some("Select an active checkout to restart".into());
            return;
        }
        if action == 4
            && !matches!(
                item.status,
                worktrees::WorktreeStatus::InProgress | worktrees::WorktreeStatus::DirtyCheckout
            )
        {
            self.worktrees_error = Some("Select an active task checkout to continue".into());
            return;
        }
        self.modal = Some(Modal::ConfirmWorktree {
            id: item.id.clone(),
            action,
        });
        self.modal_ready = false;
    }
    fn planning_key(&mut self, key: KeyEvent) {
        use KeyCode::*;
        let Some(Modal::Planning(form)) = self.modal.as_mut() else {
            return;
        };
        if key.code == Esc {
            if form.session.is_some() {
                self.notice = Some(format!(
                    "Planning cancelled; Herdr tab remains open for review ({})",
                    form.session.as_ref().map_or("", |session| &session.tab_id)
                ));
            }
            self.modal = None;
            return;
        }
        match &form.stage {
            PlanningStage::Draft => match key.code {
                Enter if key.modifiers.contains(KeyModifiers::SHIFT) => form.proposal.push('\n'),
                Enter if !form.proposal.trim().is_empty() => {
                    let request =
                        Request::PlanLaunch(form.seq, form.proposal.clone(), form.marker.clone());
                    if self.requests.send(request).is_ok() {
                        form.stage = PlanningStage::Waiting;
                    } else {
                        form.stage =
                            PlanningStage::Failed("Planning launch could not be queued".into());
                    }
                }
                Backspace => {
                    form.proposal.pop();
                }
                Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => form.proposal.push(c),
                _ => {}
            },
            PlanningStage::Question { options, .. } => {
                let options = options.clone();
                match key.code {
                    Up => form.selected = form.selected.saturating_sub(1),
                    Down => form.selected = (form.selected + 1).min(options.len()),
                    Tab => form.selected = options.len(),
                    Backspace if form.selected == options.len() => {
                        form.input.pop();
                    }
                    Char(c)
                        if form.selected == options.len()
                            && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        form.input.push(c)
                    }
                    Enter => {
                        let answer = if form.selected == options.len() {
                            form.input.trim().to_owned()
                        } else {
                            options[form.selected].clone()
                        };
                        if !answer.is_empty() {
                            Self::submit_planning_reply(form, &self.requests, answer);
                        }
                    }
                    _ => {}
                }
            }
            PlanningStage::Review(plan) => match key.code {
                Char('y' | 'Y') => {
                    let request = Request::PlanCreate(form.seq, plan.clone());
                    if self.requests.send(request).is_ok() {
                        form.stage = PlanningStage::Creating;
                        self.mutation_busy = true;
                    } else {
                        form.stage =
                            PlanningStage::Failed("Bead creation could not be queued".into());
                    }
                }
                Char('r') => {
                    form.input.clear();
                    form.stage = PlanningStage::Revision;
                }
                Up => form.scroll = form.scroll.saturating_sub(1),
                Down => form.scroll = form.scroll.saturating_add(1),
                PageUp => form.scroll = form.scroll.saturating_sub(10),
                PageDown => form.scroll = form.scroll.saturating_add(10),
                _ => {}
            },
            PlanningStage::Revision => match key.code {
                Enter if key.modifiers.contains(KeyModifiers::SHIFT) => form.input.push('\n'),
                Enter if !form.input.trim().is_empty() => {
                    Self::submit_planning_reply(form, &self.requests, form.input.trim().to_owned());
                }
                Backspace => {
                    form.input.pop();
                }
                Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => form.input.push(c),
                _ => {}
            },
            PlanningStage::Failed(_) if key.code == Char('r') && form.session.is_some() => {
                form.stage = PlanningStage::Waiting;
                form.next_poll = Instant::now();
            }
            _ => {}
        }
    }

    fn submit_planning_reply(
        form: &mut PlanningForm,
        requests: &mpsc::Sender<Request>,
        answer: String,
    ) {
        let Some(session) = form.session.clone() else {
            form.stage = PlanningStage::Failed("Planning agent session is unavailable".into());
            return;
        };
        form.round = form.round.saturating_add(1);
        form.marker = format!("{}_{}", form.marker, form.round);
        if requests
            .send(Request::PlanReply(
                form.seq,
                session,
                answer,
                form.marker.clone(),
            ))
            .is_ok()
        {
            form.stage = PlanningStage::Waiting;
            form.poll_inflight = false;
            form.next_poll = Instant::now() + Duration::from_secs(2);
        } else {
            form.stage = PlanningStage::Failed("Planning reply could not be queued".into());
        }
    }

    fn modal_key(&mut self, key: KeyEvent) -> bool {
        use crossterm::event::KeyCode::*;
        if self.mutation_busy {
            return false;
        }
        if !self.modal_ready && key.code != Esc {
            return false;
        }
        if matches!(self.modal, Some(Modal::Planning(_))) {
            self.planning_key(key);
            return false;
        }
        if key.code == Esc {
            if let Some(Modal::Settings { dropdown_open, .. }) = self.modal.as_mut() {
                if *dropdown_open {
                    *dropdown_open = false;
                } else {
                    self.modal = None;
                }
                return false;
            }
            if let Some(Modal::Create(form) | Modal::Edit { form, .. }) = self.modal.as_mut()
                && let Some(which) = form.dropdown.take()
            {
                if let Some(old) = form.dropdown_original.take() {
                    match which {
                        2 => form.issue_type = old,
                        3 => form.priority = old,
                        _ => form.harness = old,
                    }
                }
                return false;
            }
            for path in self.created_screenshots.drain(..) {
                let _ = std::fs::remove_file(path);
            }
            self.modal = None;
            self.paste_seq = self.paste_seq.wrapping_add(1);
            self.paste_pending = false;
            return false;
        }
        if self.paste_pending {
            self.notice = Some("Wait for screenshot paste or press Esc to cancel".into());
            return false;
        }
        if let Some(Modal::ConfirmWorktree { id, action }) = self.modal.as_ref() {
            if matches!(key.code, Char('y') | Char('Y')) {
                let id = id.clone();
                let request = match action {
                    0 => Request::WorktreeAction(id, WorktreeAction::RequestMerge),
                    1 => Request::WorktreeAction(id, WorktreeAction::CloseWorkspace),
                    2 => Request::WorktreeAction(id, WorktreeAction::RemoveCheckout),
                    3 => Request::RestartWorktree(id),
                    _ => Request::ContinueWorktree(id),
                };
                self.mutation_busy = true;
                if self.requests.send(request).is_err() {
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
                        *focus = (*focus + if key.code == Tab { 1 } else { 7 }) % 8;
                    }
                    Up | Down => {
                        *focus = (*focus + if key.code == Down { 1 } else { 7 }) % 8;
                        *dropdown_open = false;
                    }
                    Left | Right if *focus == 0 => {
                        draft.theme = cycle_theme(draft.theme, key.code == Right);
                        save_settings = Some(*draft);
                    }
                    Left | Right if *focus == 1 => {
                        let count = beads::HARNESSES.len() + 2;
                        let index =
                            cycle_choice(draft.default_agent.index(), count, key.code == Right);
                        draft.default_agent = DefaultAgent::at(index);
                        save_settings = Some(*draft);
                    }
                    Left | Right if *focus == 2 => {
                        draft.agent_limit = draft.agent_limit.step(key.code == Right);
                        save_settings = Some(*draft);
                    }
                    Enter | Char(' ') if *focus <= 2 && *dropdown_open => {
                        *dropdown_open = false;
                        save_settings = Some(*draft);
                    }
                    Enter | Char(' ') if *focus <= 2 => {
                        *dropdown_open = true;
                    }
                    Enter | Char(' ') if (3..=6).contains(focus) => {
                        match *focus {
                            3 => draft.auto_merge = !draft.auto_merge,
                            4 => draft.auto_merge_children = !draft.auto_merge_children,
                            5 => draft.auto_close_workspace = !draft.auto_close_workspace,
                            _ => draft.auto_remove_checkout = !draft.auto_remove_checkout,
                        }
                        save_settings = Some(*draft);
                    }
                    Enter | Char(' ') if *focus == 7 => {
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
        if let Some(Modal::ConfirmGenerateEpic { id, .. }) = self.modal.as_ref() {
            if matches!(key.code, Char('y') | Char('Y')) {
                let id = id.clone();
                if !self.planning_epics.contains(&id) {
                    if self
                        .requests
                        .send(Request::GenerateEpic(id.clone()))
                        .is_ok()
                    {
                        self.planning_epics.insert(id.clone());
                        self.modal = None;
                        self.notice = Some(format!("Starting child-task planning for {id}…"));
                        self.toast = Some(Toast::progress(format!(
                            "Starting child-task planning for {id}…"
                        )));
                    } else {
                        self.notice = Some("Epic planning could not be queued; try again".into());
                    }
                }
            } else if matches!(key.code, Char('n') | Char('N') | Enter) {
                self.modal = None;
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
        if let Some(Modal::EditLoading { id, seq, error }) = self.modal.as_mut() {
            if key.code == Char('e') && error.is_some() {
                let id = id.clone();
                self.edit_seq = self.edit_seq.wrapping_add(1);
                *seq = self.edit_seq;
                *error = None;
                if self.requests.send(Request::LoadEdit(id, *seq)).is_err()
                    && let Some(Modal::EditLoading { error, .. }) = self.modal.as_mut()
                {
                    *error = Some("Edit could not be queued; try again".into());
                }
            }
            return false;
        }
        let edit_id = match self.modal.as_ref() {
            Some(Modal::Edit { id, .. }) => Some(id.clone()),
            _ => None,
        };
        let Some(Modal::Create(form) | Modal::Edit { form, .. }) = self.modal.as_mut() else {
            return false;
        };
        if let Some(which) = form.dropdown {
            match key.code {
                Up => {
                    if which == 2 {
                        form.issue_type = form.issue_type.saturating_sub(1)
                    } else if which == 3 {
                        form.priority = form.priority.saturating_sub(1)
                    } else {
                        form.harness = form.harness.saturating_sub(1)
                    }
                }
                Down => {
                    if which == 2 {
                        form.issue_type = (form.issue_type + 1).min(beads::BEAD_TYPES.len() - 1)
                    } else if which == 3 {
                        form.priority = (form.priority + 1).min(4)
                    } else {
                        form.harness = (form.harness + 1).min(form.harnesses.len())
                    }
                }
                Left | Right => {
                    if which == 2 {
                        form.issue_type = cycle_choice(
                            form.issue_type,
                            beads::BEAD_TYPES.len(),
                            key.code == Right,
                        );
                    } else if which == 3 {
                        form.priority = cycle_choice(form.priority, 5, key.code == Right);
                    } else {
                        form.harness =
                            cycle_choice(form.harness, form.harnesses.len() + 1, key.code == Right);
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
        if form.focus == 1 && !matches!(key.code, Up | Down) {
            form.description_preferred_col = None;
        }
        if key.code == Char('v') && key.modifiers.contains(KeyModifiers::CONTROL) && form.focus == 1
        {
            self.paste_seq = self.paste_seq.wrapping_add(1);
            self.paste_pending = true;
            if self
                .requests
                .send(Request::ReadScreenshot(self.paste_seq))
                .is_err()
            {
                self.paste_pending = false;
                self.notice = Some("Screenshot paste could not be queued".into());
            }
            return false;
        }
        match key.code {
            Tab => {
                let epic = edit_id.is_none() && beads::BEAD_TYPES[form.issue_type] == "epic";
                form.focus = match form.focus {
                    0 => 2,
                    2 => 3,
                    3 => 6,
                    6 => 1,
                    1 if epic => 7,
                    1 => 4,
                    7 => 4,
                    4 => 5,
                    _ => 0,
                }
            }
            BackTab => {
                let epic = edit_id.is_none() && beads::BEAD_TYPES[form.issue_type] == "epic";
                form.focus = match form.focus {
                    0 => 5,
                    5 => 4,
                    4 if epic => 7,
                    4 => 1,
                    7 => 1,
                    1 => 6,
                    6 => 3,
                    3 => 2,
                    _ => 0,
                }
            }
            Enter if matches!(form.focus, 2 | 3 | 6) => {
                form.dropdown = Some(form.focus);
                form.dropdown_original = Some(match form.focus {
                    2 => form.issue_type,
                    3 => form.priority,
                    _ => form.harness,
                });
            }
            Left | Right if form.focus == 2 => {
                form.issue_type =
                    cycle_choice(form.issue_type, beads::BEAD_TYPES.len(), key.code == Right);
            }
            Left | Right if form.focus == 3 => {
                form.priority = cycle_choice(form.priority, 5, key.code == Right);
            }
            Left | Right if form.focus == 6 => {
                form.harness =
                    cycle_choice(form.harness, form.harnesses.len() + 1, key.code == Right);
            }
            Enter if form.focus == 4 => {
                if !form.title.trim().is_empty() {
                    if edit_id.is_none() && form.harness_loading {
                        self.notice = Some(
                            "Checking Herdr integrations; wait before creating an issue".into(),
                        );
                        return false;
                    }
                    if edit_id.is_none()
                        && form.generate_children
                        && beads::BEAD_TYPES[form.issue_type] == "epic"
                        && form.description.trim().is_empty()
                    {
                        self.notice =
                            Some("Describe the Epic proposal before generating child tasks".into());
                        return false;
                    }
                    let input = CreateBead {
                        title: form.title.trim().into(),
                        description: form.description.clone(),
                        issue_type: beads::BEAD_TYPES[form.issue_type].into(),
                        priority: form.priority as u8,
                        harness: form
                            .harness
                            .checked_sub(1)
                            .map(|index| form.harnesses[index].clone()),
                    };
                    self.mutation_busy = true;
                    let request = if let Some(id) = edit_id {
                        Request::Update(UpdateBead {
                            id,
                            fields: input,
                            previous_harness: form.previous_harness.clone(),
                        })
                    } else if form.generate_children && input.issue_type == "epic" {
                        Request::CreateEpic(input, form.parent_id.clone())
                    } else if let Some(parent) = &form.parent_id {
                        Request::CreateChild(input, parent.clone())
                    } else {
                        Request::Create(input)
                    };
                    if self.requests.send(request).is_err() {
                        self.mutation_busy = false;
                        self.notice = Some("Save could not be queued; try again".into());
                    }
                } else {
                    self.notice = Some("Title is required".into());
                }
            }
            Enter if form.focus == 5 => {
                self.modal = None;
                return false;
            }
            Enter | Char(' ') if form.focus == 7 => {
                form.generate_children = !form.generate_children;
            }
            Char(' ') if matches!(form.focus, 2 | 3 | 6) => {
                form.dropdown = Some(form.focus);
                form.dropdown_original = Some(match form.focus {
                    2 => form.issue_type,
                    3 => form.priority,
                    _ => form.harness,
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
            Enter if form.focus == 1 && key.modifiers.is_empty() => {
                form.focus = if edit_id.is_none() && beads::BEAD_TYPES[form.issue_type] == "epic" {
                    7
                } else {
                    4
                };
            }
            Up if form.focus == 1 => {
                form.description_cursor = crate::ui::move_description_vertical(
                    &form.description,
                    form.description_cursor,
                    false,
                    &mut form.description_preferred_col,
                )
            }
            Down if form.focus == 1 => {
                form.description_cursor = crate::ui::move_description_vertical(
                    &form.description,
                    form.description_cursor,
                    true,
                    &mut form.description_preferred_col,
                )
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
        if beads::BEAD_TYPES[form.issue_type] != "epic" {
            form.generate_children = false;
            if form.focus == 7 {
                form.focus = 4;
            }
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
    pub fn selected_is_epic(&self) -> bool {
        self.selected
            .and_then(|i| self.beads.get(i))
            .is_some_and(|bead| bead.data["issue_type"] == "epic")
    }
    fn open_epic_generation(&mut self) {
        if !self.selected_is_epic() {
            return;
        }
        let bead = &self.beads[self.selected.unwrap()];
        if self.planning_epics.contains(&bead.id) {
            self.notice = Some(format!("Epic {} is already being planned", bead.id));
            return;
        }
        self.modal = Some(Modal::ConfirmGenerateEpic {
            id: bead.id.clone(),
            title: bead.title.clone(),
        });
        self.modal_ready = false;
        self.notice = None;
    }
    fn open_launch(&mut self) {
        let Some(bead) = self
            .selected
            .and_then(|index| self.beads.get(index))
            .cloned()
        else {
            let message = "Select a bead before handing it to an agent".to_owned();
            self.notice = Some(message.clone());
            self.toast = Some(Toast::result(message, true));
            return;
        };
        self.start_launch(bead, false);
    }
    fn toggle_ready(&mut self) {
        if self.mutation_busy {
            return;
        }
        let Some(bead) = self.selected.and_then(|i| self.beads.get(i)) else {
            self.notice = Some("Select an open issue first".into());
            return;
        };
        let epic = bead.data["issue_type"] == "epic";
        if bead.status != "open" {
            self.notice = Some("Only open issues can be marked Ready".into());
            return;
        }
        let id = bead.id.clone();
        let ready = !beads::is_ready(bead);
        if !ready && !self.worktrees_discovered {
            self.notice = Some("Wait for tracked handoffs to load before removing Ready".into());
            return;
        }
        if self.known_handoffs.contains(&id) && ready {
            self.notice = Some("This issue already has a tracked handoff".into());
            return;
        }
        let mut protected_handoffs = self.accepted_handoffs.clone();
        protected_handoffs.extend(self.handoff.values().map(|pending| pending.bead.id.clone()));
        if !ready && !epic && protected_handoffs.contains(&id) {
            self.notice = Some(
                "This issue has been sent or is being handed off; Ready cannot be removed".into(),
            );
            return;
        }
        self.mutation_busy = true;
        self.ready_pending = true;
        self.notice = Some(format!("Updating Ready for {id}…"));
        let request = if epic {
            Request::SetEpicReady(id, ready, self.known_handoffs.clone(), protected_handoffs)
        } else {
            Request::SetReady(id, ready)
        };
        if self.requests.send(request).is_err() {
            self.mutation_busy = false;
            self.ready_pending = false;
            self.notice = Some("Ready change could not be queued".into());
        }
    }
    fn start_launch(&mut self, bead: Bead, scheduled: bool) {
        if self
            .handoff
            .values()
            .any(|pending| pending.bead.id == bead.id)
        {
            return;
        }
        self.launch_seq = self.launch_seq.wrapping_add(1);
        let seq = self.launch_seq;
        self.notice = None;
        self.toast = Some(Toast::progress(format!(
            "Finding a Herdr agent for {}…",
            bead.id
        )));
        self.handoff.insert(
            seq,
            PendingHandoff {
                seq,
                bead,
                launch_queued: false,
                scheduled,
            },
        );
        if self.requests.send(Request::Discover(seq)).is_err() {
            self.handoff.remove(&seq);
            let message = "Workspace discovery could not be queued; try again".to_owned();
            self.notice = Some(message.clone());
            self.toast = Some(Toast::result(message, true));
        }
    }
    fn handoff_feedback(&mut self, seq: u64, message: String, error: bool) {
        if seq == self.launch_seq {
            self.notice = Some(message.clone());
            self.toast = Some(Toast::result(message, error));
        }
    }
    fn mouse(&mut self, k: MouseEventKind, x: u16, y: u16) {
        if self.modal.is_some() || self.help.is_some() {
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
        "ready" => (1, status),
        "closed" => (3, status),
        _ => (2, status),
    }
}
pub fn display_status(bead: &Bead) -> &str {
    if beads::is_ready(bead) {
        "ready"
    } else {
        normalized_status(&bead.status)
    }
}
pub(super) fn worktree_status_label(status: worktrees::WorktreeStatus) -> &'static str {
    use worktrees::WorktreeStatus::*;
    match status {
        Reserved => "PREPARED",
        InProgress => "IN PROGRESS",
        Blocked => "BLOCKED",
        Unmerged => "READY TO MERGE",
        MergeQueued => "MERGE QUEUED",
        MergeRequested => "MERGE PENDING",
        MergeInProgress => "MERGE IN PROGRESS",
        NeedsResolution => "NEEDS RESOLUTION",
        Integrated => "INTEGRATED",
        WorkspaceClosed => "WORKSPACE CLOSED",
        CheckoutRemoved => "CHECKOUT REMOVED",
        DirtyCheckout => "UNCOMMITTED CHANGES",
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
    let mut groups = [
        "open",
        "ready",
        "in_progress",
        "blocked",
        "deferred",
        "closed",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain(beads.iter().map(|b| display_status(b).to_owned()))
    .collect::<Vec<_>>();
    groups.sort_by(|a, b| status_order(a).cmp(&status_order(b)).then_with(|| a.cmp(b)));
    groups.dedup();
    groups
}

/// The visible ancestry of an issue within its status pane, from root to issue.
/// A parent in another pane (or missing from the list) starts a new visible root.
pub(crate) fn issue_path(
    beads: &[Bead],
    positions: &HashMap<&str, usize>,
    index: usize,
) -> Vec<usize> {
    let status = display_status(&beads[index]);
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    let mut current = Some(index);
    while let Some(i) = current {
        if !seen.insert(i) {
            break;
        }
        path.push(i);
        current = beads[i]
            .data
            .get("parent")
            .and_then(|parent| parent.as_str())
            .and_then(|id| positions.get(id).copied())
            .filter(|&parent| display_status(&beads[parent]) == status);
    }
    path.reverse();
    path
}

fn closed_recency(bead: &Bead) -> Option<time::OffsetDateTime> {
    ["closed_at", "updated_at", "created_at"]
        .into_iter()
        .filter_map(|field| bead.data.get(field).and_then(|value| value.as_str()))
        .find_map(|value| {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
        })
}

fn order_worktrees(items: &mut [ManagedWorktree]) {
    let positions: HashMap<_, _> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.bead_id.clone(), (index, item.parent_bead_id.clone())))
        .collect();
    items.sort_by_cached_key(|item| {
        let mut path = Vec::new();
        let mut parent = Some(item.bead_id.as_str());
        let mut seen = HashSet::new();
        while let Some(id) = parent {
            if !seen.insert(id) {
                break;
            }
            let Some((index, next)) = positions.get(id) else {
                break;
            };
            path.push(*index);
            parent = next.as_deref();
        }
        path.reverse();
        path
    });
}

pub fn list_layout(beads: &[Bead], collapsed: &HashSet<String>) -> ListLayout {
    let positions = beads
        .iter()
        .enumerate()
        .map(|(index, bead)| (bead.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut layout = ListLayout {
        groups: Vec::new(),
        targets: Vec::new(),
        height: 0,
    };
    for status in group_statuses(beads) {
        let mut indices = beads
            .iter()
            .enumerate()
            .filter(|(_, b)| display_status(b) == status)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if status == "closed" {
            indices.sort_by_cached_key(|&index| {
                (
                    std::cmp::Reverse(closed_recency(&beads[index])),
                    beads[index].id.clone(),
                )
            });
        } else {
            indices.sort_by_cached_key(|&index| issue_path(beads, &positions, index));
        }
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
        app.poll_planning(Instant::now());
        app.poll_worktrees(Instant::now());
        app.poll_agent_observations(Instant::now());
        app.poll_scheduler();
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

    #[test]
    fn update_key_no_longer_opens_browser_action() {
        let (mut app, requests, _) = connected_app();
        app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
        assert!(app.modal.is_none());
        assert!(requests.try_recv().is_err());
        assert!(
            !hotkeys::Context::Issues
                .entries()
                .iter()
                .any(|(key, _)| *key == "u")
        );
    }

    #[test]
    fn shift_n_creates_child_of_selected_task_and_n_stays_top_level() {
        let mut app = app();
        app.beads[0].data["issue_type"] = json!("task");
        let (tx, rx) = mpsc::channel();
        app.requests = tx;
        app.key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT));
        let seq = take_harness_discovery(&rx);
        supply_harnesses(&mut app, seq, &["codex"]);
        assert!(
            matches!(&app.modal, Some(Modal::Create(form)) if form.parent_id.as_deref() == Some("b0"))
        );
        app.modal_ready = true;
        app.key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        for _ in 0..5 {
            app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        }
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            matches!(rx.try_recv(), Ok(Request::CreateChild(input, parent)) if input.title == "x" && parent == "b0")
        );
        app.modal = None;
        app.mutation_busy = false;
        app.key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(matches!(&app.modal, Some(Modal::Create(form)) if form.parent_id.is_none()));
    }

    #[test]
    fn managed_children_sort_immediately_below_parent() {
        let item = |id: &str, parent: Option<&str>| ManagedWorktree {
            id: id.into(),
            bead_id: id.into(),
            parent_bead_id: parent.map(str::to_owned),
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: id.into(),
            target_branch: "trunk".into(),
            workspace_id: id.into(),
            status: worktrees::WorktreeStatus::Unmerged,
            detail: None,
        };
        let mut items = vec![
            item("parent", None),
            item("other", None),
            item("child", Some("parent")),
            item("grandchild", Some("child")),
        ];
        order_worktrees(&mut items);
        assert_eq!(
            items
                .iter()
                .map(|item| item.bead_id.as_str())
                .collect::<Vec<_>>(),
            ["parent", "child", "grandchild", "other"]
        );
    }

    #[test]
    fn issue_layout_groups_children_under_visible_parents() {
        let bead = |id: &str, parent: Option<&str>, status: &str| Bead {
            id: id.into(),
            title: id.into(),
            status: status.into(),
            priority: 2,
            data: json!({"parent": parent}),
        };
        // Input is ID order, like the refreshed list. A child can precede its parent.
        let beads = vec![
            bead("a-child", Some("z-parent"), "open"),
            bead("b-grandchild", Some("a-child"), "open"),
            bead("c-other", None, "open"),
            bead("d-detached", Some("z-parent"), "closed"),
            bead("e-orphan", Some("missing"), "open"),
            bead("z-parent", None, "open"),
        ];
        let layout = list_layout(&beads, &HashSet::new());
        let open = layout
            .groups
            .iter()
            .find(|group| group.status == "open")
            .unwrap();
        assert_eq!(open.indices, [2, 4, 5, 0, 1]);
        let closed = layout
            .groups
            .iter()
            .find(|group| group.status == "closed")
            .unwrap();
        assert_eq!(closed.indices, [3]);
        let positions = beads
            .iter()
            .enumerate()
            .map(|(index, bead)| (bead.id.as_str(), index))
            .collect();
        assert_eq!(issue_path(&beads, &positions, 1), [5, 0, 1]);
        assert_eq!(issue_path(&beads, &positions, 3), [3]);
        assert_eq!(issue_path(&beads, &positions, 4), [4]);

        let mut app = app();
        app.beads = beads;
        app.collapsed.clear();
        app.selected = Some(5);
        app.step(1);
        assert_eq!(app.selected, Some(0));
        app.step(1);
        assert_eq!(app.selected, Some(1));
        app.list_rect = Rect::new(5, 7, 40, 24);
        let parent_row = list_layout(&app.beads, &app.collapsed)
            .targets
            .iter()
            .find(|target| target.row == DisplayRow::Bead(5))
            .unwrap()
            .y;
        app.mouse(
            MouseEventKind::Down(MouseButton::Left),
            6,
            7 + parent_row as u16,
        );
        assert_eq!(app.selected, Some(5));
    }

    #[test]
    fn closed_group_uses_completion_recency_instead_of_id_or_parent_order() {
        let bead = |id: &str, status: &str, data: serde_json::Value| Bead {
            id: id.into(),
            title: id.into(),
            status: status.into(),
            priority: 2,
            data,
        };
        let beads = vec![
            bead(
                "a-old",
                "closed",
                json!({"closed_at": "2026-10-20T15:00:00Z", "updated_at": "2026-10-21T15:00:00Z"}),
            ),
            bead(
                "b-fallback",
                "closed",
                json!({"closed_at": "invalid", "updated_at": "2026-10-20T14:00:00Z"}),
            ),
            bead(
                "z-new",
                "closed",
                json!({"parent": "a-old", "closed_at": "2026-10-20T17:30:00+02:00"}),
            ),
            bead(
                "d-tie",
                "closed",
                json!({"closed_at": "2026-10-20T15:00:00Z"}),
            ),
            bead("c-undated", "closed", json!({"closed_at": "invalid"})),
            bead(
                "e-created",
                "closed",
                json!({"created_at": "2026-10-20T13:00:00Z"}),
            ),
            bead("open", "open", json!({})),
        ];
        let layout = list_layout(&beads, &HashSet::new());
        let closed = layout
            .groups
            .iter()
            .find(|group| group.status == "closed")
            .unwrap();
        assert_eq!(closed.indices, [2, 0, 3, 1, 5, 4]);
        assert_eq!(
            layout
                .targets
                .iter()
                .filter_map(|target| match target.row {
                    DisplayRow::Bead(index) if beads[index].status == "closed" => Some(index),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            closed.indices
        );
    }

    #[test]
    fn closed_refresh_reorders_rows_and_keeps_selected_issue() {
        let mut a = app();
        let mut items = a.beads[..2].to_vec();
        for item in &mut items {
            item.status = "closed".into();
        }
        items[0].data["closed_at"] = json!("2026-10-20T15:00:00Z");
        items[1].data["closed_at"] = json!("2026-10-20T14:00:00Z");
        respond(&mut a, Response::List(Ok(items.clone())));
        assert_eq!(a.beads[a.selected.unwrap()].id, "b0");
        assert_eq!(
            list_layout(&a.beads, &a.collapsed).groups[5].indices,
            [0, 1]
        );

        items[1].data["closed_at"] = json!("2026-10-20T16:00:00Z");
        respond(&mut a, Response::List(Ok(items)));
        assert_eq!(a.beads[a.selected.unwrap()].id, "b0");
        assert_eq!(
            list_layout(&a.beads, &a.collapsed).groups[5].indices,
            [1, 0]
        );
        a.step(-1);
        assert_eq!(a.beads[a.selected.unwrap()].id, "b1");
    }
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
            edit_seq: 0,
            harness_seq: 0,
            launch_seq: 0,
            handoff: HashMap::new(),
            handoff_capacity: HashSet::new(),
            toast: None,
            list_rect: Rect::new(0, 0, 30, 8),
            detail_rect: Rect::new(30, 0, 50, 8),
            modal: None,
            help: None,
            settings: Settings::default(),
            theme: Theme::Dark,
            settings_path: None,
            settings_load_error: None,
            worktrees: Vec::new(),
            accepted_handoffs: HashSet::new(),
            known_handoffs: HashSet::new(),
            scheduler_attempted: HashSet::new(),
            selected_worktree: 0,
            worktree_offset: 0,
            worktree_detail_offset: 0,
            worktrees_view: false,
            worktrees_loading: false,
            worktrees_error: None,
            worktrees_focus_feedback: None,
            worktrees_focus_pending: false,
            worktrees_inflight: false,
            worktrees_discovered: true,
            worktrees_refresh_pending: false,
            worktree_action_pending: false,
            worktrees_next_refresh: Instant::now(),
            agent_observation_inflight: false,
            next_agent_observation: Instant::now(),
            notice: None,
            mutation_busy: false,
            ready_pending: false,
            planning_epics: HashSet::new(),
            planning_seq: 0,
            modal_ready: true,
            paste_seq: 0,
            paste_pending: false,
            created_screenshots: Vec::new(),
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
    #[test]
    fn ready_group_is_distinct_while_beads_status_stays_open() {
        let mut a = app();
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL]);
        assert_eq!(a.beads[0].status, "open");
        assert_eq!(
            group_statuses(&a.beads),
            [
                "open",
                "ready",
                "blocked",
                "deferred",
                "in_progress",
                "closed"
            ]
        );
        let layout = list_layout(&a.beads, &HashSet::new());
        assert_eq!(layout.groups[1].indices, [0]);
        a.select(0);
        assert_eq!(a.focused_status().as_deref(), Some("ready"));
    }

    #[test]
    fn scheduler_uses_selected_harness_once_and_keeps_ready_after_launch() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:opencode"]);
        a.poll_scheduler();
        let Request::Discover(seq) = rx.try_recv().unwrap() else {
            panic!("discovery not queued")
        };
        a.poll_scheduler();
        assert!(rx.try_recv().is_err());
        let (out, responses) = mpsc::channel();
        a.responses = responses;
        out.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        let Request::LaunchReady(sent_seq, _, agent, bead) = rx.try_recv().unwrap() else {
            panic!("launch not queued")
        };
        assert_eq!(sent_seq, seq);
        assert_eq!(agent.id, "opencode");
        assert_eq!(bead.id, "b0");
        out.send(Response::Launched(
            seq,
            Ok(LaunchOutcome {
                message: "Sent".into(),
                reserved: false,
            }),
        ))
        .unwrap();
        a.receive();
        assert!(rx.try_recv().is_err(), "handoff must not clear Ready");
        assert_eq!(display_status(&a.beads[0]), "ready");
        assert!(a.known_handoffs.contains("b0"));
        assert!(a.accepted_handoffs.contains("b0"));
        a.poll_scheduler();
        assert!(rx.try_recv().is_err());
        a.beads[0].status = "in_progress".into();
        assert_eq!(display_status(&a.beads[0]), "in_progress");
    }

    #[test]
    fn ready_epic_prepares_parent_before_unlabelled_child_launch() {
        let mut a = app();
        let (requests, queued) = mpsc::channel();
        a.requests = requests;
        let (replies, responses) = mpsc::channel();
        a.responses = responses;
        a.beads[0].data["issue_type"] = json!("epic");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL]);
        a.beads[1].data["issue_type"] = json!("bug");
        a.beads[1].data["parent"] = json!("b0");
        a.beads[1].data["labels"] = json!([beads::READY_LABEL]);
        a.settings.default_agent = DefaultAgent::Kind("opencode");
        a.settings.agent_limit = settings::AgentLimit::Limited(1);
        a.worktrees.push(ManagedWorktree {
            id: "active".into(),
            bead_id: "other".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/other".into(),
            target_branch: "main".into(),
            workspace_id: "ws-other".into(),
            status: worktrees::WorktreeStatus::InProgress,
            detail: None,
        });

        a.poll_scheduler();
        let Request::Discover(parent_seq) = queued.try_recv().unwrap() else {
            panic!("parent discovery not queued")
        };
        a.poll_scheduler();
        assert!(queued.try_recv().is_err());
        replies
            .send(Response::Discovered(parent_seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(matches!(queued.try_recv(), Ok(Request::Launch(_, _, _, bead)) if bead.id == "b0"));
        replies
            .send(Response::Launched(
                parent_seq,
                Ok(LaunchOutcome {
                    message: "Prepared".into(),
                    reserved: true,
                }),
            ))
            .unwrap();
        a.receive();
        assert!(a.known_handoffs.contains("b0"));
        a.poll_scheduler();
        assert!(queued.try_recv().is_err());
        replies
            .send(Response::Worktrees(Ok(vec![ManagedWorktree {
                id: "parent".into(),
                bead_id: "b0".into(),
                parent_bead_id: None,
                handoff_accepted: false,
                bead_title: None,
                bead_description: None,
                branch: "ontrack/parent".into(),
                target_branch: "main".into(),
                workspace_id: "ws-parent".into(),
                status: worktrees::WorktreeStatus::Reserved,
                detail: None,
            }])))
            .unwrap();
        a.receive();
        a.poll_scheduler();
        let Request::Discover(child_seq) = queued.try_recv().unwrap() else {
            panic!("child discovery not queued")
        };
        replies
            .send(Response::Discovered(child_seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(
            matches!(queued.try_recv(), Ok(Request::LaunchReady(_, _, agent, bead))
            if bead.id == "b1" && agent.id == "opencode")
        );
    }

    #[test]
    fn explicit_unassigned_ready_task_is_not_scheduled() {
        let mut a = app();
        let (requests, queued) = mpsc::channel();
        a.requests = requests;
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL, beads::UNASSIGNED_LABEL]);
        a.poll_scheduler();
        assert!(queued.try_recv().is_err());
        assert!(a.notice.as_deref().unwrap().contains("Unassigned"));
        a.poll_scheduler();
        assert!(queued.try_recv().is_err());
    }

    #[test]
    fn failed_epic_preparation_is_not_retried_or_duplicated() {
        let mut a = app();
        let (requests, queued) = mpsc::channel();
        a.requests = requests;
        let (replies, responses) = mpsc::channel();
        a.responses = responses;
        a.beads[0].data["issue_type"] = json!("epic");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL]);
        a.beads[1].data["issue_type"] = json!("task");
        a.beads[1].data["parent"] = json!("b0");
        a.beads[1].data["labels"] = json!([beads::READY_LABEL]);
        a.poll_scheduler();
        let Request::Discover(seq) = queued.try_recv().unwrap() else {
            panic!("parent discovery not queued")
        };
        replies
            .send(Response::Discovered(
                seq,
                Err(std::io::Error::other("Herdr unavailable")),
            ))
            .unwrap();
        a.receive();
        a.poll_scheduler();
        assert!(queued.try_recv().is_err());
        assert!(a.notice.as_deref().unwrap().contains("Herdr unavailable"));

        a.scheduler_attempted.clear();
        a.known_handoffs.insert("b0".into());
        a.poll_scheduler();
        assert!(queued.try_recv().is_err());
    }

    #[test]
    fn scheduler_defers_unavailable_harness_and_existing_handoffs() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:gemini"]);
        a.poll_scheduler();
        let Request::Discover(seq) = rx.try_recv().unwrap() else {
            panic!()
        };
        let (out, responses) = mpsc::channel();
        a.responses = responses;
        out.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(rx.try_recv().is_err());
        assert!(a.notice.as_deref().unwrap().contains("unavailable"));
        a.poll_scheduler();
        assert!(rx.try_recv().is_err());
        a.scheduler_attempted.remove("b0");
        a.known_handoffs.insert("b0".into());
        a.poll_scheduler();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn scheduler_waits_at_limit_and_resumes_after_active_handoff_finishes() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.settings.agent_limit = settings::AgentLimit::Limited(1);
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:codex"]);
        a.worktrees.push(ManagedWorktree {
            id: "manual".into(),
            bead_id: "other".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/other".into(),
            target_branch: "main".into(),
            workspace_id: "ws-other".into(),
            status: worktrees::WorktreeStatus::DirtyCheckout,
            detail: None,
        });
        a.poll_scheduler();
        assert!(rx.try_recv().is_err());
        assert!(!a.scheduler_attempted.contains("b0"));
        let (out, responses) = mpsc::channel();
        a.responses = responses;
        let mut finished = a.worktrees[0].clone();
        finished.status = worktrees::WorktreeStatus::Unmerged;
        out.send(Response::Worktrees(Ok(vec![finished]))).unwrap();
        a.receive();
        a.poll_scheduler();
        assert!(matches!(rx.try_recv(), Ok(Request::Discover(_))));
        assert_eq!(a.handoff.values().next().unwrap().bead.id, "b0");
    }

    #[test]
    fn unlimited_scheduler_ignores_active_handoff_count() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.settings.agent_limit = settings::AgentLimit::Unlimited;
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:codex"]);
        a.worktrees.push(ManagedWorktree {
            id: "active".into(),
            bead_id: "other".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/other".into(),
            target_branch: "main".into(),
            workspace_id: "ws-other".into(),
            status: worktrees::WorktreeStatus::InProgress,
            detail: None,
        });
        a.poll_scheduler();
        assert!(matches!(rx.try_recv(), Ok(Request::Discover(_))));
    }
    #[test]
    fn scheduler_skips_ready_task_with_journaled_handoff_after_restart() {
        let mut a = app();
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:codex"]);
        let (requests, rx) = mpsc::channel();
        a.requests = requests;
        let (responses, incoming) = mpsc::channel();
        a.responses = incoming;
        responses
            .send(Response::Worktrees(Ok(vec![ManagedWorktree {
                id: "handoff".into(),
                bead_id: "b0".into(),
                parent_bead_id: None,
                handoff_accepted: true,
                bead_title: None,
                bead_description: None,
                branch: "ontrack/b0".into(),
                target_branch: "main".into(),
                workspace_id: "ws".into(),
                status: worktrees::WorktreeStatus::InProgress,
                detail: None,
            }])))
            .unwrap();
        a.receive();
        assert_eq!(display_status(&a.beads[0]), "ready");
        assert!(a.known_handoffs.contains("b0"));
        a.poll_scheduler();
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn ready_toggle_uses_r_in_both_panes_and_does_not_change_beads_status() {
        let mut a = app();
        a.beads[0].data["issue_type"] = json!("task");
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        press(&mut a, KeyCode::Char('r'));
        assert!(matches!(rx.try_recv(), Ok(Request::SetReady(id, true)) if id == "b0"));
        assert!(rx.try_recv().is_err());
        assert!(!a.refresh_pending);
        assert_eq!(a.beads[0].status, "open");
        a.mutation_busy = false;
        a.ready_pending = false;
        a.beads[0].data["labels"] = json!([beads::READY_LABEL]);
        a.focus_detail = true;
        press(&mut a, KeyCode::Char('r'));
        assert!(matches!(rx.try_recv(), Ok(Request::SetReady(id, false)) if id == "b0"));
        assert!(rx.try_recv().is_err());
        assert!(!a.refresh_pending);
        a.mutation_busy = false;
        a.ready_pending = false;
        press(&mut a, KeyCode::Char('R'));
        assert!(matches!(rx.try_recv(), Ok(Request::SetReady(id, false)) if id == "b0"));
    }

    #[test]
    fn non_task_types_toggle_ready_and_keep_handoff_protection() {
        for issue_type in ["bug", "feature", "chore", "custom"] {
            let (mut app, requests, _) = connected_app();
            app.beads[0].data["issue_type"] = json!(issue_type);
            press(&mut app, KeyCode::Char('r'));
            assert!(matches!(requests.try_recv(), Ok(Request::SetReady(id, true)) if id == "b0"));
            app.mutation_busy = false;
            app.ready_pending = false;
            app.beads[0].data["labels"] = json!([beads::READY_LABEL]);
            assert_eq!(display_status(&app.beads[0]), "ready");
            assert!(
                list_layout(&app.beads, &HashSet::new())
                    .groups
                    .iter()
                    .any(|group| { group.status == "ready" && group.indices.contains(&0) })
            );
            app.accepted_handoffs.insert("b0".into());
            press(&mut app, KeyCode::Char('R'));
            assert!(requests.try_recv().is_err());
            assert!(
                app.notice
                    .as_deref()
                    .unwrap()
                    .contains("Ready cannot be removed")
            );
            app.accepted_handoffs.clear();
            press(&mut app, KeyCode::Char('R'));
            assert!(matches!(requests.try_recv(), Ok(Request::SetReady(id, false)) if id == "b0"));
            assert_eq!(app.beads[0].status, "open");
        }
    }

    #[test]
    fn non_task_ready_issue_enters_scheduled_handoff() {
        for issue_type in ["bug", "feature", "custom"] {
            let (mut app, requests, responses) = connected_app();
            app.beads[0].data["issue_type"] = json!(issue_type);
            app.beads[0].data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:opencode"]);
            app.poll_scheduler();
            let Request::Discover(seq) = requests.try_recv().unwrap() else {
                panic!("Ready issue did not enter handoff discovery");
            };
            responses
                .send(Response::Discovered(seq, Ok(launch_context())))
                .unwrap();
            app.receive();
            assert!(matches!(
                requests.try_recv(),
                Ok(Request::LaunchReady(_, _, agent, bead))
                    if agent.id == "opencode" && bead.id == "b0"
            ));
        }
    }

    #[test]
    fn pending_ready_keeps_browser_input_live_and_excludes_mutations() {
        for epic in [false, true] {
            for ready in [false, true] {
                let (mut a, requests, replies) = connected_app();
                a.beads[0].data["issue_type"] = json!(if epic { "epic" } else { "task" });
                a.beads[1].data["issue_type"] = json!("task");
                if !ready {
                    a.beads[0].data["labels"] = json!([beads::READY_LABEL]);
                    a.beads[1].data["labels"] = json!([beads::READY_LABEL]);
                }
                press(&mut a, KeyCode::Char('r'));
                assert!(a.ready_pending && a.mutation_busy);
                assert!(a.notice.as_deref().unwrap().contains("Updating Ready"));
                assert!(matches!(
                    requests.try_recv(),
                    Ok(Request::SetReady(..) | Request::SetEpicReady(..))
                ));
                for key in ['r', 'R', 'n', 'N', 'e', 'd', 'g', 'h', 's', 'u'] {
                    press(&mut a, KeyCode::Char(key));
                }
                a.poll_scheduler();
                assert!(requests.try_recv().is_err());
                assert!(a.modal.is_none());
                press(&mut a, KeyCode::Down);
                assert_ne!(a.selected, Some(0));
                assert!(matches!(requests.try_recv(), Ok(Request::Show(..))));
                press(&mut a, KeyCode::Tab);
                press(&mut a, KeyCode::Down);
                assert!(a.focus_detail);
                assert_eq!(a.detail_offset, 1);
                // Ctrl+? opens help while a Ready update is still blocked.
                a.key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::CONTROL));
                assert!(a.help.is_some());
                press(&mut a, KeyCode::Esc);
                assert!(a.help.is_none());
                assert!(a.key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)));
                assert_eq!(beads::is_ready(&a.beads[0]), !ready);
                replies
                    .send(Response::ReadySet(
                        "b0".into(),
                        ready,
                        Ok(vec!["b0".into()]),
                    ))
                    .unwrap();
                a.receive();
                assert!(!a.ready_pending && !a.mutation_busy);
                assert_eq!(beads::is_ready(&a.beads[0]), ready);
                assert!(matches!(requests.try_recv(), Ok(Request::List)));
            }
        }
    }

    #[test]
    fn ready_queue_failure_releases_input_and_mutation_state() {
        let mut a = app(); // Its request receiver has been dropped.
        a.beads[0].data["issue_type"] = json!("task");
        press(&mut a, KeyCode::Char('r'));
        assert!(!a.ready_pending && !a.mutation_busy);
        assert!(a.notice.as_deref().unwrap().contains("could not be queued"));
        assert!(a.key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)));
    }

    #[test]
    fn blocked_ready_does_not_delay_browser_reads_or_managed_work() {
        for epic in [false, true] {
            let (release, blocked) = mpsc::channel();
            let blocked = std::sync::Mutex::new(blocked);
            let (entered, started) = mpsc::channel();
            let (requests, responses, _) = start_workers(move |request, out| {
                let response = match request {
                    Request::SetReady(id, ready) | Request::SetEpicReady(id, ready, ..) => {
                        entered.send(()).unwrap();
                        // Timeout also releases the worker if a regression fails the test.
                        blocked
                            .lock()
                            .unwrap()
                            .recv_timeout(Duration::from_secs(10))
                            .unwrap();
                        Response::ReadySet(id.clone(), ready, Ok(vec![id]))
                    }
                    Request::List => Response::List(Ok(vec![])),
                    Request::Show(id, seq) => Response::Show(id, seq, Ok(json!({}))),
                    Request::ListWorktrees => Response::Worktrees(Ok(vec![])),
                    _ => panic!("unexpected request"),
                };
                out.send(response).unwrap();
            });
            requests
                .send(if epic {
                    Request::SetEpicReady("epic".into(), true, HashSet::new(), HashSet::new())
                } else {
                    Request::SetReady("task".into(), false)
                })
                .unwrap();
            started.recv_timeout(Duration::from_secs(5)).unwrap();
            requests.send(Request::List).unwrap();
            requests.send(Request::Show("other".into(), 1)).unwrap();
            requests.send(Request::ListWorktrees).unwrap();
            let mut observed = HashSet::new();
            for _ in 0..3 {
                observed.insert(
                    match responses.recv_timeout(Duration::from_secs(5)).unwrap() {
                        Response::List(Ok(_)) => "list",
                        Response::Show(_, _, Ok(_)) => "detail",
                        Response::Worktrees(Ok(_)) => "managed",
                        _ => panic!("Ready must still be blocked"),
                    },
                );
            }
            assert_eq!(observed.len(), 3);
            release.send(()).unwrap();
            assert!(matches!(
                responses.recv_timeout(Duration::from_secs(5)),
                Ok(Response::ReadySet(_, _, Ok(_)))
            ));
        }
    }

    #[test]
    fn ready_key_respects_closed_and_handoff_guards() {
        let (mut a, rx, _) = connected_app();
        a.beads[0].data["issue_type"] = json!("bug");
        a.beads[0].status = "closed".into();
        press(&mut a, KeyCode::Char('r'));
        assert_eq!(
            a.notice.as_deref(),
            Some("Only open issues can be marked Ready")
        );
        assert!(rx.try_recv().is_err());

        a.beads[0].status = "open".into();
        a.beads[0].data["issue_type"] = json!("task");
        a.known_handoffs.insert("b0".into());
        press(&mut a, KeyCode::Char('r'));
        assert_eq!(
            a.notice.as_deref(),
            Some("This issue already has a tracked handoff")
        );
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn sent_ready_task_cannot_be_unreadied_but_unsent_handoff_can() {
        let (mut a, rx, _) = connected_app();
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL]);
        a.known_handoffs.insert("b0".into());
        a.accepted_handoffs.insert("b0".into());
        for focus_detail in [false, true] {
            a.focus_detail = focus_detail;
            press(&mut a, KeyCode::Char('r'));
            assert!(rx.try_recv().is_err());
            assert!(
                a.notice
                    .as_deref()
                    .unwrap()
                    .contains("Ready cannot be removed")
            );
            assert!(beads::is_ready(&a.beads[0]));
        }

        a.accepted_handoffs.clear();
        a.worktrees_discovered = false;
        press(&mut a, KeyCode::Char('r'));
        assert!(rx.try_recv().is_err());
        assert!(
            a.notice
                .as_deref()
                .unwrap()
                .contains("tracked handoffs to load")
        );
        a.worktrees_discovered = true;
        press(&mut a, KeyCode::Char('r'));
        assert!(matches!(rx.try_recv(), Ok(Request::SetReady(id, false)) if id == "b0"));
    }

    #[test]
    fn ready_task_cannot_be_unreadied_during_handoff() {
        let (mut a, rx, _) = connected_app();
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL]);
        press(&mut a, KeyCode::Char('h'));
        assert!(matches!(rx.try_recv(), Ok(Request::Discover(_))));
        press(&mut a, KeyCode::Char('r'));
        assert!(rx.try_recv().is_err());
        assert!(
            a.notice
                .as_deref()
                .unwrap()
                .contains("Ready cannot be removed")
        );
    }

    #[test]
    fn epic_ready_toggle_queues_cascade_and_only_tasks_are_scheduled() {
        let mut a = app();
        a.beads[0].data["issue_type"] = json!("epic");
        a.beads[1].data["issue_type"] = json!("task");
        a.beads[1].data["parent"] = json!("b0");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL]);
        assert_eq!(display_status(&a.beads[0]), "ready");
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.poll_scheduler();
        assert!(rx.try_recv().is_err());

        a.beads[0].data["labels"] = json!([]);
        a.known_handoffs.insert("b2".into());
        press(&mut a, KeyCode::Char('r'));
        assert!(matches!(
            rx.try_recv(),
            Ok(Request::SetEpicReady(id, true, handoffs, protected))
                if id == "b0" && handoffs.contains("b2") && protected.is_empty()
        ));
        assert_eq!(a.beads[0].status, "open");
    }

    #[test]
    fn epic_ready_response_updates_confirmed_ids_and_refreshes_after_failure() {
        let mut a = app();
        a.beads[0].data["issue_type"] = json!("epic");
        a.beads[1].data["issue_type"] = json!("task");
        let (requests, queued) = mpsc::channel();
        a.requests = requests;
        let (replies, responses) = mpsc::channel();
        a.responses = responses;
        a.mutation_busy = true;
        a.ready_pending = true;
        replies
            .send(Response::ReadySet(
                "b0".into(),
                true,
                Ok(vec!["b0".into(), "b1".into()]),
            ))
            .unwrap();
        a.receive();
        assert!(a.beads.iter().take(2).all(beads::is_ready));
        assert!(!a.mutation_busy);
        assert!(matches!(queued.try_recv(), Ok(Request::List)));

        a.refresh_inflight = false;
        a.mutation_busy = true;
        a.ready_pending = true;
        replies
            .send(Response::ReadySet(
                "b0".into(),
                false,
                Err(std::io::Error::other("partial update")),
            ))
            .unwrap();
        a.receive();
        assert!(matches!(queued.try_recv(), Ok(Request::List)));
        assert!(a.notice.as_deref().unwrap().contains("partial update"));
        assert!(!a.ready_pending && !a.mutation_busy);
    }

    #[test]
    fn prepared_parent_waits_for_children_then_can_resume() {
        let mut a = app();
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:codex"]);
        a.beads[1].data["parent"] = json!("b0");
        a.known_handoffs.insert("b0".into());
        a.worktrees.push(ManagedWorktree {
            id: "handoff".into(),
            bead_id: "b0".into(),
            parent_bead_id: None,
            handoff_accepted: false,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/parent".into(),
            target_branch: "main".into(),
            workspace_id: "w1".into(),
            status: worktrees::WorktreeStatus::Reserved,
            detail: None,
        });
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.poll_scheduler();
        assert!(rx.try_recv().is_err());
        a.beads[1].status = "closed".into();
        a.poll_scheduler();
        assert!(matches!(rx.try_recv(), Ok(Request::Discover(_))));
    }

    #[test]
    fn ready_child_waits_for_parent_checkout_even_at_higher_priority() {
        let mut a = app();
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:codex"]);
        a.beads[0].priority = 3;
        a.beads[1].data["issue_type"] = json!("task");
        a.beads[1].data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:codex"]);
        a.beads[1].data["parent"] = json!("b0");
        a.beads[1].priority = 0;
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.poll_scheduler();
        assert!(matches!(rx.try_recv(), Ok(Request::Discover(_))));
        assert_eq!(
            a.handoff.values().next().map(|item| item.bead.id.as_str()),
            Some("b0")
        );
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
    fn observed_row(bead: &str, status: worktrees::WorktreeStatus) -> ManagedWorktree {
        ManagedWorktree {
            id: format!("handoff-{bead}"),
            bead_id: bead.into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: format!("ontrack/{bead}"),
            target_branch: "main".into(),
            workspace_id: "wTask".into(),
            status,
            detail: Some("Keep recovery evidence".into()),
        }
    }

    #[test]
    fn blocked_launch_does_not_delay_ready_launch_browser_create_or_managed_refresh() {
        use std::sync::Barrier;
        let gate = Arc::new(Barrier::new(2));
        let worker_gate = gate.clone();
        let (entered, started) = mpsc::channel();
        let (requests, responses, _) = start_workers(move |request, out| {
            let response = match request {
                Request::Launch(seq, ..) => {
                    entered.send(()).unwrap();
                    worker_gate.wait();
                    Response::Launched(seq, Err(std::io::Error::other("first failed")))
                }
                Request::LaunchReady(seq, ..) => Response::Launched(
                    seq,
                    Ok(LaunchOutcome {
                        reserved: false,
                        message: "second accepted".into(),
                    }),
                ),
                Request::List => Response::List(Ok(vec![])),
                Request::Create(_) => Response::Created(Ok("created".into())),
                Request::ListWorktrees => Response::Worktrees(Ok(vec![])),
                _ => panic!("unexpected request"),
            };
            out.send(response).unwrap();
        });
        let context = launch_context();
        let bead = app().beads[0].clone();
        requests
            .send(Request::Launch(
                1,
                context.clone(),
                context.agents[0].clone(),
                bead.clone(),
            ))
            .unwrap();
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        requests
            .send(Request::LaunchReady(
                2,
                context.clone(),
                context.agents[0].clone(),
                bead,
            ))
            .unwrap();
        requests.send(Request::List).unwrap();
        requests
            .send(Request::Create(CreateBead {
                title: "new".into(),
                description: String::new(),
                issue_type: "task".into(),
                priority: 2,
                harness: None,
            }))
            .unwrap();
        requests.send(Request::ListWorktrees).unwrap();
        let mut observed = HashSet::new();
        for _ in 0..4 {
            observed.insert(
                match responses.recv_timeout(Duration::from_secs(5)).unwrap() {
                    Response::Launched(2, Ok(_)) => "ready",
                    Response::List(Ok(_)) => "browser",
                    Response::Created(Ok(_)) => "create",
                    Response::Worktrees(Ok(_)) => "managed",
                    _ => panic!("first launch remains blocked"),
                },
            );
        }
        assert_eq!(observed.len(), 4);
        gate.wait();
        assert!(matches!(
            responses.recv_timeout(Duration::from_secs(5)),
            Ok(Response::Launched(1, Err(_)))
        ));
    }

    #[test]
    fn managed_work_is_serial_and_cannot_occupy_the_browser_worker() {
        use std::sync::Barrier;
        let gate = Arc::new(Barrier::new(2));
        let worker_gate = gate.clone();
        let (tx, started) = mpsc::channel();
        let (steps, received_steps) = mpsc::channel();
        let (requests, responses, _) = start_workers(move |request, out| {
            let response = match request {
                Request::WorktreeAction(_, _) => {
                    tx.send(()).unwrap();
                    worker_gate.wait();
                    steps.send(1).unwrap();
                    Response::WorktreeChanged(Err(std::io::Error::other("finished")))
                }
                Request::ListWorktrees => {
                    steps.send(2).unwrap();
                    Response::Worktrees(Ok(vec![]))
                }
                Request::List => Response::List(Ok(vec![])),
                _ => panic!("unexpected request"),
            };
            out.send(response).unwrap();
        });
        requests
            .send(Request::WorktreeAction(
                "first".into(),
                WorktreeAction::RequestMerge,
            ))
            .unwrap();
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        requests.send(Request::ListWorktrees).unwrap();
        requests.send(Request::List).unwrap();
        assert!(matches!(
            responses.recv_timeout(Duration::from_secs(5)),
            Ok(Response::List(Ok(_)))
        ));
        assert!(received_steps.try_recv().is_err());
        gate.wait();
        assert_eq!(
            received_steps.recv_timeout(Duration::from_secs(5)).unwrap(),
            1
        );
        assert_eq!(
            received_steps.recv_timeout(Duration::from_secs(5)).unwrap(),
            2
        );
    }

    #[test]
    fn distinct_manual_and_ready_results_do_not_clear_other_pending_state_or_toast() {
        let (mut a, rx, tx) = connected_app();
        a.start_launch(a.beads[0].clone(), false);
        let Request::Discover(first) = rx.try_recv().unwrap() else {
            panic!()
        };
        a.start_launch(a.beads[1].clone(), true);
        let Request::Discover(second) = rx.try_recv().unwrap() else {
            panic!()
        };
        a.start_launch(a.beads[0].clone(), false);
        assert!(rx.try_recv().is_err());
        tx.send(Response::Discovered(first, Ok(launch_context())))
            .unwrap();
        tx.send(Response::Discovered(second, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(matches!(rx.try_recv(), Ok(Request::Launch(seq,..)) if seq == first));
        assert!(matches!(rx.try_recv(), Ok(Request::LaunchReady(seq,..)) if seq == second));
        let toast = a.toast.as_ref().unwrap().message.clone();
        a.worktrees_inflight = true;
        tx.send(Response::Launched(
            first,
            Ok(LaunchOutcome {
                message: "first success".into(),
                reserved: false,
            }),
        ))
        .unwrap();
        a.receive();
        assert_eq!(a.handoff.len(), 1);
        assert_eq!(a.handoff[&second].bead.id, "b1");
        assert_eq!(a.toast.as_ref().unwrap().message, toast);
        assert!(a.worktrees_refresh_pending);
        assert!(a.handoff_capacity.contains("b0"));
        tx.send(Response::Launched(
            first,
            Err(std::io::Error::other("stale")),
        ))
        .unwrap();
        a.receive();
        assert_eq!(a.toast.as_ref().unwrap().message, toast);
        tx.send(Response::Launched(
            second,
            Err(std::io::Error::other("second uncertain")),
        ))
        .unwrap();
        a.receive();
        assert!(a.handoff.is_empty());
        assert!(
            a.toast
                .as_ref()
                .unwrap()
                .message
                .contains("second uncertain")
        );
        tx.send(Response::Worktrees(Ok(vec![]))).unwrap();
        a.receive();
        assert!(
            !a.worktrees_discovered,
            "a stale scan requires a trailing durable refresh"
        );
        assert!(a.handoff_capacity.contains("b0"));
        a.poll_worktrees(Instant::now());
        assert!(matches!(rx.try_recv(), Ok(Request::ListWorktrees)));
        tx.send(Response::Worktrees(Ok(vec![observed_row(
            "b0",
            worktrees::WorktreeStatus::InProgress,
        )])))
        .unwrap();
        a.receive();
        assert!(!a.handoff_capacity.contains("b0"));
    }

    #[test]
    fn ready_capacity_counts_all_pending_agents_but_not_epic_checkout_reservations() {
        let (mut a, rx, _) = connected_app();
        a.settings.agent_limit = settings::AgentLimit::Limited(1);
        for bead in &mut a.beads {
            bead.data["issue_type"] = json!("task");
            bead.data["labels"] = json!([beads::READY_LABEL, "ontrack:agent:codex"]);
        }
        a.beads[0].data["issue_type"] = json!("epic");
        a.beads[1].data["parent"] = json!("b0");
        a.poll_scheduler();
        assert!(matches!(rx.try_recv(), Ok(Request::Discover(_))));
        assert!(a.handoff.values().any(|pending| pending.bead.id == "b0"));
        a.poll_scheduler();
        assert!(
            matches!(rx.try_recv(), Ok(Request::Discover(_))),
            "reserving an Epic's parent checkout consumes no agent slot"
        );
        assert!(a.handoff.values().any(|pending| pending.bead.id == "b2"));
        a.poll_scheduler();
        assert!(
            rx.try_recv().is_err(),
            "pending agent launch consumes the remaining slot"
        );
        a.selected = Some(2);
        a.toggle_ready();
        assert!(
            rx.try_recv().is_err(),
            "Ready cannot be removed from any pending task"
        );
        a.selected = Some(0);
        a.toggle_ready();
        assert!(
            matches!(rx.try_recv(), Ok(Request::SetEpicReady(_,false,_,protected)) if protected.contains("b0") && protected.contains("b2"))
        );
    }

    #[test]
    fn display_only_agent_polling_is_bounded_and_preserves_nonactive_worktree_state() {
        let (mut a, rx, tx) = connected_app();
        a.worktrees.push(observed_row(
            "b0",
            worktrees::WorktreeStatus::NeedsAttention,
        ));
        let now = Instant::now();
        a.poll_agent_observations(now);
        a.poll_agent_observations(now + Duration::from_secs(50));
        assert!(matches!(rx.try_recv(), Ok(Request::ObserveAgents)));
        assert!(rx.try_recv().is_err());
        tx.send(Response::AgentsObserved(Err(std::io::Error::other(
            "offline",
        ))))
        .unwrap();
        a.receive_at(now);
        assert_eq!(a.worktrees.len(), 1);
        assert_eq!(
            a.worktrees[0].status,
            worktrees::WorktreeStatus::NeedsAttention
        );
        assert!(
            a.worktrees[0]
                .detail
                .as_ref()
                .unwrap()
                .contains("Keep recovery evidence")
        );
        assert!(a.worktrees[0].detail.as_ref().unwrap().contains("unknown"));
        a.poll_agent_observations(now + Duration::from_secs(1));
        assert!(rx.try_recv().is_err());
        a.poll_agent_observations(now + Duration::from_secs(2));
        assert!(matches!(rx.try_recv(), Ok(Request::ObserveAgents)));
        tx.send(Response::AgentsObserved(Ok(vec![
            worktrees::AgentDisplay {
                id: "handoff-b0".into(),
                workspace_id: "wTask".into(),
                detail: "Agent observation (display only): working".into(),
                blocked: false,
            },
        ])))
        .unwrap();
        a.receive_at(now + Duration::from_secs(2));
        assert_eq!(
            a.worktrees[0].status,
            worktrees::WorktreeStatus::NeedsAttention
        );
        assert!(a.worktrees[0].detail.as_ref().unwrap().contains("working"));
        assert!(!a.worktrees[0].detail.as_ref().unwrap().contains("offline"));
        tx.send(Response::Worktrees(Ok(vec![observed_row(
            "b0",
            worktrees::WorktreeStatus::NeedsAttention,
        )])))
        .unwrap();
        a.receive_at(now + Duration::from_secs(2));
        assert!(
            a.worktrees[0]
                .detail
                .as_ref()
                .unwrap()
                .contains("Agent observation (display only): working")
        );
    }
    #[test]
    fn verified_agent_failure_blocks_only_an_in_progress_worktree() {
        let (mut app, _requests, responses) = connected_app();
        app.worktrees.push(observed_row(
            "active",
            worktrees::WorktreeStatus::InProgress,
        ));
        app.worktrees.push(observed_row(
            "dirty",
            worktrees::WorktreeStatus::DirtyCheckout,
        ));
        let now = Instant::now();

        for state in ["error", "blocked"] {
            app.agent_observation_inflight = true;
            responses
                .send(Response::AgentsObserved(Ok(vec![
                    worktrees::AgentDisplay {
                        id: "handoff-active".into(),
                        workspace_id: "wTask".into(),
                        detail: format!("Agent observation (display only): {state}"),
                        blocked: true,
                    },
                    worktrees::AgentDisplay {
                        id: "handoff-dirty".into(),
                        workspace_id: "wTask".into(),
                        detail: format!("Agent observation (display only): {state}"),
                        blocked: true,
                    },
                ])))
                .unwrap();
            app.receive_at(now);
            assert_eq!(app.worktrees[0].status, worktrees::WorktreeStatus::Blocked);
            assert_eq!(worktree_status_label(app.worktrees[0].status), "BLOCKED");
            assert!(app.worktrees[0].detail.as_deref().unwrap().contains(state));
            assert_eq!(
                app.worktrees[1].status,
                worktrees::WorktreeStatus::DirtyCheckout
            );
        }

        app.agent_observation_inflight = true;
        responses
            .send(Response::Worktrees(Ok(vec![
                observed_row("active", worktrees::WorktreeStatus::InProgress),
                observed_row("dirty", worktrees::WorktreeStatus::DirtyCheckout),
            ])))
            .unwrap();
        app.receive_at(now);
        assert_eq!(app.worktrees[0].status, worktrees::WorktreeStatus::Blocked);

        responses
            .send(Response::AgentsObserved(Err(std::io::Error::other(
                "offline",
            ))))
            .unwrap();
        app.receive_at(now);
        assert_eq!(
            app.worktrees[0].status,
            worktrees::WorktreeStatus::InProgress
        );
        assert!(
            app.worktrees[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("unknown")
        );
        app.agent_observation_inflight = true;
        responses
            .send(Response::AgentsObserved(Ok(vec![
                worktrees::AgentDisplay {
                    id: "handoff-active".into(),
                    workspace_id: "wTask".into(),
                    detail: "Agent observation (display only): working".into(),
                    blocked: false,
                },
            ])))
            .unwrap();
        app.receive_at(now);
        assert_eq!(
            app.worktrees[0].status,
            worktrees::WorktreeStatus::InProgress
        );
        assert!(
            app.worktrees[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("working")
        );
        assert_eq!(
            app.worktrees[1].status,
            worktrees::WorktreeStatus::DirtyCheckout
        );
    }
    #[test]
    fn handoff_discovers_and_launches_once_without_dialog_or_confirmation() {
        let (mut a, rx, tx) = connected_app();
        a.beads[0].data["description"] = json!("Full context");
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => panic!("expected discovery"),
        };
        assert!(a.modal.is_none());
        assert!(!a.handoff.is_empty());
        assert!(
            a.toast
                .as_ref()
                .is_some_and(|toast| toast.expires_at.is_none())
        );
        press(&mut a, KeyCode::Char('h'));
        assert!(rx.try_recv().is_err());
        press(&mut a, KeyCode::Down);
        assert_eq!(a.selected, Some(1), "browsing remains available");
        assert!(matches!(rx.try_recv(), Ok(Request::Show(..))));
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(
            matches!(rx.try_recv(), Ok(Request::Launch(id, context, agent, bead))
            if id == seq && context.workspace.id == "ws" && agent.id == "codex"
                && bead.id == "b0" && bead.data["description"] == "Full context")
        );
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(
            rx.try_recv().is_err(),
            "duplicate discovery cannot relaunch"
        );
        press(&mut a, KeyCode::Char('h'));
        assert!(
            matches!(rx.try_recv(), Ok(Request::Discover(next)) if next != seq),
            "a distinct selected bead can launch concurrently"
        );
        assert_eq!(a.handoff.len(), 2);
    }

    #[test]
    fn worktree_poll_uses_short_read_scan_during_pending_handoff() {
        let (mut a, requests, _) = connected_app();
        a.worktrees
            .push(observed_row("b3", worktrees::WorktreeStatus::InProgress));
        press(&mut a, KeyCode::Char('h'));
        assert!(matches!(requests.try_recv(), Ok(Request::Discover(_))));
        a.poll_worktrees(Instant::now());
        assert!(matches!(requests.try_recv(), Ok(Request::ListWorktrees)));
    }
    #[test]
    fn handoff_requires_selection_and_ignores_stale_discovery() {
        let (mut a, rx, tx) = connected_app();
        a.clear_selection();
        press(&mut a, KeyCode::Char('h'));
        assert!(rx.try_recv().is_err());
        assert!(a.toast.as_ref().is_some_and(|toast| toast.error));
        a.select(0);
        let _ = rx.try_recv();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(seq + 1, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(rx.try_recv().is_err());
        assert!(!a.handoff.is_empty());
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(matches!(rx.try_recv(), Ok(Request::Launch(..))));
    }
    #[test]
    fn handoff_uses_first_available_agent_if_codex_is_absent() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        let mut context = launch_context();
        context.agents.remove(0);
        tx.send(Response::Discovered(seq, Ok(context))).unwrap();
        a.receive();
        assert!(
            matches!(rx.try_recv(), Ok(Request::Launch(_, _, agent, _)) if agent.id == "opencode")
        );
    }
    #[test]
    fn unavailable_agents_and_discovery_failure_do_not_launch() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        let mut context = launch_context();
        context.agents.clear();
        tx.send(Response::Discovered(seq, Ok(context))).unwrap();
        a.receive();
        assert!(a.handoff.is_empty());
        assert!(a.toast.as_ref().is_some_and(|toast| toast.error));
        assert!(rx.try_recv().is_err());
        press(&mut a, KeyCode::Char('h'));
        let next = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(
            next,
            Err(std::io::Error::other("workspace unavailable")),
        ))
        .unwrap();
        a.receive();
        assert!(a.handoff.is_empty());
        assert!(
            a.toast
                .as_ref()
                .is_some_and(|toast| toast.message.contains("workspace unavailable"))
        );
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn disconnected_handoff_queues_show_toast_and_leave_ui_available() {
        let mut a = app();
        let (requests, rx) = mpsc::channel();
        drop(rx);
        a.requests = requests;
        press(&mut a, KeyCode::Char('h'));
        assert!(a.handoff.is_empty());
        assert!(a.modal.is_none());
        assert!(a.toast.as_ref().is_some_and(|toast| toast.error));

        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        let (requests, receiver) = mpsc::channel();
        drop(receiver);
        a.requests = requests;
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(a.handoff.is_empty());
        assert!(
            a.toast
                .as_ref()
                .is_some_and(|toast| toast.message.contains("could not be queued"))
        );
    }
    #[test]
    fn handoff_result_sets_toast_and_refreshes_tracked_worktrees() {
        let (mut a, rx, tx) = connected_app();
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([beads::READY_LABEL]);
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(matches!(rx.try_recv(), Ok(Request::Launch(..))));
        a.worktrees_inflight = true;
        tx.send(Response::Launched(
            seq,
            Ok(LaunchOutcome {
                message: "Prompt sent · workspace ws-new".into(),
                reserved: false,
            }),
        ))
        .unwrap();
        a.receive();
        assert!(a.handoff.is_empty());
        assert_eq!(display_status(&a.beads[0]), "ready");
        assert!(
            rx.try_recv().is_err(),
            "manual handoff must not clear Ready"
        );
        assert!(a.worktrees_refresh_pending);
        assert!(a.accepted_handoffs.contains("b0"));
        tx.send(Response::Worktrees(Ok(Vec::new()))).unwrap();
        a.receive();
        assert!(a.accepted_handoffs.contains("b0"));
        assert!(a.modal.is_none());
        assert!(
            a.toast
                .as_ref()
                .is_some_and(|toast| !toast.error && toast.expires_at.is_some())
        );
        assert_eq!(a.notice.as_deref(), Some("Prompt sent · workspace ws-new"));
        tx.send(Response::Launched(seq, Err(std::io::Error::other("stale"))))
            .unwrap();
        a.receive();
        assert!(
            !a.toast.as_ref().unwrap().error,
            "stale result cannot replace success"
        );
    }
    #[test]
    fn failed_handoff_shows_recovery_error_without_modal() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('h'));
        let seq = match rx.try_recv().unwrap() {
            Request::Discover(seq) => seq,
            _ => unreachable!(),
        };
        tx.send(Response::Discovered(seq, Ok(launch_context())))
            .unwrap();
        a.receive();
        assert!(matches!(rx.try_recv(), Ok(Request::Launch(..))));
        tx.send(Response::Launched(
            seq,
            Err(std::io::Error::other(
                "worktree may exist; inspect before retrying",
            )),
        ))
        .unwrap();
        a.receive();
        assert!(a.handoff.is_empty());
        assert!(a.modal.is_none());
        assert!(
            a.toast.as_ref().is_some_and(
                |toast| toast.error && toast.message.contains("inspect before retrying")
            )
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

    #[test]
    fn planning_asks_and_sends_custom_answer_without_creating_beads() {
        let (mut a, requests, replies) = connected_app();
        press(&mut a, KeyCode::Char('p'));
        for c in "Improve search".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        assert!(requests.try_recv().is_err());
        press(&mut a, KeyCode::Enter);
        let (seq, marker) = match requests.try_recv().unwrap() {
            Request::PlanLaunch(seq, proposal, marker) => {
                assert_eq!(proposal, "Improve search");
                (seq, marker)
            }
            _ => panic!("expected planning launch"),
        };
        let session = crate::epic::Session {
            workspace_id: "w1".into(),
            tab_id: "w1:t2".into(),
            pane_id: "w1:p2".into(),
        };
        replies
            .send(Response::PlanLaunched(seq, Ok(session.clone())))
            .unwrap();
        a.receive();
        a.poll_planning(Instant::now());
        assert!(
            matches!(requests.try_recv(), Ok(Request::PlanRead(read_seq, _, read_marker)) if read_seq == seq && read_marker == marker)
        );
        replies
            .send(Response::PlanRead(
                seq,
                marker,
                Ok(Some(crate::planning::Message::Question {
                    text: "Which scope?".into(),
                    options: vec!["Small".into(), "Large".into()],
                })),
            ))
            .unwrap();
        a.receive();
        press(&mut a, KeyCode::Tab);
        for c in "Only titles".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        press(&mut a, KeyCode::Enter);
        assert!(
            matches!(requests.try_recv(), Ok(Request::PlanReply(reply_seq, reply_session, answer, _)) if reply_seq == seq && reply_session == session && answer == "Only titles")
        );
        assert!(matches!(
            a.modal,
            Some(Modal::Planning(PlanningForm {
                stage: PlanningStage::Waiting,
                ..
            }))
        ));
    }

    #[test]
    fn planning_repolls_pending_output_until_agent_asks() {
        let (mut a, requests, replies) = connected_app();
        press(&mut a, KeyCode::Char('p'));
        let (seq, marker) = match a.modal.as_ref() {
            Some(Modal::Planning(form)) => (form.seq, form.marker.clone()),
            _ => panic!("expected planning form"),
        };
        if let Some(Modal::Planning(form)) = a.modal.as_mut() {
            form.stage = PlanningStage::Waiting;
        }
        let session = crate::epic::Session {
            workspace_id: "w1".into(),
            tab_id: "w1:t2".into(),
            pane_id: "w1:p2".into(),
        };
        let now = Instant::now();
        replies
            .send(Response::PlanLaunched(seq, Ok(session.clone())))
            .unwrap();
        a.receive_at(now);
        a.poll_planning(now);
        assert!(matches!(
            requests.try_recv(),
            Ok(Request::PlanRead(read_seq, read_session, read_marker))
                if read_seq == seq && read_session == session && read_marker == marker
        ));
        a.poll_planning(now + Duration::from_secs(2));
        assert!(requests.try_recv().is_err(), "one read at a time");

        replies
            .send(Response::PlanRead(seq, marker.clone(), Ok(None)))
            .unwrap();
        a.receive_at(now);
        a.poll_planning(now + Duration::from_secs(2) - Duration::from_nanos(1));
        assert!(requests.try_recv().is_err());
        a.poll_planning(now + Duration::from_secs(2));
        assert!(matches!(requests.try_recv(), Ok(Request::PlanRead(..))));

        replies
            .send(Response::PlanRead(
                seq,
                marker,
                Ok(Some(crate::planning::Message::Question {
                    text: "Which scope?".into(),
                    options: vec!["Small".into(), "Large".into()],
                })),
            ))
            .unwrap();
        a.receive_at(now + Duration::from_secs(2));
        assert!(matches!(
            a.modal,
            Some(Modal::Planning(PlanningForm {
                stage: PlanningStage::Question { .. },
                ..
            }))
        ));
        assert!(
            requests.try_recv().is_err(),
            "polling stops for the question"
        );
    }

    #[test]
    fn planning_review_requires_explicit_confirmation_and_blocks_repeat_create() {
        let (mut a, requests, replies) = connected_app();
        press(&mut a, KeyCode::Char('p'));
        let seq = a.planning_seq;
        let plan = crate::planning::Plan {
            kind: "task".into(),
            root: crate::planning::Item {
                title: "Search".into(),
                description: "Implement search".into(),
                acceptance: "Find a result".into(),
                priority: 2,
            },
            tasks: vec![],
        };
        if let Some(Modal::Planning(form)) = a.modal.as_mut() {
            form.stage = PlanningStage::Review(plan.clone());
        }
        assert!(requests.try_recv().is_err());
        press(&mut a, KeyCode::Char('y'));
        assert!(
            matches!(requests.try_recv(), Ok(Request::PlanCreate(create_seq, submitted)) if create_seq == seq && submitted == plan)
        );
        press(&mut a, KeyCode::Char('y'));
        assert!(requests.try_recv().is_err());
        replies
            .send(Response::PlanCreated(
                seq,
                Err("Created b1; inspect".into()),
            ))
            .unwrap();
        a.receive();
        assert!(matches!(
            a.modal,
            Some(Modal::Planning(PlanningForm {
                stage: PlanningStage::Failed(_),
                ..
            }))
        ));
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none());
        assert!(matches!(requests.try_recv(), Ok(Request::List)));
        assert!(requests.try_recv().is_err());
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
    fn take_harness_discovery(rx: &mpsc::Receiver<Request>) -> u64 {
        match rx.try_recv() {
            Ok(Request::DiscoverHarnesses(seq)) => seq,
            _ => panic!("missing harness discovery"),
        }
    }
    fn supply_harnesses(app: &mut App, seq: u64, kinds: &[&str]) {
        let (tx, responses) = mpsc::channel();
        app.responses = responses;
        tx.send(Response::HarnessesDiscovered(
            seq,
            Ok(kinds.iter().map(|kind| (*kind).to_owned()).collect()),
        ))
        .unwrap();
        app.receive();
    }
    #[test]
    fn new_form_uses_only_discovered_harnesses_and_saves_selected_label() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('n'));
        let seq = take_harness_discovery(&rx);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert!(form.harnesses.is_empty());
        assert!(form.harness_loading);
        let Some(Modal::Create(form)) = a.modal.as_mut() else {
            panic!()
        };
        form.title = "New task".into();
        form.focus = 4;
        press(&mut a, KeyCode::Enter);
        assert!(rx.try_recv().is_err());
        assert!(a.notice.as_deref().unwrap().contains("Checking Herdr"));
        tx.send(Response::HarnessesDiscovered(
            seq,
            Ok(vec!["opencode".into(), "hermes".into()]),
        ))
        .unwrap();
        a.receive();
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.harnesses, ["opencode", "hermes"]);
        assert_eq!(form.harness, 1);
        // Right wraps through the installed kinds and Unassigned.
        let Some(Modal::Create(form)) = a.modal.as_mut() else {
            panic!()
        };
        form.focus = 0;
        for _ in 0..3 {
            press(&mut a, KeyCode::Tab);
        }
        press(&mut a, KeyCode::Right);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.harnesses[form.harness - 1], "hermes");
        press(&mut a, KeyCode::Right);
        press(&mut a, KeyCode::Right);
        let Some(Modal::Create(form)) = a.modal.as_mut() else {
            panic!()
        };
        assert_eq!(form.harness, 1);
        form.title = "New task".into();
        form.focus = 4;
        press(&mut a, KeyCode::Enter);
        assert!(
            matches!(rx.try_recv(), Ok(Request::Create(input)) if input.harness.as_deref() == Some("opencode"))
        );
    }

    #[test]
    fn new_form_defaults_to_codex_when_current_and_unassigned_on_failure() {
        let (mut a, rx, tx) = connected_app();
        press(&mut a, KeyCode::Char('n'));
        let first = take_harness_discovery(&rx);
        tx.send(Response::HarnessesDiscovered(
            first,
            Ok(vec!["pi".into(), "codex".into()]),
        ))
        .unwrap();
        a.receive();
        assert!(matches!(&a.modal, Some(Modal::Create(form)) if form.harness == 2));
        press(&mut a, KeyCode::Esc);
        press(&mut a, KeyCode::Char('n'));
        let second = take_harness_discovery(&rx);
        tx.send(Response::HarnessesDiscovered(
            first,
            Ok(vec!["claude".into()]),
        ))
        .unwrap();
        tx.send(Response::HarnessesDiscovered(
            second,
            Err(std::io::Error::other("offline")),
        ))
        .unwrap();
        a.receive();
        assert!(
            matches!(&a.modal, Some(Modal::Create(form)) if form.harness == 0 && form.harnesses.is_empty() && form.harness_error.as_deref().unwrap().contains("offline"))
        );
    }
    #[test]
    fn saved_default_agent_applies_to_new_and_child_forms_with_safe_fallback() {
        let (mut a, rx, tx) = connected_app();
        a.beads[0].data["issue_type"] = serde_json::json!("task");
        a.settings.default_agent = DefaultAgent::Kind("hermes");
        press(&mut a, KeyCode::Char('n'));
        let seq = take_harness_discovery(&rx);
        tx.send(Response::HarnessesDiscovered(
            seq,
            Ok(vec!["codex".into(), "hermes".into()]),
        ))
        .unwrap();
        a.receive();
        assert!(matches!(&a.modal, Some(Modal::Create(form)) if form.harness == 2));
        press(&mut a, KeyCode::Esc);

        a.settings.default_agent = DefaultAgent::Unassigned;
        press(&mut a, KeyCode::Char('N'));
        let seq = take_harness_discovery(&rx);
        tx.send(Response::HarnessesDiscovered(seq, Ok(vec!["codex".into()])))
            .unwrap();
        a.receive();
        assert!(
            matches!(&a.modal, Some(Modal::Create(form)) if form.parent_id.is_some() && form.harness == 0)
        );
        press(&mut a, KeyCode::Esc);

        a.settings.default_agent = DefaultAgent::Kind("hermes");
        press(&mut a, KeyCode::Char('n'));
        let seq = take_harness_discovery(&rx);
        tx.send(Response::HarnessesDiscovered(seq, Ok(vec!["codex".into()])))
            .unwrap();
        a.receive();
        assert!(matches!(&a.modal, Some(Modal::Create(form)) if form.harness == 1));
        assert_eq!(a.settings.default_agent, DefaultAgent::Kind("hermes"));

        let existing = serde_json::json!({
            "title": "Existing task",
            "issue_type": "task",
            "priority": 2,
            "labels": ["ontrack:agent:codex"]
        });
        let edit = edit_form(&existing).unwrap();
        assert_eq!(edit.harnesses[edit.harness - 1], "codex");
    }

    #[test]
    fn settings_agent_choice_saves_and_reloads() {
        let (mut a, _, _) = connected_app();
        let dir =
            std::env::temp_dir().join(format!("ontrack-agent-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        a.settings_path = Some(path.clone());
        press(&mut a, KeyCode::Char('s'));
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Right);
        assert_eq!(a.settings.default_agent, DefaultAgent::Unassigned);
        assert_eq!(
            settings::load_from(&path).unwrap().default_agent,
            DefaultAgent::Unassigned
        );
        press(&mut a, KeyCode::Right);
        assert_eq!(
            a.settings.default_agent,
            DefaultAgent::Kind(beads::HARNESSES[0])
        );
        let _ = std::fs::remove_dir_all(dir);
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
    fn repeated_refresh_requests_coalesce_into_one_trailing_snapshot() {
        let (mut a, rx, tx) = connected_app();
        let now = Instant::now();
        a.refresh_at(now);
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        for _ in 0..20 {
            a.refresh();
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
        assert_eq!(layout.height, 20);
        assert_eq!(
            layout
                .groups
                .iter()
                .map(|g| (&*g.status, g.y, g.height, g.collapsed))
                .collect::<Vec<_>>(),
            [
                ("open", 0, 4, false),
                ("ready", 4, 2, false),
                ("blocked", 6, 2, true),
                ("deferred", 8, 2, false),
                ("in_progress", 10, 2, false),
                ("unknown", 12, 4, false),
                ("closed", 16, 4, false)
            ]
        );
        assert_eq!(
            layout
                .groups
                .iter()
                .map(|g| g.indices.clone())
                .collect::<Vec<_>>(),
            [vec![2], vec![], vec![1], vec![], vec![], vec![3], vec![0]]
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
                    row: DisplayRow::Group("ready".into()),
                    y: 4,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Group("blocked".into()),
                    y: 6,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Group("deferred".into()),
                    y: 8,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Group("in_progress".into()),
                    y: 10,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Group("unknown".into()),
                    y: 12,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Bead(3),
                    y: 13,
                    height: 2
                },
                ListTarget {
                    row: DisplayRow::Group("closed".into()),
                    y: 16,
                    height: 1
                },
                ListTarget {
                    row: DisplayRow::Bead(0),
                    y: 17,
                    height: 2
                },
            ]
        );
        let empty = list_layout(&[], &HashSet::new());
        assert_eq!(empty.groups.len(), 6);
        assert_eq!(empty.height, 12);
        assert!(empty.groups.iter().all(|group| group.indices.is_empty()));
        assert!(
            empty
                .targets
                .iter()
                .all(|target| matches!(target.row, DisplayRow::Group(_)))
        );
        a.beads[1].status = "unknown".into();
        assert_eq!(
            list_layout(&a.beads, &HashSet::new()).groups[5].indices,
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
        assert_eq!(a.theme, Theme::Light, "Tab only moves focus");
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
        for _ in 0..7 {
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
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 7, .. })));
        press(&mut a, KeyCode::Down);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 0, .. })));
        press(&mut a, KeyCode::Down);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 1, .. })));
        press(&mut a, KeyCode::Down);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 2, .. })));
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.theme, Theme::Dark);
        assert!(a.settings.auto_merge);
        assert!(settings::load_from(&path).unwrap().auto_merge);
        press(&mut a, KeyCode::Down);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 4, .. })));
        press(&mut a, KeyCode::Enter);
        assert!(!a.settings.auto_merge_children);
        assert!(!settings::load_from(&path).unwrap().auto_merge_children);
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
    fn settings_agent_limit_changes_save_and_survive_reopening() {
        let dir = std::env::temp_dir().join(format!("ontrack-agent-limit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        let mut a = app();
        a.settings_path = Some(path.clone());
        press(&mut a, KeyCode::Char('s'));
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Tab);
        assert!(matches!(a.modal, Some(Modal::Settings { focus: 2, .. })));
        for expected in [4, 3, 2, 1] {
            press(&mut a, KeyCode::Left);
            assert_eq!(
                a.settings.agent_limit,
                settings::AgentLimit::Limited(expected)
            );
            assert_eq!(
                settings::load_from(&path).unwrap().agent_limit,
                a.settings.agent_limit
            );
        }
        press(&mut a, KeyCode::Left);
        assert_eq!(a.settings.agent_limit, settings::AgentLimit::Unlimited);
        press(&mut a, KeyCode::Enter);
        assert!(matches!(
            a.modal,
            Some(Modal::Settings {
                dropdown_open: true,
                ..
            })
        ));
        press(&mut a, KeyCode::Right);
        assert_eq!(a.settings.agent_limit, settings::AgentLimit::Limited(1));
        press(&mut a, KeyCode::Enter);
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none());
        press(&mut a, KeyCode::Char('s'));
        assert!(
            matches!(a.modal, Some(Modal::Settings { draft, .. }) if draft.agent_limit == settings::AgentLimit::Limited(1))
        );
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
        assert_eq!(a.collapse_context.as_deref(), Some("closed"));
        assert!(a.selected.is_none());
        a.beads.clear();
        a.step(1);
        assert_eq!(a.selected, None);
        assert_eq!(a.collapse_context.as_deref(), Some("closed"));
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
            [
                "open",
                "ready",
                "blocked",
                "deferred",
                "in_progress",
                "unknown",
                "closed"
            ]
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
        for _ in 0..5 {
            press(&mut a, KeyCode::Down);
        }
        assert_bead(&a, "b1");
        press(&mut a, KeyCode::Char(' '));
        assert_eq!(a.collapsed.len(), 2);
        assert_collapsed_header_selected(&a, "closed");
        for _ in 0..5 {
            press(&mut a, KeyCode::Up);
        }
        assert_collapsed_header_selected(&a, "open");
        press(&mut a, KeyCode::Char(' '));
        assert_bead(&a, "b0");
        press(&mut a, KeyCode::Down);
        assert_bead(&a, "b3");
        for _ in 0..5 {
            press(&mut a, KeyCode::Down);
        }
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
        assert_eq!(a.collapse_context.as_deref(), Some("ready"));
        press(&mut a, KeyCode::Down);
        assert_bead(&a, "b2");
        press(&mut a, KeyCode::Up);
        assert_eq!(a.collapse_context.as_deref(), Some("ready"));
        press(&mut a, KeyCode::Up);
        assert_bead(&a, "b1");
        for _ in 0..5 {
            press(&mut a, KeyCode::Down);
        }
        assert_bead(&a, "b3");
        press(&mut a, KeyCode::Down);
        assert_bead(&a, "b3");
        press(&mut a, KeyCode::Up);
        assert_eq!(a.collapse_context.as_deref(), Some("in_progress"));
        press(&mut a, KeyCode::Up);
        assert_eq!(a.collapse_context.as_deref(), Some("deferred"));
        press(&mut a, KeyCode::Up);
        assert_bead(&a, "b2");
        for _ in 0..3 {
            press(&mut a, KeyCode::Up);
        }
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
        assert_eq!(a.collapse_context.as_deref(), Some("ready"));
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
        assert_eq!(a.collapse_context.as_deref(), Some("ready"));
        press(&mut a, KeyCode::Down);
        assert_collapsed_header_selected(&a, "blocked");
        for _ in 0..3 {
            press(&mut a, KeyCode::Down);
        }
        assert_collapsed_header_selected(&a, "closed");
        press(&mut a, KeyCode::Down);
        assert_collapsed_header_selected(&a, "closed");
        for _ in 0..3 {
            press(&mut a, KeyCode::Up);
        }
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
        for (x, y) in [(0, 0), (0, 3), (29, 3), (4, 9), (4, 11), (4, 13), (4, 16)] {
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
        assert_eq!(layout.groups.len(), 6);
        assert!(!layout.groups[0].collapsed);
        assert!(layout.groups[5].collapsed);
        assert_eq!(layout.groups[5].indices, vec![1]);
        assert!(
            !layout
                .targets
                .iter()
                .any(|target| target.row == DisplayRow::Bead(1))
        );

        for _ in 0..5 {
            press(&mut a, KeyCode::Down);
        }
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
    fn closed_group_defaults_on_first_empty_list_and_remains_collapsed() {
        let mut a = unloaded_app();
        respond(&mut a, Response::List(Ok(vec![])));
        assert!(!a.closed_default_pending);
        assert!(a.collapsed.contains("closed"));

        let mut items = app().beads;
        items.truncate(2);
        respond(&mut a, Response::List(Ok(items.clone())));
        assert!(!a.closed_default_pending);
        assert!(a.collapsed.contains("closed"));
        assert_bead(&a, "b0");

        items[1].status = "closed".into();
        respond(&mut a, Response::List(Ok(items)));
        assert!(!a.closed_default_pending);
        assert!(a.collapsed.contains("closed"));
        assert_bead(&a, "b0");
    }
    #[test]
    fn empty_group_headers_remain_keyboard_and_mouse_targets_without_issue_actions() {
        let mut a = unloaded_app();
        respond(&mut a, Response::List(Ok(vec![])));
        let layout = list_layout(&a.beads, &a.collapsed);
        assert_eq!(layout.groups.len(), 6);
        assert!(layout.groups.iter().all(|group| group.indices.is_empty()));

        press(&mut a, KeyCode::Down);
        assert_eq!(a.collapse_context.as_deref(), Some("open"));
        press(&mut a, KeyCode::Char(' '));
        assert!(a.collapsed.contains("open"));
        press(&mut a, KeyCode::Char(' '));
        assert!(!a.collapsed.contains("open"));
        assert_eq!(a.collapse_context.as_deref(), Some("open"));
        press(&mut a, KeyCode::Char('d'));
        assert!(a.modal.is_none());
        assert!(a.selected.is_none());
        assert!(a.detail.is_none());

        press(&mut a, KeyCode::Down);
        assert_eq!(a.collapse_context.as_deref(), Some("ready"));
        respond(&mut a, Response::List(Ok(vec![])));
        assert_eq!(a.collapse_context.as_deref(), Some("ready"));
        a.list_rect = Rect::new(0, 0, 30, 16);
        a.mouse(MouseEventKind::Down(MouseButton::Left), 4, 2);
        assert!(a.collapsed.contains("ready"));
        assert_eq!(a.collapse_context.as_deref(), Some("ready"));
        assert!(a.selected.is_none());
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
        for _ in 0..6 {
            press(&mut a, KeyCode::Down);
        }
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
        assert_eq!(
            a.collapsed,
            HashSet::from(["open".to_owned(), "closed".to_owned()])
        );
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
    fn existing_epic_generation_requires_readable_confirmation_and_prevents_repeats() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        a.selected = None;
        press(&mut a, KeyCode::Char('g'));
        assert!(a.modal.is_none());
        a.selected = Some(0);
        a.beads[0].data["issue_type"] = json!("task");
        press(&mut a, KeyCode::Char('g'));
        assert!(a.modal.is_none());
        assert!(rx.try_recv().is_err());
        a.beads[0].data["issue_type"] = json!("epic");
        for cancel in [
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Char('n'),
            KeyCode::Char('N'),
        ] {
            press(&mut a, KeyCode::Char('g'));
            assert!(matches!(&a.modal, Some(Modal::ConfirmGenerateEpic { id, .. }) if id == "b0"));
            a.modal_ready = false;
            a.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
            assert!(rx.try_recv().is_err()); // No render yet.
            a.modal_ready = true;
            press(&mut a, cancel);
            assert!(a.modal.is_none());
            assert!(rx.try_recv().is_err());
        }
        a.focus_detail = true;
        press(&mut a, KeyCode::Char('g'));
        a.modal_ready = true;
        // The confirmation remains bound to the original Epic if selection changes.
        a.selected = Some(1);
        press(&mut a, KeyCode::Char('y'));
        assert!(matches!(rx.try_recv(), Ok(Request::GenerateEpic(id)) if id == "b0"));
        assert!(a.modal.is_none());
        assert!(a.planning_epics.contains("b0"));
        assert!(!a.mutation_busy);
        a.selected = Some(0);
        press(&mut a, KeyCode::Char('g'));
        assert!(a.modal.is_none());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn epic_planning_failure_completion_and_queue_errors_allow_recovery() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        let (out, responses) = mpsc::channel();
        a.responses = responses;
        a.planning_epics.insert("b0".into());
        out.send(Response::EpicLaunched(
            "b0".into(),
            Err(std::io::Error::other("proposal is empty")),
        ))
        .unwrap();
        a.receive();
        assert!(!a.planning_epics.contains("b0"));
        assert!(a.notice.as_deref().unwrap().contains("proposal is empty"));
        assert!(!a.notice.as_deref().unwrap().contains("created"));
        out.send(Response::EpicLaunched(
            "b0".into(),
            Ok(crate::epic::Session {
                workspace_id: "w1".into(),
                tab_id: "w1:t2".into(),
                pane_id: "w1:p1".into(),
            }),
        ))
        .unwrap();
        a.receive();
        assert!(a.planning_epics.contains("b0"));
        assert!(a.notice.as_deref().unwrap().contains("tab w1:t2"));
        out.send(Response::EpicFinished("b0".into(), Ok(vec!["b0.1".into()])))
            .unwrap();
        a.receive();
        assert!(!a.planning_epics.contains("b0"));
        assert!(a.notice.as_deref().unwrap().contains("1 child Beads"));
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
        a.planning_epics.insert("b0".into());
        out.send(Response::EpicFinished(
            "b0".into(),
            Err(std::io::Error::other("agent blocked")),
        ))
        .unwrap();
        a.receive();
        assert!(!a.planning_epics.contains("b0"));
        assert!(a.notice.as_deref().unwrap().contains("agent blocked"));
        drop(rx);
        a.selected = Some(0);
        a.beads[0].data["issue_type"] = json!("epic");
        press(&mut a, KeyCode::Char('g'));
        a.modal_ready = true;
        press(&mut a, KeyCode::Char('y'));
        assert!(!a.planning_epics.contains("b0"));
        assert!(a.modal.is_some());
        assert!(a.notice.as_deref().unwrap().contains("could not be queued"));
    }

    #[test]
    fn epic_toggle_only_dispatches_opted_in_epics() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        press(&mut a, KeyCode::Char('n'));
        let seq = take_harness_discovery(&rx);
        supply_harnesses(&mut a, seq, &["codex"]);
        let Some(Modal::Create(form)) = a.modal.as_mut() else {
            panic!()
        };
        form.title = "Plan launch".into();
        form.description = "Large proposal".into();
        form.issue_type = beads::BEAD_TYPES
            .iter()
            .position(|value| *value == "epic")
            .unwrap();
        form.focus = 7;
        press(&mut a, KeyCode::Enter);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert!(form.generate_children);
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Enter);
        assert!(
            matches!(rx.try_recv(), Ok(Request::CreateEpic(input, None)) if input.issue_type == "epic" && input.description == "Large proposal")
        );

        let mut b = app();
        let (tx, rx) = mpsc::channel();
        b.requests = tx;
        press(&mut b, KeyCode::Char('n'));
        let seq = take_harness_discovery(&rx);
        supply_harnesses(&mut b, seq, &["codex"]);
        let Some(Modal::Create(form)) = b.modal.as_mut() else {
            panic!()
        };
        form.title = "Ordinary".into();
        form.issue_type = beads::BEAD_TYPES
            .iter()
            .position(|value| *value == "epic")
            .unwrap();
        form.generate_children = true;
        form.focus = 2;
        press(&mut b, KeyCode::Left);
        let Some(Modal::Create(form)) = b.modal.as_mut() else {
            panic!()
        };
        assert!(!form.generate_children);
        form.focus = 4;
        press(&mut b, KeyCode::Enter);
        assert!(
            matches!(rx.try_recv(), Ok(Request::Create(input)) if input.issue_type == "feature" && input.harness.as_deref() == Some("codex"))
        );

        let mut empty = app();
        let (tx, rx) = mpsc::channel();
        empty.requests = tx;
        press(&mut empty, KeyCode::Char('n'));
        let seq = take_harness_discovery(&rx);
        supply_harnesses(&mut empty, seq, &["codex"]);
        let Some(Modal::Create(form)) = empty.modal.as_mut() else {
            panic!()
        };
        form.title = "Needs proposal".into();
        form.issue_type = beads::BEAD_TYPES
            .iter()
            .position(|value| *value == "epic")
            .unwrap();
        form.generate_children = true;
        form.focus = 4;
        press(&mut empty, KeyCode::Enter);
        assert!(rx.try_recv().is_err());
        assert!(empty.notice.as_deref().unwrap().contains("proposal"));
    }
    #[test]
    fn epic_form_keeps_harness_and_generation_controls_separate() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        let Some(Modal::Create(form)) = a.modal.as_mut() else {
            panic!()
        };
        form.issue_type = beads::BEAD_TYPES
            .iter()
            .position(|value| *value == "epic")
            .unwrap();
        form.focus = 3;
        press(&mut a, KeyCode::Tab);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.focus, 6);
        press(&mut a, KeyCode::Tab);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.focus, 1);
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Enter);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.focus, 7);
        assert!(form.generate_children);
        assert!(form.dropdown.is_none());
        press(&mut a, KeyCode::Tab);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.focus, 4);
        press(&mut a, KeyCode::BackTab);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.focus, 7);
    }
    #[test]
    fn description_accepts_shortcut_letters_and_shift_enter_but_not_control_chars() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        press(&mut a, KeyCode::Tab);
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
    fn screenshot_paste_inserts_reference_at_description_caret() {
        let mut form = CreateForm {
            description: "before after".into(),
            description_cursor: 7,
            focus: 1,
            ..Default::default()
        };
        form.insert_screenshot(std::path::Path::new("/shared/.beads/screenshots/test.png"));
        assert_eq!(
            form.description,
            "before ![Screenshot](</shared/.beads/screenshots/test.png>)after"
        );
        assert_eq!(form.description_cursor, form.description.len() - 5);
    }

    #[test]
    fn failed_screenshot_paste_keeps_description() {
        let mut a = app();
        a.modal = Some(Modal::Create(CreateForm {
            description: "keep this".into(),
            description_cursor: 4,
            focus: 1,
            ..Default::default()
        }));
        a.paste_seq = 1;
        a.paste_pending = true;
        let (tx, rx) = mpsc::channel();
        a.responses = rx;
        tx.send(Response::ScreenshotRead(
            1,
            Err(std::io::Error::other("no image")),
        ))
        .unwrap();
        a.receive();
        let Some(Modal::Create(form)) = a.modal else {
            panic!("create form was closed")
        };
        assert_eq!(form.description, "keep this");
        assert!(!a.paste_pending);
        assert!(a.notice.as_deref().unwrap().contains("no image"));
    }

    #[test]
    fn control_v_requests_image_only_in_description_for_create_and_edit() {
        for editing in [false, true] {
            let mut a = app();
            let mut form = CreateForm {
                focus: 1,
                ..Default::default()
            };
            form.description = "draft".into();
            a.modal = Some(if editing {
                Modal::Edit {
                    id: "b0".into(),
                    form,
                }
            } else {
                Modal::Create(form)
            });
            let (tx, rx) = mpsc::channel();
            a.requests = tx;
            a.modal_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL));
            assert!(matches!(rx.try_recv(), Ok(Request::ReadScreenshot(1))));
            assert!(a.paste_pending);
            a.modal_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
            let form = match a.modal.as_ref().unwrap() {
                Modal::Create(form) | Modal::Edit { form, .. } => form,
                _ => unreachable!(),
            };
            assert_eq!(form.description, "draft");
        }
    }

    #[test]
    fn cancel_form_removes_unsaved_screenshot() {
        let path =
            std::env::temp_dir().join(format!("ontrack-unsaved-screenshot-{}", std::process::id()));
        std::fs::write(&path, b"image").unwrap();
        let mut a = app();
        a.modal = Some(Modal::Create(CreateForm::default()));
        a.created_screenshots.push(path.clone());
        a.modal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(a.modal.is_none());
        assert!(!path.exists());
    }
    #[test]
    fn enter_from_description_focuses_create_without_submitting() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.requests = tx;
        press(&mut a, KeyCode::Char('n'));
        let seq = take_harness_discovery(&rx);
        supply_harnesses(&mut a, seq, &["codex"]);
        for c in "Title".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        press(&mut a, KeyCode::Tab);
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
        for _ in 0..7 {
            press(&mut a, KeyCode::Tab);
            if let Some(Modal::Create(f)) = &a.modal {
                forward.push(f.focus)
            }
        }
        assert_eq!(forward, [2, 3, 6, 1, 4, 5, 0]);
        let mut reverse = Vec::new();
        for _ in 0..7 {
            press(&mut a, KeyCode::BackTab);
            if let Some(Modal::Create(f)) = &a.modal {
                reverse.push(f.focus)
            }
        }
        assert_eq!(reverse, [5, 4, 1, 6, 3, 2, 0]);
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
    fn help_tracks_active_view_and_preserves_underlying_form() {
        let mut a = app();
        let help_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::CONTROL);
        a.key(help_key);
        assert_eq!(a.help.unwrap().context, hotkeys::Context::Issues);
        press(&mut a, KeyCode::Down);
        assert_eq!(a.selected, Some(0));
        press(&mut a, KeyCode::Esc);
        assert!(a.help.is_none());

        press(&mut a, KeyCode::Tab);
        a.key(help_key);
        assert_eq!(a.help.unwrap().context, hotkeys::Context::Details);
        a.key(help_key);
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Char('n'));
        press(&mut a, KeyCode::Char('?'));
        a.key(help_key);
        assert_eq!(a.help.unwrap().context, hotkeys::Context::Form);
        press(&mut a, KeyCode::Esc);
        assert!(matches!(a.modal, Some(Modal::Create(ref form)) if form.title == "?"));
        a.mutation_busy = true;
        a.key(help_key);
        assert!(a.help.is_none());
    }

    #[test]
    fn help_lists_view_specific_commands() {
        let mut a = app();
        a.worktrees_view = true;
        a.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL));
        let context = a.help.unwrap().context;
        assert_eq!(context, hotkeys::Context::Worktrees);
        assert!(context.entries().iter().any(|(key, _)| *key == "Shift+R"));
        assert!(!context.entries().iter().any(|(key, _)| *key == "n"));
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
        let seq = take_harness_discovery(&rx);
        supply_harnesses(&mut a, seq, &["codex"]);
        for c in "q/rn界".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        for _ in 0..5 {
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
        take_harness_discovery(&rx);
        for _ in 0..5 {
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
        let s = "界a\nxyz".to_string();
        let mut cursor = "界".len();
        assert_eq!(previous_boundary(&s, cursor), 0);
        cursor = next_boundary(&s, 0);
        assert_eq!(cursor, 3);
        let down = crate::ui::move_description_vertical(&s, 3, true, &mut None);
        assert_eq!(&s[down..down + 1], "z");
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
        assert_eq!(form.title, "a\nxyz");
    }
    #[test]
    fn description_arrows_follow_soft_wrap_in_create_and_edit_forms() {
        let description = format!("{} {}", "a".repeat(60), "b".repeat(20));
        let first_line_cursor = 50;
        for editing in [false, true] {
            let mut a = app();
            let form = CreateForm {
                description: description.clone(),
                description_cursor: first_line_cursor,
                focus: 1,
                ..Default::default()
            };
            a.modal = Some(if editing {
                Modal::Edit {
                    id: "b0".into(),
                    form,
                }
            } else {
                Modal::Create(form)
            });
            press(&mut a, KeyCode::Up);
            let Some(Modal::Create(form) | Modal::Edit { form, .. }) = a.modal.as_ref() else {
                panic!("form closed")
            };
            assert_eq!(form.description_cursor, first_line_cursor);
            press(&mut a, KeyCode::Down);
            let Some(Modal::Create(form) | Modal::Edit { form, .. }) = a.modal.as_ref() else {
                panic!("form closed")
            };
            assert_eq!(form.description_cursor, description.len());
            assert_eq!(form.description, description);
            press(&mut a, KeyCode::Down);
            press(&mut a, KeyCode::Up);
            let Some(Modal::Create(form) | Modal::Edit { form, .. }) = a.modal.as_ref() else {
                panic!("form closed")
            };
            assert_eq!(form.description_cursor, first_line_cursor);
            assert_eq!(form.description, description);
        }
    }
    #[test]
    fn description_arrows_restore_column_across_short_explicit_line() {
        let description = format!("{}\nhi\n{}", "a".repeat(55), "界".repeat(28));
        let mut a = app();
        a.modal = Some(Modal::Create(CreateForm {
            description: description.clone(),
            description_cursor: 50,
            focus: 1,
            ..Default::default()
        }));
        press(&mut a, KeyCode::Down);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.description_cursor, 58);
        press(&mut a, KeyCode::Down);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.description_cursor, 59 + 25 * "界".len());
        assert!(form.description.is_char_boundary(form.description_cursor));
        press(&mut a, KeyCode::Up);
        press(&mut a, KeyCode::Up);
        press(&mut a, KeyCode::Up);
        let Some(Modal::Create(form)) = a.modal.as_ref() else {
            panic!()
        };
        assert_eq!(form.description_cursor, 50);
        assert_eq!(form.description, description);
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
    fn edit_loads_live_record_and_update_failure_keeps_draft_for_retry() {
        let mut a = app();
        let (requests, rx) = mpsc::channel();
        let (out, responses) = mpsc::channel();
        a.requests = requests;
        a.responses = responses;
        press(&mut a, KeyCode::Char('e'));
        let seq = match rx.try_recv() {
            Ok(Request::LoadEdit(id, seq)) if id == "b0" => seq,
            _ => panic!("missing edit load"),
        };
        assert!(matches!(a.modal, Some(Modal::EditLoading { .. })));
        out.send(Response::EditLoaded(
            "b0".into(),
            seq,
            Ok(json!({
                "id": "b0", "title": "Live title", "description": "Live\ntext",
                "issue_type": "bug", "priority": 3, "notes": "untouched"
            })),
        ))
        .unwrap();
        a.receive();
        let Some(Modal::Edit { id, form }) = a.modal.as_mut() else {
            panic!("edit form not opened")
        };
        assert_eq!(id, "b0");
        assert_eq!(
            (
                &*form.title,
                &*form.description,
                form.issue_type,
                form.priority
            ),
            ("Live title", "Live\ntext", 1, 3)
        );
        form.title = "Changed".into();
        form.focus = 4;
        press(&mut a, KeyCode::Enter);
        let Ok(Request::Update(input)) = rx.try_recv() else {
            panic!("missing update request")
        };
        assert_eq!(input.id, "b0");
        assert_eq!(input.fields.title, "Changed");
        assert_eq!(input.fields.description, "Live\ntext");
        assert_eq!(input.fields.issue_type, "bug");
        assert_eq!(input.fields.priority, 3);
        press(&mut a, KeyCode::Enter);
        assert!(rx.try_recv().is_err());
        out.send(Response::Updated(
            "b0".into(),
            Err(std::io::Error::other("offline")),
        ))
        .unwrap();
        a.receive();
        assert!(!a.mutation_busy);
        assert!(matches!(&a.modal, Some(Modal::Edit { form, .. }) if form.title == "Changed"));
        assert!(a.notice.as_deref().unwrap().contains("offline"));
        press(&mut a, KeyCode::Enter);
        assert!(matches!(rx.try_recv(), Ok(Request::Update(_))));
        out.send(Response::Updated("b0".into(), Ok(()))).unwrap();
        a.receive();
        assert!(a.modal.is_none());
        assert_eq!(a.preferred_id.as_deref(), Some("b0"));
        assert!(matches!(rx.try_recv(), Ok(Request::List)));
    }

    #[test]
    fn edit_cancellation_and_bad_live_record_do_not_write() {
        let mut a = app();
        let (requests, rx) = mpsc::channel();
        let (out, responses) = mpsc::channel();
        a.requests = requests;
        a.responses = responses;
        press(&mut a, KeyCode::Char('e'));
        let seq = match rx.try_recv() {
            Ok(Request::LoadEdit(_, seq)) => seq,
            _ => panic!(),
        };
        out.send(Response::EditLoaded(
            "b0".into(),
            seq.wrapping_sub(1),
            Ok(json!({"id": "b0", "title": "stale", "issue_type": "task", "priority": 2})),
        ))
        .unwrap();
        a.receive();
        assert!(matches!(a.modal, Some(Modal::EditLoading { .. })));
        out.send(Response::EditLoaded(
            "b0".into(),
            seq,
            Ok(json!({
                "id": "b0", "title": "Existing", "issue_type": "custom", "priority": 2
            })),
        ))
        .unwrap();
        a.receive();
        assert!(matches!(
            &a.modal,
            Some(Modal::EditLoading { error: Some(_), .. })
        ));
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none());
        assert!(rx.try_recv().is_err());
        press(&mut a, KeyCode::Char('e'));
        let seq = match rx.try_recv() {
            Ok(Request::LoadEdit(_, seq)) => seq,
            _ => panic!(),
        };
        out.send(Response::EditLoaded(
            "b0".into(),
            seq,
            Ok(json!({
                "id": "b0", "title": "Existing", "issue_type": "task", "priority": 2
            })),
        ))
        .unwrap();
        a.receive();
        assert!(matches!(a.modal, Some(Modal::Edit { .. })));
        press(&mut a, KeyCode::Esc);
        assert!(a.modal.is_none());
        assert!(rx.try_recv().is_err());
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
    fn enter_focuses_selected_worktree_without_repeating_pending_request() {
        let (mut a, rx, tx) = connected_app();
        a.worktrees_view = true;
        a.worktrees = ["w1", "w2"]
            .into_iter()
            .map(|id| ManagedWorktree {
                id: id.into(),
                bead_id: "b1".into(),
                parent_bead_id: None,
                handoff_accepted: true,
                bead_title: None,
                bead_description: None,
                branch: "ontrack/b1".into(),
                target_branch: "main".into(),
                workspace_id: id.into(),
                status: worktrees::WorktreeStatus::Unmerged,
                detail: None,
            })
            .collect();
        a.selected_worktree = 1;
        press(&mut a, KeyCode::Enter);
        press(&mut a, KeyCode::Enter);
        assert!(matches!(rx.try_recv(), Ok(Request::FocusWorkspace(id)) if id == "w2"));
        assert!(rx.try_recv().is_err());
        tx.send(Response::WorkspaceFocused("w2".into(), Ok(())))
            .unwrap();
        a.receive_at(Instant::now());
        assert!(matches!(
            a.worktrees_focus_feedback,
            Some(WorktreeFocusFeedback::Success(ref message)) if message.contains("w2")
        ));
        assert!(!a.worktrees_focus_pending);
    }
    #[test]
    fn enter_without_open_workspace_reports_error_without_request() {
        let (mut a, rx, tx) = connected_app();
        a.worktrees_view = true;
        press(&mut a, KeyCode::Enter);
        assert!(matches!(
            a.worktrees_focus_feedback,
            Some(WorktreeFocusFeedback::Error(ref message)) if message.contains("Select")
        ));
        a.worktrees.push(ManagedWorktree {
            id: "wt-1".into(),
            bead_id: "b1".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/b1".into(),
            target_branch: "main".into(),
            workspace_id: "w1".into(),
            status: worktrees::WorktreeStatus::WorkspaceClosed,
            detail: None,
        });
        press(&mut a, KeyCode::Enter);
        assert!(matches!(
            a.worktrees_focus_feedback,
            Some(WorktreeFocusFeedback::Error(ref message)) if message.contains("closed")
        ));
        assert!(rx.try_recv().is_err());

        a.worktrees[0].status = worktrees::WorktreeStatus::CheckoutRemoved;
        press(&mut a, KeyCode::Enter);
        assert!(matches!(
            a.worktrees_focus_feedback,
            Some(WorktreeFocusFeedback::Error(ref message)) if message.contains("closed")
        ));
        assert!(rx.try_recv().is_err());

        a.worktrees[0].status = worktrees::WorktreeStatus::Integrated;
        press(&mut a, KeyCode::Enter);
        assert!(matches!(rx.try_recv(), Ok(Request::FocusWorkspace(id)) if id == "w1"));
        tx.send(Response::WorkspaceFocused(
            "w1".into(),
            Err(std::io::Error::other("workspace missing")),
        ))
        .unwrap();
        a.receive_at(Instant::now());
        assert!(matches!(
            a.worktrees_focus_feedback,
            Some(WorktreeFocusFeedback::Error(ref message)) if message.contains("workspace missing")
        ));
    }
    #[test]
    fn worktree_merge_requires_confirmation_and_cancel_has_no_side_effect() {
        let (mut a, rx, _) = connected_app();
        a.worktrees_view = true;
        a.worktrees.push(ManagedWorktree {
            id: "wt-1".into(),
            bead_id: "b1".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/b1".into(),
            target_branch: "main".into(),
            workspace_id: "ws1".into(),
            status: worktrees::WorktreeStatus::InProgress,
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
    fn restart_requires_explicit_confirmation_and_queues_selected_handoff() {
        let (mut app, requests, _) = connected_app();
        app.worktrees_view = true;
        app.worktrees.push(ManagedWorktree {
            id: "wt-1".into(),
            bead_id: "b1".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: Some("Task".into()),
            bead_description: None,
            branch: "ontrack/b1".into(),
            target_branch: "main".into(),
            workspace_id: "ws1".into(),
            status: worktrees::WorktreeStatus::NeedsAttention,
            detail: None,
        });
        press(&mut app, KeyCode::Char('R'));
        assert!(matches!(
            app.modal,
            Some(Modal::ConfirmWorktree { action: 3, .. })
        ));
        assert!(requests.try_recv().is_err());
        press(&mut app, KeyCode::Esc);
        assert!(requests.try_recv().is_err());
        press(&mut app, KeyCode::Char('R'));
        press(&mut app, KeyCode::Char('y'));
        assert!(matches!(requests.try_recv(), Ok(Request::RestartWorktree(id)) if id == "wt-1"));
        assert!(app.mutation_busy);
    }
    #[test]
    fn continue_requires_confirmation_and_queues_active_handoff() {
        let (mut app, requests, _) = connected_app();
        app.worktrees_view = true;
        app.worktrees.push(ManagedWorktree {
            id: "wt-1".into(),
            bead_id: "b1".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: Some("Task".into()),
            bead_description: None,
            branch: "ontrack/b1".into(),
            target_branch: "main".into(),
            workspace_id: "ws1".into(),
            status: worktrees::WorktreeStatus::DirtyCheckout,
            detail: None,
        });
        press(&mut app, KeyCode::Char('a'));
        assert!(matches!(
            app.modal,
            Some(Modal::ConfirmWorktree { action: 4, .. })
        ));
        assert!(requests.try_recv().is_err());
        press(&mut app, KeyCode::Char('y'));
        assert!(matches!(requests.try_recv(), Ok(Request::ContinueWorktree(id)) if id == "wt-1"));
    }

    #[test]
    fn empty_worktree_view_stays_rendered_through_refresh_and_error_retry() {
        use ratatui::{Terminal, backend::TestBackend};

        for (width, height) in [(120, 24), (64, 16)] {
            let (mut app, requests, responses) = connected_app();
            app.worktrees_view = true;
            app.worktrees_discovered = false;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut frame = |app: &mut App| {
                terminal
                    .draw(|f| crate::ui::render(f, Duration::from_secs(2), app))
                    .unwrap();
                terminal.backend().buffer().clone()
            };
            let now = Instant::now();
            app.poll_worktrees(now);
            assert!(matches!(requests.try_recv(), Ok(Request::ListWorktrees)));
            assert!(app.worktrees_loading, "initial discovery stays quiet");
            let initial = frame(&mut app);
            responses.send(Response::Worktrees(Ok(vec![]))).unwrap();
            app.receive_at(now);
            let settled = frame(&mut app);
            assert_ne!(initial, settled, "completed discovery shows guidance");

            for cycle in 1..=3 {
                let now = now + REFRESH_INTERVAL * cycle;
                app.poll_worktrees(now);
                assert!(matches!(requests.try_recv(), Ok(Request::ListWorktrees)));
                assert_eq!(frame(&mut app), settled, "background scan hides no text");
                app.poll_worktrees(now + REFRESH_INTERVAL * 10);
                assert!(requests.try_recv().is_err(), "requests remain bounded");
                responses.send(Response::Worktrees(Ok(vec![]))).unwrap();
                app.receive_at(now);
                assert_eq!(frame(&mut app), settled);
            }

            press(&mut app, KeyCode::Char('r'));
            assert!(matches!(requests.try_recv(), Ok(Request::ListWorktrees)));
            assert_eq!(frame(&mut app), settled, "manual refresh hides no text");
            responses
                .send(Response::Worktrees(Err(std::io::Error::other(
                    "registry unavailable",
                ))))
                .unwrap();
            app.receive_at(now);
            assert_eq!(app.worktrees_error.as_deref(), Some("registry unavailable"));
            let failed = frame(&mut app);
            assert_ne!(failed, settled, "scan errors render in the header");
            app.poll_worktrees(now + REFRESH_INTERVAL);
            assert!(matches!(requests.try_recv(), Ok(Request::ListWorktrees)));
            assert_eq!(frame(&mut app), failed, "retry preserves empty guidance");
            responses.send(Response::Worktrees(Ok(vec![]))).unwrap();
            app.receive_at(now);
            assert_eq!(frame(&mut app), settled, "success clears the error");

            app.poll_worktrees(now + REFRESH_INTERVAL);
            assert!(matches!(requests.try_recv(), Ok(Request::ListWorktrees)));
            responses
                .send(Response::Worktrees(Ok(vec![observed_row(
                    "new-handoff",
                    worktrees::WorktreeStatus::InProgress,
                )])))
                .unwrap();
            app.receive_at(now);
            assert_eq!(app.worktrees.len(), 1);
            assert_ne!(frame(&mut app), settled, "new work replaces empty guidance");
        }
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
    fn worktree_view_r_still_refreshes_worktrees() {
        let (mut a, rx, _) = connected_app();
        a.worktrees_view = true;
        press(&mut a, KeyCode::Char('r'));
        assert!(matches!(rx.try_recv(), Ok(Request::ListWorktrees)));
        assert!(rx.try_recv().is_err());
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
                parent_bead_id: None,
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
    fn tracked_worktree_reconciles_with_cleanup_preferences_disabled() {
        let (mut app, rx, _) = connected_app();
        app.worktrees_discovered = true;
        app.worktrees.push(ManagedWorktree {
            id: "tracked".into(),
            bead_id: "bead".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/task".into(),
            target_branch: "main".into(),
            workspace_id: "ws".into(),
            status: worktrees::WorktreeStatus::Unmerged,
            detail: None,
        });
        app.poll_worktrees(Instant::now());
        assert!(matches!(
            rx.try_recv(),
            Ok(Request::ReconcileWorktrees(CleanupPolicy {
                auto_merge: false,
                auto_merge_children: true,
                auto_close_workspace: false,
                auto_remove_checkout: false,
            }))
        ));
    }
    #[test]
    fn closed_bead_does_not_remove_its_managed_worktree() {
        let (mut a, _, tx) = connected_app();
        a.beads[0].status = "closed".into();
        let item = ManagedWorktree {
            id: "tracked-closed".into(),
            bead_id: a.beads[0].id.clone(),
            parent_bead_id: None,
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
    fn removed_checkout_is_hidden_on_refresh_without_hiding_attention_or_closed_workspace() {
        let (mut app, _, tx) = connected_app();
        let item = |id: &str, status| ManagedWorktree {
            id: id.into(),
            bead_id: format!("bead-{id}"),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: format!("ontrack/{id}"),
            target_branch: "main".into(),
            workspace_id: format!("ws-{id}"),
            status,
            detail: None,
        };
        let removed = item("removed", worktrees::WorktreeStatus::CheckoutRemoved);
        let closed = item("closed", worktrees::WorktreeStatus::WorkspaceClosed);
        let attention = item("attention", worktrees::WorktreeStatus::NeedsAttention);
        app.worktrees = vec![removed.clone(), closed.clone()];
        app.selected_worktree = 1;
        app.worktree_detail_offset = 7;
        tx.send(Response::Worktrees(Ok(vec![
            removed.clone(),
            closed.clone(),
            attention.clone(),
        ])))
        .unwrap();
        app.receive();
        assert_eq!(
            app.worktrees
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            vec!["closed", "attention"]
        );
        assert_eq!(app.selected_worktree, 0);
        assert_eq!(app.worktree_detail_offset, 7);
        assert!(app.accepted_handoffs.contains(&removed.bead_id));

        tx.send(Response::Worktrees(Ok(vec![
            removed.clone(),
            attention.clone(),
        ])))
        .unwrap();
        app.receive();
        assert_eq!(app.worktrees[0].id, attention.id);
        assert_eq!(app.selected_worktree, 0);
        assert_eq!(app.worktree_detail_offset, 0);

        tx.send(Response::Worktrees(Ok(vec![removed]))).unwrap();
        app.receive();
        assert!(app.worktrees.is_empty());
        assert_eq!(app.selected_worktree, 0);
    }
    #[test]
    fn successful_checkout_removal_hides_selected_row_immediately() {
        let (mut app, _, tx) = connected_app();
        let item = |id: &str| ManagedWorktree {
            id: id.into(),
            bead_id: format!("bead-{id}"),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: format!("ontrack/{id}"),
            target_branch: "main".into(),
            workspace_id: format!("ws-{id}"),
            status: worktrees::WorktreeStatus::Integrated,
            detail: None,
        };
        let first = item("first");
        let selected = item("selected");
        let last = item("last");
        app.worktrees = vec![first, selected.clone(), last.clone()];
        app.selected_worktree = 1;
        app.worktree_detail_offset = 9;
        tx.send(Response::WorktreeChanged(Ok(ManagedWorktree {
            status: worktrees::WorktreeStatus::CheckoutRemoved,
            ..selected
        })))
        .unwrap();
        app.receive();
        assert_eq!(app.worktrees.len(), 2);
        assert_eq!(app.worktrees[app.selected_worktree].id, last.id);
        assert_eq!(app.worktree_detail_offset, 0);
        assert!(app.accepted_handoffs.contains("bead-selected"));
        assert!(app.notice.as_deref().unwrap().contains("CHECKOUT REMOVED"));
    }
    #[test]
    fn older_worktree_snapshot_cannot_overwrite_action_result() {
        let (mut a, _, tx) = connected_app();
        let before = ManagedWorktree {
            id: "wt".into(),
            bead_id: "b0".into(),
            parent_bead_id: None,
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
