use crate::{app::App, beads::Bead};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use ratatui_splash_screen::{SplashConfig, SplashScreen};
use std::time::Duration;
const BG: Color = Color::Rgb(13, 23, 28);
const INK: Color = Color::Rgb(220, 228, 229);
const MUTED: Color = Color::Rgb(123, 144, 148);
const TEAL: Color = Color::Rgb(73, 211, 182);
const GOLD: Color = Color::Rgb(239, 183, 99);
pub fn new_splash() -> Option<SplashScreen> {
    SplashScreen::new(SplashConfig {
        image_data: include_bytes!("../assets/splash.png"),
        sha256sum: None,
        render_steps: 12,
        use_colors: true,
    })
    .ok()
}
fn item(b: &Bead, selected: bool) -> ListItem<'static> {
    let status = if b.status.is_empty() {
        "unknown"
    } else {
        &b.status
    };
    let lead = if selected { "› " } else { "  " };
    ListItem::new(vec![
        Line::from(vec![
            Span::styled(lead, Style::default().fg(TEAL)),
            Span::styled(
                format!("{}  P{}  ·  {}", b.id, b.priority, status),
                Style::default().fg(GOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(b.title.clone(), Style::default().fg(INK)),
        ]),
    ])
}
pub fn render(f: &mut Frame, elapsed: Duration, splash: &mut Option<SplashScreen>, app: &mut App) {
    let a = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG)), a);
    app.list_rect = Rect::default();
    app.detail_rect = Rect::default();
    if a.width < 24 || a.height < 7 {
        f.render_widget(
            Paragraph::new("ontrack · expand terminal")
                .style(Style::default().fg(MUTED))
                .alignment(Alignment::Center),
            a,
        );
        return;
    }
    if elapsed < Duration::from_millis(700)
        && let Some(s) = splash.as_mut().filter(|s| !s.is_rendered())
    {
        f.render_widget(
            s,
            Rect::new(
                a.x + a.width.saturating_sub(26) / 2,
                a.y + a.height.saturating_sub(12) / 2,
                26.min(a.width),
                12.min(a.height),
            ),
        );
        return;
    }
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(a);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " BEADS ",
                Style::default()
                    .fg(BG)
                    .bg(TEAL)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  ONTRACK  /  WORKSPACE ISSUES"),
        ]))
        .style(Style::default().fg(INK)),
        v[0],
    );
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(v[1]);
    app.list_rect = split[0];
    app.detail_rect = split[1];
    let visible = (split[0].height.saturating_sub(2) / 2) as usize;
    app.list_offset = app.list_offset.min(app.beads.len().saturating_sub(visible));
    if let Some(i) = app.selected {
        if i < app.list_offset {
            app.list_offset = i
        }
        if visible > 0 && i >= app.list_offset + visible {
            app.list_offset = i + 1 - visible
        }
    }
    let items = app
        .beads
        .iter()
        .map(|b| {
            item(
                b,
                app.selected
                    .is_some_and(|i| app.beads.get(i).is_some_and(|s| s.id == b.id)),
            )
        })
        .collect::<Vec<_>>();
    let lb = Block::default()
        .title(format!(" ISSUES · {} ", app.beads.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if !app.focus_detail { TEAL } else { MUTED }));
    let list = List::new(items)
        .block(lb)
        .highlight_style(Style::default().bg(Color::Rgb(31, 54, 58)));
    let mut state = ListState::default()
        .with_selected(app.selected)
        .with_offset(app.list_offset);
    f.render_stateful_widget(list, split[0], &mut state);
    app.list_offset = state.offset();
    let selected = app.selected.and_then(|i| app.beads.get(i));
    let detail = app.detail.as_ref().or_else(|| selected.map(|b| &b.data));
    let detail_text = detail
        .map(|v| serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()))
        .unwrap_or_else(|| {
            if app.beads.is_empty() {
                "No issues found in this workspace.\n\nPress r to refresh.".into()
            } else {
                "Select an issue to inspect its full record.".into()
            }
        });
    let inner_width = split[1].width.saturating_sub(2).max(1);
    let lines = Paragraph::new(detail_text.as_str())
        .wrap(Wrap { trim: false })
        .line_count(inner_width);
    let viewport = split[1].height.saturating_sub(2);
    app.detail_offset = app.detail_offset.min(
        lines
            .saturating_sub(viewport as usize)
            .min(u16::MAX as usize) as u16,
    );
    let title = selected
        .map(|b| format!(" {} · {} ", b.id, b.status))
        .unwrap_or_else(|| " ISSUE DETAILS ".into());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus_detail { TEAL } else { MUTED }));
    f.render_widget(
        Paragraph::new(detail_text)
            .block(block)
            .style(Style::default().fg(INK))
            .wrap(Wrap { trim: false })
            .scroll((app.detail_offset, 0)),
        split[1],
    );
    let state = if let Some(e) = &app.list_error {
        format!("LIST ERROR · {e}")
    } else if let Some(e) = &app.detail_error {
        format!("DETAIL ERROR · {e}")
    } else if app.list_loading {
        "REFRESHING ISSUES…".into()
    } else if app.detail_loading {
        "LOADING DETAILS…".into()
    } else {
        "READY".into()
    };
    f.render_widget(
        Paragraph::new(format!(
            " {state}  ·  ↑↓ select  ·  Tab/←→ focus  ·  PgUp/PgDn scroll  ·  r refresh  ·  q quit"
        ))
        .style(Style::default().fg(MUTED)),
        v[2],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beads::Bead;
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;
    use std::sync::mpsc;
    fn app() -> App {
        let (tx, _) = mpsc::channel();
        let (_, responses) = mpsc::channel();
        App {
            beads: vec![Bead {
                id: "demo-1".into(),
                title: "Title is visible".into(),
                status: "closed".into(),
                priority: 1,
                data: json!({"unknown":{"n":5},"comments":[{"body":"hello comment"}],"unicode":"界界界界界界界界界界"}),
            }],
            selected: Some(0),
            detail: None,
            focus_detail: false,
            list_offset: 0,
            detail_offset: 0,
            list_loading: false,
            detail_loading: false,
            list_error: None,
            detail_error: None,
            detail_seq: 0,
            list_rect: Rect::default(),
            detail_rect: Rect::default(),
            requests: tx,
            responses,
        }
    }
    fn draw(size: (u16, u16), a: &mut App) -> (String, Rect) {
        let mut t = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
        let mut s = None;
        t.draw(|f| render(f, Duration::from_secs(1), &mut s, a))
            .unwrap();
        (
            t.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect(),
            a.list_rect,
        )
    }
    #[test]
    fn renders_exact_two_line_issue_with_status_and_full_nested_data() {
        let mut a = app();
        let (s, _) = draw((120, 40), &mut a);
        assert!(s.contains("demo-1  P1  ·  closed"));
        assert!(s.contains("Title is visible"));
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let mut splash = None;
        terminal
            .draw(|f| render(f, Duration::from_secs(1), &mut splash, &mut a))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let line = |y: u16| {
            (0..120)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        };
        assert!(line(3).contains("demo-1  P1  ·  closed"));
        assert!(line(4).contains("Title is visible"));
        for x in ["unknown", "comments", "hello comment", "界", "5"] {
            assert!(s.contains(x), "{x}")
        }
    }
    #[test]
    fn tiny_empty_error_splash_and_wrapped_detail_are_safe() {
        let mut a = app();
        a.list_rect = Rect::new(1, 1, 10, 10);
        assert!(!draw((18, 5), &mut a).0.is_empty());
        assert_eq!(a.list_rect, Rect::default());
        let mut splash = new_splash();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|f| render(f, Duration::ZERO, &mut splash, &mut a))
            .unwrap();
        assert_eq!(a.list_rect, Rect::default());
        a.list_loading = true;
        let (s, _) = draw((80, 24), &mut a);
        assert!(s.contains("REFRESHING"));
        a.list_loading = false;
        a.detail_error = Some("broken".into());
        let (s, _) = draw((80, 24), &mut a);
        assert!(s.contains("DETAIL ERROR"));
        a.beads.clear();
        a.selected = None;
        a.detail = None;
        a.detail_error = None;
        assert!(draw((80, 24), &mut a).0.contains("No issues"));
        a.beads.push(Bead {
            id: "long".into(),
            title: "long".into(),
            status: "open".into(),
            priority: 2,
            data: json!({"text":"界".repeat(700)}),
        });
        a.selected = Some(0);
        a.beads[0].data = json!({"text": format!("{}FINAL_SENTINEL", "word ".repeat(800))});
        a.detail = Some(a.beads[0].data.clone());
        a.detail_offset = u16::MAX;
        let (rendered, rect) = draw((50, 14), &mut a);
        assert!(a.detail_offset > 0);
        assert!(rendered.contains("FINAL_SENTINEL"));
        assert!(rect.height > 0)
    }
}
