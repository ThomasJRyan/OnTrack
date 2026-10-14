use crate::{
    Tui,
    beads::{self, Bead},
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
}
pub(super) enum Response {
    List(std::io::Result<Vec<Bead>>),
    Show(String, u64, std::io::Result<serde_json::Value>),
}
fn coalesce(
    first: Request,
    pending: impl IntoIterator<Item = Request>,
) -> (bool, Option<(String, u64)>) {
    let mut list = matches!(first, Request::List);
    let mut show = match first {
        Request::Show(id, seq) => Some((id, seq)),
        _ => None,
    };
    for request in pending {
        match request {
            Request::List => list = true,
            Request::Show(id, seq) => show = Some((id, seq)),
        }
    }
    (list, show)
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
    pub(super) requests: mpsc::Sender<Request>,
    pub(super) responses: mpsc::Receiver<Response>,
}
impl App {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Request>();
        let (out, responses) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut pending = Vec::new();
                while let Ok(next) = rx.try_recv() {
                    pending.push(next);
                }
                let (list_requested, show) = coalesce(first, pending);
                if list_requested && out.send(Response::List(beads::list_beads())).is_err() {
                    break;
                }
                if let Some((id, seq)) = show
                    && out
                        .send(Response::Show(id.clone(), seq, beads::show_bead(&id)))
                        .is_err()
                {
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
                            self.selected = id
                                .and_then(|id| self.beads.iter().position(|b| b.id == id))
                                .or_else(|| (!self.beads.is_empty()).then_some(0));
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
            }
        }
    }
    fn key(&mut self, k: KeyEvent) -> bool {
        if event::is_quit(k) {
            return true;
        }
        match k.code {
            KeyCode::Char('r') => self.refresh(),
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
    fn mouse(&mut self, k: MouseEventKind, x: u16, y: u16) {
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
    fn coalescing_preserves_list_and_latest_show() {
        assert_eq!(
            coalesce(
                Request::List,
                [Request::Show("a".into(), 1), Request::Show("b".into(), 2)]
            ),
            (true, Some(("b".into(), 2)))
        );
        assert_eq!(
            coalesce(
                Request::Show("a".into(), 1),
                [Request::List, Request::Show("b".into(), 2)]
            ),
            (true, Some(("b".into(), 2)))
        );
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
}
