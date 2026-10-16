use crate::app::{App, DisplayRow};
use crate::settings::Theme;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::collections::HashMap;
use std::time::Duration;
#[cfg(test)]
const MUTED: Color = Color::Rgb(123, 144, 148);
#[cfg(test)]
const TEAL: Color = Color::Rgb(73, 211, 182);
#[cfg(test)]
const GOLD: Color = Color::Rgb(239, 183, 99);
#[cfg(test)]
fn status_color(status: &str) -> Color {
    match status {
        "open" => TEAL,
        "ready" => Color::Rgb(130, 205, 135),
        "closed" | "blocked" => Color::Rgb(255, 123, 123),
        "in_progress" => GOLD,
        "deferred" => Color::Rgb(145, 170, 220),
        "unknown" | "" => MUTED,
        _ => Color::Rgb(125, 168, 235),
    }
}
#[cfg(test)]
fn row_type_color(kind: &str) -> Color {
    match kind.to_ascii_lowercase().as_str() {
        "task" => TEAL,
        "bug" => Color::Rgb(255, 123, 123),
        "feature" => Color::Rgb(103, 177, 255),
        "epic" => Color::Rgb(197, 145, 255),
        "chore" => Color::Rgb(168, 207, 125),
        "decision" => Color::Rgb(240, 141, 197),
        "spike" => Color::Rgb(255, 192, 115),
        "story" => Color::Rgb(119, 204, 219),
        "milestone" => Color::Rgb(145, 170, 220),
        _ => MUTED,
    }
}
#[cfg(test)]
fn row_priority_color(priority: u64) -> Color {
    match priority {
        0 => Color::Rgb(255, 105, 105),
        1 => Color::Rgb(255, 159, 105),
        2 => GOLD,
        3 => Color::Rgb(138, 197, 169),
        _ => MUTED,
    }
}
const ASCII_LOGO: [&str; 8] = [
    "    ███████               ███████████                              █████     ",
    "  ███▒▒▒▒▒███            ▒█▒▒▒███▒▒▒█                             ▒▒███      ",
    " ███     ▒▒███ ████████  ▒   ▒███  ▒  ████████   ██████    ██████  ▒███ █████",
    "▒███      ▒███▒▒███▒▒███     ▒███    ▒▒███▒▒███ ▒▒▒▒▒███  ███▒▒███ ▒███▒▒███ ",
    "▒███      ▒███ ▒███ ▒███     ▒███     ▒███ ▒▒▒   ███████ ▒███ ▒▒▒  ▒██████▒  ",
    "▒▒███     ███  ▒███ ▒███     ▒███     ▒███      ███▒▒███ ▒███  ███ ▒███▒▒███ ",
    " ▒▒▒███████▒   ████ █████    █████    █████    ▒▒████████▒▒██████  ████ █████",
    "   ▒▒▒▒▒▒▒    ▒▒▒▒ ▒▒▒▒▒    ▒▒▒▒▒    ▒▒▒▒▒      ▒▒▒▒▒▒▒▒  ▒▒▒▒▒▒  ▒▒▒▒ ▒▒▒▒▒ ",
];
const SPLASH_DURATION: Duration = Duration::from_millis(1200);
const GIRL_FRAMES: [[&str; 10]; 3] = [
    [
        "          .-\"\"\"-.          ",
        "        .'  .-.  '.        ",
        "       /  //   \\  \\       ",
        "      |  | o   o |  |      ",
        "      |  |   ^   |  |      ",
        "      |  |  \\_/  |  |      ",
        "       \\  \\_____/  /  \\o   ",
        "        '-._|_|_.-'   /    ",
        "           /|\\      /     ",
        "          /_|_\\           ",
    ],
    [
        "          .-\"\"\"-.       *  ",
        "        .'  .-.  '.     o/ ",
        "       /  //   \\  \\    /  ",
        "      |  | o   o |  |  /   ",
        "      |  |   ^   |  | /    ",
        "      |  |  \\_/  |  |/     ",
        "       \\  \\_____/  /      ",
        "        '-._|_|_.-'       ",
        "           /|\\            ",
        "          /_|_\\           ",
    ],
    [
        "          .-\"\"\"-.          ",
        "        .'  .-.  '.    *   ",
        "       /  //   \\  \\   o/  ",
        "      |  | -   - |  | /    ",
        "      |  |   ^   |  |/     ",
        "      |  |  \\_/  |  |      ",
        "       \\  \\_____/  /      ",
        "        '-._|_|_.-'       ",
        "           /|\\            ",
        "          /_|_\\           ",
    ],
];

fn girl_frame(elapsed: Duration) -> &'static [&'static str; 10] {
    // The existing redraw loop runs every 35 ms, so elapsed time is enough state.
    &GIRL_FRAMES[(elapsed.as_millis() / 300 % GIRL_FRAMES.len() as u128) as usize]
}

fn splash_lines(
    elapsed: Duration,
    area: Rect,
    teal: Color,
    gold: Color,
    muted: Color,
) -> Vec<Line<'static>> {
    let girl = girl_frame(elapsed);
    // One width for every frame keeps the character fixed in place as she moves.
    let girl_width = GIRL_FRAMES
        .iter()
        .flatten()
        .map(|line| Span::raw(*line).width())
        .max()
        .unwrap_or(0);
    let logo_width = ASCII_LOGO
        .iter()
        .map(|line| Span::raw(*line).width())
        .max()
        .unwrap_or(0);
    let large = usize::from(area.width) >= logo_width
        && usize::from(area.height) >= ASCII_LOGO.len() + 1 + girl.len();
    let show_girl = usize::from(area.width) >= girl_width
        && usize::from(area.height) >= (if large { ASCII_LOGO.len() } else { 2 }) + 1 + girl.len();

    let mut lines = if large {
        ASCII_LOGO
            .iter()
            .map(|line| {
                Line::from(Span::styled(
                    *line,
                    Style::default().fg(teal).add_modifier(Modifier::BOLD),
                ))
            })
            .collect::<Vec<_>>()
    } else {
        vec![
            Line::from(Span::styled(
                "ONTRACK",
                Style::default().fg(teal).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "BEADS WORKSPACE BROWSER",
                Style::default().fg(muted),
            )),
        ]
    };
    if show_girl {
        lines.push(Line::raw(""));
        lines.extend(girl.iter().enumerate().map(|(index, line)| {
            Line::from(Span::styled(
                format!("{line:<girl_width$}"),
                Style::default().fg(if index <= 2 { gold } else { teal }),
            ))
        }));
    }
    lines
}
fn row_issue_type(bead: &crate::beads::Bead) -> String {
    ["issue_type", "type"]
        .into_iter()
        .find_map(|field| {
            bead.data
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|kind| !kind.is_empty())
        })
        .map(str::to_owned)
        .unwrap_or_else(|| "unknown".to_owned())
}
fn themed_status_color(status: &str, theme: Theme) -> Color {
    crate::theme::status_color(theme, status)
}
fn themed_type_color(kind: &str, theme: Theme) -> Color {
    crate::theme::issue_type_color(theme, kind)
}
fn themed_priority_color(priority: u64, theme: Theme) -> Color {
    crate::theme::priority_color(theme, priority)
}
fn worktree_status_style(theme: Theme, status: crate::worktrees::WorktreeStatus) -> Style {
    Style::default()
        .fg(crate::theme::worktree_status_color(theme, status))
        .add_modifier(Modifier::BOLD)
}
fn settings_hint(dropdown_open: bool, focus: usize) -> &'static str {
    if dropdown_open {
        "↑ / ↓ move · ← / → choice · Enter / Space select · Tab moves · Esc close"
    } else if focus == 7 {
        "↑ / ↓ move · Enter / Space close · Tab moves · Esc close"
    } else if focus >= 3 {
        "↑ / ↓ move · Enter / Space toggle · Tab moves · Esc close"
    } else {
        "↑ / ↓ move · ← / → choice · Enter / Space open · Tab moves"
    }
}
fn settings_columns(width: usize) -> (usize, usize) {
    let title = width * 2 / 3;
    (title, width - title)
}
fn settings_row(
    label: &str,
    value: &str,
    width: usize,
    label_style: Style,
    value_style: Style,
) -> Line<'static> {
    let (title_width, value_width) = settings_columns(width);
    let title = truncate_display(label, title_width);
    let title_padding = title_width.saturating_sub(Span::raw(title.as_str()).width());
    let value = truncate_display(value, value_width);
    let value_padding = value_width.saturating_sub(Span::raw(value.as_str()).width());
    let left_padding = value_padding / 2;
    Line::from(vec![
        Span::styled(format!("{title}{}", " ".repeat(title_padding)), label_style),
        Span::styled(
            format!(
                "{}{}{}",
                " ".repeat(left_padding),
                value,
                " ".repeat(value_padding - left_padding)
            ),
            value_style,
        ),
    ])
}
const MAX_VISIBLE_THEMES: usize = 6;
fn visible_theme_window(theme: Theme, available_height: u16) -> (usize, usize) {
    let total = crate::theme::CATALOG.len();
    let visible = usize::from(available_height)
        .saturating_sub(22)
        .clamp(1, MAX_VISIBLE_THEMES.min(total));
    let selected = Theme::ALL
        .iter()
        .position(|choice| *choice == theme)
        .unwrap_or(0);
    let start = selected
        .saturating_sub(visible.saturating_sub(1))
        .min(total.saturating_sub(visible));
    (start, visible)
}
fn visible_agent_window(
    agent: crate::settings::DefaultAgent,
    available_height: u16,
) -> (usize, usize) {
    let total = crate::beads::HARNESSES.len() + 2;
    let visible = usize::from(available_height).saturating_sub(23).clamp(1, 6);
    let start = agent
        .index()
        .saturating_sub(visible.saturating_sub(1))
        .min(total - visible);
    (start, visible)
}
fn visible_limit_window(limit: crate::settings::AgentLimit, available_height: u16) -> (u64, usize) {
    let visible = usize::from(available_height)
        .saturating_sub(23)
        .clamp(1, MAX_VISIBLE_THEMES);
    let selected = match limit {
        crate::settings::AgentLimit::Unlimited => 0,
        crate::settings::AgentLimit::Limited(count) => u64::from(count),
    };
    (selected.saturating_sub((visible - 1) as u64), visible)
}
fn metadata_column_widths(beads: &[crate::beads::Bead]) -> (usize, usize) {
    let type_width = beads
        .iter()
        .map(|bead| Span::raw(row_issue_type(bead)).width())
        .max()
        .unwrap_or(0)
        .clamp(8, 18);
    let priority_width = beads
        .iter()
        .map(|bead| Span::raw(format!("P{}", bead.priority)).width())
        .max()
        .unwrap_or(2)
        .max(2);
    (type_width, priority_width)
}
fn render_header(f: &mut Frame, area: Rect, left: Line<'static>, app: &App) {
    let palette = crate::theme::definition(app.theme).palette;
    let mut left_area = area;
    if let Some(version) = &app.update_available {
        let full = format!(" UPDATE AVAILABLE · v{version} ");
        let badge = if usize::from(area.width) > left.width() + Span::raw(&full).width() {
            full.as_str()
        } else {
            " UPDATE "
        };
        let badge_width = Span::raw(badge).width() as u16;
        if area.width > badge_width + 7 {
            left_area.width = area.width - badge_width;
            f.render_widget(
                Paragraph::new(Span::styled(
                    badge.to_owned(),
                    Style::default()
                        .fg(palette.warning)
                        .add_modifier(Modifier::BOLD),
                ))
                .alignment(Alignment::Right),
                Rect::new(area.x + left_area.width, area.y, badge_width, 1),
            );
        }
    }
    f.render_widget(
        Paragraph::new(left).style(Style::default().fg(palette.foreground)),
        left_area,
    );
}
fn render_header_feedback(f: &mut Frame, area: Rect, message: Option<&str>, theme: Theme) {
    let Some(message) = message else {
        return;
    };
    let palette = crate::theme::definition(theme).palette;
    f.render_widget(
        Paragraph::new(truncate_display(
            message,
            area.width.saturating_sub(1) as usize,
        ))
        .style(Style::default().fg(palette.warning).bg(palette.background)),
        Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(1), 1),
    );
}
pub fn render(f: &mut Frame, elapsed: Duration, app: &mut App) {
    let a = f.area();
    let palette = crate::theme::definition(app.theme).palette;
    debug_assert_eq!(
        app.theme.is_light(),
        crate::theme::definition(app.theme).is_light
    );
    let bg = palette.background;
    let ink = palette.foreground;
    let muted = palette.muted;
    let teal = palette.accent;
    let gold = palette.warning;
    f.render_widget(Block::default().style(Style::default().bg(bg)), a);
    app.list_rect = Rect::default();
    app.detail_rect = Rect::default();
    if a.width < 24 || a.height < 7 {
        app.modal_ready = false;
        f.render_widget(
            Paragraph::new(if app.modal.is_some() {
                "Resize terminal"
            } else {
                "ontrack · expand terminal"
            })
            .style(Style::default().fg(muted))
            .alignment(Alignment::Center),
            a,
        );
        render_toast(f, app);
        render_help(f, app);
        return;
    }
    if app.modal.is_none() && app.help.is_none() && elapsed < SPLASH_DURATION {
        let lines = splash_lines(elapsed, a, teal, gold, muted);
        let height = lines.len() as u16;
        f.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .style(Style::default().fg(ink)),
            Rect::new(
                a.x,
                a.y + a.height.saturating_sub(height) / 2,
                a.width,
                height,
            ),
        );
        render_toast(f, app);
        return;
    }
    if app.worktrees_view {
        app.modal_ready = true;
        f.render_widget(Block::default().style(Style::default().bg(bg)), a);
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(a);
        render_header(
            f,
            areas[0],
            Line::from(vec![
                Span::styled(
                    " WORKTREES ",
                    Style::default()
                        .fg(palette.selected_foreground)
                        .bg(palette.selected_background)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  ONTRACK  /  Git integration, not Bead status"),
            ]),
            app,
        );
        let feedback = app
            .worktrees_error
            .as_deref()
            .map(|error| format!("ERROR · {error}"))
            .or_else(|| {
                app.worktrees_focus_feedback
                    .as_ref()
                    .map(|feedback| match feedback {
                        crate::app::WorktreeFocusFeedback::Success(message) => message.clone(),
                        crate::app::WorktreeFocusFeedback::Error(message) => {
                            format!("ERROR · {message}")
                        }
                    })
            })
            .or_else(|| app.notice.clone());
        render_header_feedback(f, areas[0], feedback.as_deref(), app.theme);
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(areas[1]);
        app.list_rect = panes[0];
        app.detail_rect = panes[1];
        let item_height = 2usize;
        let visible = (usize::from(panes[0].height.saturating_sub(2)) / item_height).max(1);
        if app.selected_worktree < app.worktree_offset {
            app.worktree_offset = app.selected_worktree;
        }
        if app.selected_worktree >= app.worktree_offset + visible {
            app.worktree_offset = app.selected_worktree + 1 - visible;
        }
        app.worktree_offset = app
            .worktree_offset
            .min(app.worktrees.len().saturating_sub(visible));
        let mut rows = Vec::new();
        if app.worktrees.is_empty() {
            rows.push(Line::from(if app.worktrees_loading {
                "Checking tracked handoffs…"
            } else {
                "No outstanding OnTrack-tracked handoffs."
            }));
            rows.push(Line::from("Existing worktrees are not adopted."));
        }
        for (index, item) in app
            .worktrees
            .iter()
            .enumerate()
            .skip(app.worktree_offset)
            .take(visible)
        {
            let selected = index == app.selected_worktree;
            let style = if selected {
                Style::default()
                    .fg(palette.selected_foreground)
                    .bg(palette.selected_background)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(ink)
            };
            let mut depth = 0usize;
            let mut parent = item.parent_bead_id.as_deref();
            while let Some(id) = parent {
                let Some(ancestor) = app
                    .worktrees
                    .iter()
                    .find(|candidate| candidate.bead_id == id)
                else {
                    break;
                };
                depth += 1;
                if depth >= 4 {
                    break;
                }
                parent = ancestor.parent_bead_id.as_deref();
            }
            let width = panes[0].width.saturating_sub(2) as usize;
            let prefix = truncate_display(
                &format!(
                    "{} {}{} · ",
                    if selected { "›" } else { " " },
                    "  ".repeat(depth),
                    item.bead_id
                ),
                width,
            );
            let remaining = width.saturating_sub(Span::raw(prefix.as_str()).width());
            let label = truncate_display(crate::app::worktree_status_label(item.status), remaining);
            rows.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(label, worktree_status_style(app.theme, item.status)),
            ]));
            rows.push(Line::from(Span::styled(
                truncate_display(
                    &format!(
                        "  {}{} → {}",
                        "  ".repeat(depth),
                        item.branch,
                        item.target_branch
                    ),
                    panes[0].width.saturating_sub(2) as usize,
                ),
                Style::default().fg(muted),
            )));
        }
        f.render_widget(
            Paragraph::new(rows)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" TRACKED HANDOFFS ")
                        .border_style(Style::default().fg(palette.border)),
                )
                .style(Style::default().fg(ink).bg(palette.panel)),
            panes[0],
        );
        let selected = app.worktrees.get(app.selected_worktree);
        let detail = if let Some(item) = selected {
            let live_bead = app.beads.iter().find(|bead| bead.id == item.bead_id);
            let title = live_bead
                .map(|bead| bead.title.as_str())
                .filter(|title| !title.trim().is_empty())
                .or(item.bead_title.as_deref())
                .filter(|title| !title.trim().is_empty())
                .unwrap_or("(not recorded at handoff)");
            let description = live_bead
                .and_then(|bead| bead.data.get("description"))
                .and_then(serde_json::Value::as_str)
                .or(item.bead_description.as_deref());
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(
                        format!("{} · ", item.bead_id),
                        Style::default().fg(teal).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        crate::app::worktree_status_label(item.status),
                        worktree_status_style(app.theme, item.status),
                    ),
                ]),
                Line::from(format!("Branch: {}", item.branch)),
                Line::from(format!("Target: {}", item.target_branch)),
                Line::from(format!("Herdr workspace: {}", item.workspace_id)),
            ];
            if let Some(detail) = &item.detail {
                lines.push(Line::from(Span::styled(
                    format!("Detail: {detail}"),
                    Style::default().fg(
                        if matches!(
                            item.status,
                            crate::worktrees::WorktreeStatus::NeedsAttention
                                | crate::worktrees::WorktreeStatus::NeedsResolution
                        ) {
                            palette.danger
                        } else {
                            muted
                        },
                    ),
                )));
            }
            lines.push(Line::default());
            lines.push(Line::from(vec![
                Span::styled("Title: ", Style::default().fg(muted)),
                Span::raw(title),
            ]));
            lines.push(Line::from(Span::styled(
                "Description:",
                Style::default().fg(muted),
            )));
            match description {
                Some(description) if !description.trim().is_empty() => {
                    lines.extend(description.split('\n').map(Line::from));
                }
                _ => lines.push(Line::from("(none recorded at handoff)")),
            }
            lines
        } else {
            vec![Line::from("Select a tracked handoff to inspect its state.")]
        };
        let detail_width = panes[1].width.saturating_sub(2);
        let detail_height = panes[1].height.saturating_sub(2);
        let line_count = Paragraph::new(detail.clone())
            .wrap(Wrap { trim: false })
            .line_count(detail_width);
        app.worktree_detail_offset = app.worktree_detail_offset.min(
            line_count
                .saturating_sub(detail_height as usize)
                .min(u16::MAX as usize) as u16,
        );
        f.render_widget(
            Paragraph::new(detail)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" HANDOFF DETAILS ")
                        .border_style(Style::default().fg(palette.border)),
                )
                .wrap(Wrap { trim: false })
                .scroll((app.worktree_detail_offset, 0))
                .style(Style::default().fg(ink).bg(bg)),
            panes[1],
        );
        f.render_widget(
            Paragraph::new(worktree_footer_text(areas[2].width as usize))
                .style(Style::default().fg(muted).bg(palette.panel_alt)),
            areas[2],
        );
        if let Some(crate::app::Modal::ConfirmWorktree { id, action }) = &app.modal {
            let restart = *action == 3;
            let height = if restart { 12 } else { 8 };
            if a.width < 50 || a.height < height {
                app.modal_ready = false;
                f.render_widget(Clear, a);
                f.render_widget(
                    Paragraph::new("Resize terminal to confirm · Esc cancels")
                        .style(Style::default().fg(gold))
                        .alignment(Alignment::Center),
                    a,
                );
            } else {
                app.modal_ready = true;
                let width = 62.min(a.width.saturating_sub(2));
                let area = Rect::new(
                    a.x + a.width.saturating_sub(width) / 2,
                    a.y + a.height.saturating_sub(height) / 2,
                    width,
                    height,
                );
                f.render_widget(Clear, area);
                f.render_widget(
                    Block::default()
                        .title(if restart {
                            " RESTART TASK? "
                        } else {
                            " CONFIRM WORKTREE ACTION "
                        })
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(palette.border))
                        .style(Style::default().bg(palette.panel)),
                    area,
                );
                let prompt = match action {
                    0 => {
                        "Request merge resolution from the recorded agent? This is pending, not merged."
                    }
                    1 => "Close the integrated Herdr workspace?",
                    2 => {
                        "Remove integrated checkout and close its Herdr workspace? Branch is preserved."
                    }
                    _ => {
                        "Restart this task from scratch?\n\nDiscard all uncommitted checkout changes.\nThe current Herdr workspace will close.\nThe old branch is kept. A fresh worktree\nand agent will start for this Bead."
                    }
                };
                let inner = Rect::new(area.x + 2, area.y + 1, area.width - 4, area.height - 2);
                f.render_widget(
                    Paragraph::new(format!(
                        "{prompt}\n\nWorktree: {id}\n\ny confirm · n / Esc cancel"
                    ))
                    .wrap(Wrap { trim: false })
                    .style(Style::default().fg(ink)),
                    inner,
                );
            }
        }
        render_toast(f, app);
        render_help(f, app);
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
    render_header(
        f,
        v[0],
        Line::from(vec![
            Span::styled(
                " BEADS ",
                Style::default()
                    .fg(palette.selected_foreground)
                    .bg(palette.selected_background)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  ONTRACK  /  WORKSPACE ISSUES"),
        ]),
        app,
    );
    let feedback = if let Some(error) = &app.list_error {
        Some(format!("LIST ERROR · {error}"))
    } else if let Some(error) = &app.detail_error {
        Some(format!("DETAIL ERROR · {error}"))
    } else {
        app.notice.clone()
    };
    render_header_feedback(f, v[0], feedback.as_deref(), app.theme);
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(v[1]);
    app.list_rect = split[0];
    app.detail_rect = split[1];
    let layout = crate::app::list_layout(&app.beads, &app.collapsed);
    let bead_positions = app
        .beads
        .iter()
        .enumerate()
        .map(|(index, bead)| (bead.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let (type_width, priority_width) = metadata_column_widths(&app.beads);
    let viewport = split[0].height as usize;
    let focused_target = app
        .collapse_context
        .as_ref()
        .and_then(|status| {
            layout.targets.iter().find(|target| {
                target.row == DisplayRow::Group(status.clone()) && app.collapsed.contains(status)
            })
        })
        .or_else(|| {
            app.selected
                .and_then(|i| layout.targets.iter().find(|t| t.row == DisplayRow::Bead(i)))
        });
    if let Some(target) = focused_target {
        let keep = target.height.min(viewport);
        if target.y < app.list_offset {
            app.list_offset = target.y;
        } else if target.y + keep > app.list_offset + viewport {
            app.list_offset = target.y + keep - viewport;
        }
    }
    app.list_offset = app.list_offset.min(layout.height.saturating_sub(viewport));
    for group in &layout.groups {
        let natural_top = group.y;
        let natural_bottom = group.y + group.height;
        let clip_top = natural_top.max(app.list_offset);
        let clip_bottom = natural_bottom.min(app.list_offset + viewport);
        if clip_top >= clip_bottom {
            continue;
        }
        let color = themed_status_color(&group.status, app.theme);
        let border = Style::default().fg(color);
        // Draw the sides only over the true intersection. Top and bottom are
        // drawn at their natural rows, so scrolling never creates fake edges.
        let side_y = split[0].y + clip_top.saturating_sub(app.list_offset) as u16;
        let side_h = (clip_bottom - clip_top) as u16;
        for x in [split[0].x, split[0].x + split[0].width.saturating_sub(1)] {
            let side_lines = (0..side_h).map(|_| Line::from("│")).collect::<Vec<_>>();
            f.render_widget(
                Paragraph::new(side_lines).style(border),
                Rect::new(x, side_y, 1, side_h),
            );
        }
        if natural_top >= app.list_offset && natural_top < app.list_offset + viewport {
            let y = split[0].y + (natural_top - app.list_offset) as u16;
            let label = if group.status.is_empty() {
                "UNKNOWN".to_owned()
            } else {
                group.status.to_uppercase()
            };
            let marker = if group.collapsed { "▸" } else { "▾" };
            let focused =
                app.collapse_context.as_deref() == Some(group.status.as_str()) && group.collapsed;
            let title_style = if focused {
                Style::default()
                    .fg(palette.selected_foreground)
                    .bg(palette.selected_background)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            };
            let title = truncate_display(
                &format!(" {marker} {label} · {} ", group.indices.len()),
                split[0].width.saturating_sub(2) as usize,
            );
            let fill = (split[0].width.saturating_sub(2) as usize)
                .saturating_sub(Span::raw(title.as_str()).width());
            let top = Line::from(vec![
                Span::styled("╭", border),
                Span::styled(title, title_style),
                Span::styled("─".repeat(fill), border),
                Span::styled("╮", border),
            ]);
            f.render_widget(
                Paragraph::new(top),
                Rect::new(split[0].x, y, split[0].width, 1),
            );
        }
        if natural_bottom > app.list_offset && natural_bottom <= app.list_offset + viewport {
            let y = split[0].y + (natural_bottom - 1 - app.list_offset) as u16;
            f.render_widget(
                Paragraph::new(format!(
                    "╰{}╯",
                    "─".repeat(split[0].width.saturating_sub(2) as usize)
                ))
                .style(border),
                Rect::new(split[0].x, y, split[0].width, 1),
            );
        }
        for target in layout
            .targets
            .iter()
            .filter(|t| matches!(&t.row,DisplayRow::Bead(i) if group.indices.contains(i)))
        {
            let DisplayRow::Bead(i) = target.row else {
                continue;
            };
            let bead = &app.beads[i];
            for line in 0..target.height {
                let natural_y = target.y + line;
                if natural_y < app.list_offset
                    || natural_y >= app.list_offset + viewport
                    || natural_y <= group.y
                    || natural_y >= natural_bottom - 1
                {
                    continue;
                }
                let screen_y = split[0].y + (natural_y - app.list_offset) as u16;
                let selected = app.selected == Some(i);
                let content = if line == 0 {
                    let kind = truncate_display(&row_issue_type(bead), type_width);
                    let kind_width = Span::raw(kind.as_str()).width();
                    let kind = format!(
                        "{kind}{}",
                        " ".repeat(type_width.saturating_sub(kind_width))
                    );
                    let status = crate::app::display_status(bead);
                    let handed_off = bead.status == "open"
                        && (app.accepted_handoffs.contains(&bead.id)
                            || app.worktrees.iter().any(|worktree| {
                                worktree.bead_id == bead.id && worktree.handoff_accepted
                            }));
                    let mut spans = vec![
                        Span::styled(
                            if selected { "› " } else { "  " },
                            Style::default()
                                .fg(if selected { teal } else { muted })
                                .add_modifier(if selected {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                        Span::styled(
                            kind,
                            Style::default()
                                .fg(themed_type_color(&row_issue_type(bead), app.theme)),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            {
                                let priority = format!("P{}", bead.priority);
                                format!(
                                    "{priority}{}",
                                    " ".repeat(
                                        priority_width
                                            .saturating_sub(Span::raw(priority.as_str()).width())
                                    )
                                )
                            },
                            Style::default().fg(themed_priority_color(bead.priority, app.theme)),
                        ),
                        Span::raw("  "),
                        Span::styled(
                            status,
                            Style::default().fg(themed_status_color(status, app.theme)),
                        ),
                    ];
                    if handed_off {
                        spans.push(Span::styled(
                            " · sent",
                            Style::default().fg(gold).add_modifier(Modifier::BOLD),
                        ));
                    }
                    Line::from(spans)
                } else {
                    let path = crate::app::issue_path(&app.beads, &bead_positions, i);
                    let depth = path.len().saturating_sub(1);
                    let width = split[0].width.saturating_sub(2) as usize;
                    let parent = bead.data.get("parent").and_then(|value| value.as_str());
                    let prefix = if depth > 0 {
                        format!("{}↳ ", "  ".repeat(depth.min(width.saturating_sub(4) / 2)))
                    } else if let Some(parent) = parent.filter(|parent| !parent.trim().is_empty()) {
                        format!(
                            "  ↳ {} · ",
                            truncate_display(parent, width.saturating_sub(8) / 2)
                        )
                    } else {
                        "  ".to_owned()
                    };
                    Line::from(vec![
                        Span::raw(prefix),
                        Span::styled(bead.title.clone(), Style::default().fg(ink)),
                    ])
                };
                f.render_widget(
                    Paragraph::new(content),
                    Rect::new(
                        split[0].x + 1,
                        screen_y,
                        split[0].width.saturating_sub(2),
                        1,
                    ),
                );
            }
        }
    }
    let selected = app.selected.and_then(|i| app.beads.get(i));
    let detail = app.detail.as_ref().or_else(|| selected.map(|b| &b.data));
    let inner_width = split[1].width.saturating_sub(2).max(1);
    let selected_title = selected.map(|b| b.title.as_str());
    let mut detail_text = detail
        .map(|v| crate::detail::render(v, selected_title, inner_width))
        .unwrap_or_else(|| {
            ratatui::text::Text::from(if app.beads.is_empty() {
                "No issues found in this workspace.\n\nThe issue list refreshes automatically."
            } else {
                "Select an issue to inspect its full record."
            })
        });
    for line in &mut detail_text.lines {
        for span in &mut line.spans {
            let old = span.style;
            let style = old.fg(old
                .fg
                .map(|color| crate::theme::markdown_foreground(app.theme, color))
                .unwrap_or(ink));
            let code = old.fg == Some(Color::Rgb(239, 183, 99))
                && !old.add_modifier.contains(Modifier::BOLD);
            // tui-markdown marks inline code by its code-foreground style, but
            // leaves the background unset. Keep its established panel backing
            // while the surrounding detail copy inherits the list background.
            span.style = old
                .bg
                .map(|background| {
                    style.bg(crate::theme::markdown_background(app.theme, background))
                })
                .unwrap_or_else(|| {
                    style.bg(if code {
                        palette.panel
                    } else {
                        palette.background
                    })
                });
        }
    }
    let lines = Paragraph::new(detail_text.clone())
        .wrap(Wrap { trim: false })
        .line_count(inner_width);
    let viewport = split[1].height.saturating_sub(2);
    app.detail_offset = app.detail_offset.min(
        lines
            .saturating_sub(viewport as usize)
            .min(u16::MAX as usize) as u16,
    );
    let title = selected
        .map(|b| format!(" {} · {} · P{} ", b.id, b.status, b.priority))
        .unwrap_or_else(|| " ISSUE DETAILS ".into());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(if app.focus_detail {
            teal
        } else {
            palette.border
        }))
        .style(Style::default().bg(palette.background));
    f.render_widget(
        Paragraph::new(detail_text)
            .block(block)
            .style(Style::default().fg(ink).bg(palette.background))
            .wrap(Wrap { trim: false })
            .scroll((app.detail_offset, 0)),
        split[1],
    );
    f.render_widget(
        Paragraph::new(footer_text(v[2].width as usize))
            .style(Style::default().fg(muted).bg(palette.panel_alt)),
        v[2],
    );
    if let Some(modal) = &app.modal {
        let width = match modal {
            crate::app::Modal::Create(_) | crate::app::Modal::Edit { .. } => {
                FORM_DESCRIPTION_WIDTH as u16 + 6
            }
            crate::app::Modal::Settings { .. } => 64,
            crate::app::Modal::Release | crate::app::Modal::ConfirmUpdate(_) => 76,
            _ => 62,
        }
        .min(a.width.saturating_sub(2));
        let delete_lines = match modal {
            crate::app::Modal::ConfirmDelete { id, title } => Some(delete_dialog_lines(
                id,
                title,
                width.saturating_sub(4) as usize,
                app.notice.as_deref(),
                app.mutation_busy,
            )),
            _ => None,
        };
        let height = match modal {
            crate::app::Modal::Settings {
                error,
                dropdown_open,
                draft,
                focus,
            } => {
                let error_lines = error
                    .as_deref()
                    .map(|message| wrap_words(message, width.saturating_sub(4) as usize).len())
                    .unwrap_or(0);
                let hint_lines = wrap_words(
                    settings_hint(*dropdown_open, *focus),
                    width.saturating_sub(4) as usize,
                )
                .len();
                let option_lines = if *dropdown_open {
                    if *focus == 0 {
                        let (start, count) = visible_theme_window(draft.theme, a.height);
                        count
                            + usize::from(start > 0)
                            + usize::from(start + count < crate::theme::CATALOG.len())
                    } else if *focus == 1 {
                        let (start, count) = visible_agent_window(draft.default_agent, a.height);
                        count
                            + usize::from(start > 0)
                            + usize::from(start + count < crate::beads::HARNESSES.len() + 2)
                    } else if *focus == 2 {
                        let (start, count) = visible_limit_window(draft.agent_limit, a.height);
                        count
                            + usize::from(start > 0)
                            + usize::from(start + count as u64 <= u32::MAX as u64)
                    } else {
                        0
                    }
                } else {
                    0
                };
                (11 + option_lines + error_lines + hint_lines + 2) as u16
            }
            crate::app::Modal::Create(form) | crate::app::Modal::Edit { form, .. } => {
                let epic_toggle = matches!(modal, crate::app::Modal::Create(_))
                    && crate::beads::BEAD_TYPES[form.issue_type] == "epic";
                (if form.dropdown.is_some() { 22 } else { 18 }) + u16::from(epic_toggle)
            }
            crate::app::Modal::EditLoading { .. } => 8,
            crate::app::Modal::ConfirmDelete { .. } => delete_lines
                .as_ref()
                .map_or(10, |lines| (lines.len() + 2).max(10) as u16),
            crate::app::Modal::ConfirmWorktree { action: 3, .. } => 12,
            crate::app::Modal::ConfirmWorktree { .. } => 8,
            crate::app::Modal::Release => 12,
            crate::app::Modal::ConfirmUpdate(_) => 10,
            crate::app::Modal::InstallingUpdate(_) => 8,
        };
        let min_width = if matches!(
            modal,
            crate::app::Modal::Create(_) | crate::app::Modal::Edit { .. }
        ) {
            70
        } else if matches!(
            modal,
            crate::app::Modal::Settings { .. }
                | crate::app::Modal::EditLoading { .. }
                | crate::app::Modal::Release
                | crate::app::Modal::ConfirmUpdate(_)
        ) {
            38
        } else {
            50
        };
        if a.width < min_width || a.height < height {
            app.modal_ready = false;
            f.render_widget(Clear, a);
            let escape_hint = if matches!(modal, crate::app::Modal::Settings { .. }) {
                "Esc closes"
            } else {
                "Esc cancels"
            };
            let prompt = if a.width < min_width {
                format!("Expand terminal width to {min_width} columns · {escape_hint}")
            } else {
                format!("Expand terminal height to {height} rows · {escape_hint}")
            };
            f.render_widget(
                Paragraph::new(prompt)
                    .style(Style::default().fg(gold))
                    .alignment(Alignment::Center),
                a,
            );
            render_toast(f, app);
            render_help(f, app);
            return;
        }
        app.modal_ready = true;
        let area = Rect::new(
            a.x + a.width.saturating_sub(width) / 2,
            a.y + a.height.saturating_sub(height) / 2,
            width,
            height,
        );
        f.render_widget(Clear, area);
        let frame = Block::default()
            .title(match modal {
                crate::app::Modal::Create(form) if form.parent_id.is_some() => " NEW CHILD TASK ",
                crate::app::Modal::Create(_) => " NEW BEAD ",
                crate::app::Modal::EditLoading { .. } | crate::app::Modal::Edit { .. } => {
                    " EDIT BEAD "
                }
                crate::app::Modal::Settings { .. } => " SETTINGS ",
                crate::app::Modal::Release => " ONTRACK RELEASE ",
                crate::app::Modal::ConfirmUpdate(_) => " UPDATE ONTRACK? ",
                crate::app::Modal::InstallingUpdate(_) => " INSTALLING ONTRACK ",
                crate::app::Modal::ConfirmWorktree { action: 3, .. } => " RESTART TASK? ",
                crate::app::Modal::ConfirmWorktree { .. } => " CONFIRM WORKTREE ACTION ",
                _ => " DELETE BEAD? ",
            })
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.panel));
        f.render_widget(frame, area);
        let inner = Rect::new(
            area.x + 2,
            area.y + 1,
            area.width.saturating_sub(4),
            area.height.saturating_sub(2),
        );
        match modal {
            crate::app::Modal::Release => {
                let version = env!("CARGO_PKG_VERSION");
                let result = if app.update_preparing {
                    "Checking for a compatible release asset…".to_owned()
                } else if app.release_inflight {
                    "Checking GitHub for the latest release…".to_owned()
                } else {
                    match app.release_result.as_ref() {
                        Some(Ok(crate::release::CheckResult::Available { version, url })) => {
                            format!(
                                "Update available: v{version}\n\nRelease link:\n{url}\n\nPress i to prepare installation."
                            )
                        }
                        Some(Ok(crate::release::CheckResult::Current { .. })) => {
                            "You are running the latest published release.".into()
                        }
                        Some(Ok(crate::release::CheckResult::NoPublishedRelease)) => {
                            "No public OnTrack release is available.".into()
                        }
                        Some(Err(error)) => format!("Update check failed: {error}"),
                        None => "Check not started.".into(),
                    }
                };
                f.render_widget(
                    Paragraph::new(format!(
                        "Running version: v{version}\n\n{result}\n\nr check again · Esc return to browser"
                    ))
                    .wrap(Wrap { trim: false })
                    .style(Style::default().fg(ink)),
                    inner,
                );
            }
            crate::app::Modal::ConfirmUpdate(release) => {
                f.render_widget(
                    Paragraph::new(format!("OnTrack {} is available.\n\n{}\n\nInstall the verified release?\ny confirm · n / Esc cancel", release.version, release.page))
                        .wrap(Wrap { trim: false })
                        .style(Style::default().fg(ink)),
                    inner,
                );
            }
            crate::app::Modal::InstallingUpdate(version) => {
                f.render_widget(
                    Paragraph::new(format!("Downloading and verifying OnTrack {version}…\n\nEsc or c cancels before replacement.\nKeep this session open until the result appears."))
                        .wrap(Wrap { trim: false })
                        .style(Style::default().fg(ink)),
                    inner,
                );
            }
            crate::app::Modal::EditLoading { id, error, .. } => {
                let message = error.as_ref().map_or_else(
                    || format!("Loading {id}…"),
                    |error| format!("Could not load {id}: {error}\nPress e to retry · Esc cancels"),
                );
                f.render_widget(
                    Paragraph::new(message)
                        .wrap(Wrap { trim: false })
                        .style(Style::default().fg(if error.is_some() { gold } else { ink })),
                    inner,
                );
            }
            crate::app::Modal::Settings {
                draft,
                error,
                dropdown_open,
                focus,
            } => {
                let selected_style = Style::default()
                    .fg(palette.selected_foreground)
                    .bg(palette.selected_background)
                    .add_modifier(Modifier::BOLD);
                let value_style = Style::default().fg(teal).add_modifier(Modifier::BOLD);
                let row_width = inner.width as usize;
                let mut lines = vec![
                    Line::from("Application preferences").alignment(Alignment::Center),
                    settings_row(
                        "Theme",
                        &format!("{} ▾", draft.theme.label()),
                        row_width,
                        if *focus == 0 {
                            value_style
                        } else {
                            Style::default().fg(ink)
                        },
                        if *focus == 0 {
                            selected_style
                        } else {
                            value_style
                        },
                    ),
                ];
                if *dropdown_open && *focus == 0 {
                    let (start, count) = visible_theme_window(draft.theme, a.height);
                    let end = start + count;
                    if start > 0 {
                        lines.push(settings_row(
                            "",
                            "↑ earlier themes",
                            row_width,
                            Style::default(),
                            Style::default().fg(muted),
                        ));
                    }
                    for theme in Theme::ALL.iter().skip(start).take(count).copied() {
                        let selected = draft.theme == theme;
                        lines.push(settings_row(
                            "",
                            &format!("{} {}", if selected { "›" } else { " " }, theme.label()),
                            row_width,
                            Style::default(),
                            if selected {
                                selected_style
                            } else {
                                Style::default().fg(ink)
                            },
                        ));
                    }
                    if end < crate::theme::CATALOG.len() {
                        lines.push(settings_row(
                            "",
                            "↓ later themes",
                            row_width,
                            Style::default(),
                            Style::default().fg(muted),
                        ));
                    }
                }
                lines.push(settings_row(
                    "Default agent for new tasks",
                    &format!("{} ▾", draft.default_agent.label()),
                    row_width,
                    if *focus == 1 {
                        value_style
                    } else {
                        Style::default().fg(ink)
                    },
                    if *focus == 1 {
                        selected_style
                    } else {
                        value_style
                    },
                ));
                if *dropdown_open && *focus == 1 {
                    let (start, count) = visible_agent_window(draft.default_agent, a.height);
                    if start > 0 {
                        lines.push(settings_row(
                            "",
                            "↑ earlier agents",
                            row_width,
                            Style::default(),
                            Style::default().fg(muted),
                        ));
                    }
                    for index in start..start + count {
                        let agent = crate::settings::DefaultAgent::at(index);
                        let selected = draft.default_agent == agent;
                        lines.push(settings_row(
                            "",
                            &format!("{} {}", if selected { "›" } else { " " }, agent.label()),
                            row_width,
                            Style::default(),
                            if selected {
                                selected_style
                            } else {
                                Style::default().fg(ink)
                            },
                        ));
                    }
                    if start + count < crate::beads::HARNESSES.len() + 2 {
                        lines.push(settings_row(
                            "",
                            "↓ later agents",
                            row_width,
                            Style::default(),
                            Style::default().fg(muted),
                        ));
                    }
                }
                lines.push(settings_row(
                    "Agent limit",
                    &format!("{} ▾", draft.agent_limit.label()),
                    row_width,
                    if *focus == 2 {
                        value_style
                    } else {
                        Style::default().fg(ink)
                    },
                    if *focus == 2 {
                        selected_style
                    } else {
                        value_style
                    },
                ));
                if *dropdown_open && *focus == 2 {
                    let (start, count) = visible_limit_window(draft.agent_limit, a.height);
                    if start > 0 {
                        lines.push(settings_row(
                            "",
                            "↑ earlier limits",
                            row_width,
                            Style::default(),
                            Style::default().fg(muted),
                        ));
                    }
                    for index in start..start + count as u64 {
                        let choice = if index == 0 {
                            crate::settings::AgentLimit::Unlimited
                        } else {
                            crate::settings::AgentLimit::Limited(index as u32)
                        };
                        let selected = draft.agent_limit == choice;
                        lines.push(settings_row(
                            "",
                            &format!("{} {}", if selected { "›" } else { " " }, choice.label()),
                            row_width,
                            Style::default(),
                            if selected {
                                selected_style
                            } else {
                                Style::default().fg(ink)
                            },
                        ));
                    }
                    if start + count as u64 <= u32::MAX as u64 {
                        lines.push(settings_row(
                            "",
                            "↓ later limits",
                            row_width,
                            Style::default(),
                            Style::default().fg(muted),
                        ));
                    }
                }
                for (index, (label, enabled)) in [
                    ("Auto-merge top-level tasks", draft.auto_merge),
                    (
                        "Auto-merge child tasks into parent",
                        draft.auto_merge_children,
                    ),
                    (
                        "Close Herdr workspace after integration",
                        draft.auto_close_workspace,
                    ),
                    (
                        "Remove checkout + close Herdr workspace",
                        draft.auto_remove_checkout,
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let focused = *focus == index + 3;
                    lines.push(settings_row(
                        label,
                        if enabled { "[x]" } else { "[ ]" },
                        row_width,
                        if focused {
                            value_style
                        } else {
                            Style::default().fg(ink)
                        },
                        if focused {
                            selected_style
                        } else {
                            Style::default().fg(ink)
                        },
                    ));
                }
                lines.push(
                    Line::from(Span::styled(
                        "Removing a checkout also closes its Herdr workspace.",
                        Style::default().fg(muted),
                    ))
                    .alignment(Alignment::Center),
                );
                lines.push(Line::from(""));
                lines.push(
                    Line::from(Span::styled(
                        "[ Close ]",
                        if *focus == 7 {
                            selected_style
                        } else {
                            value_style
                        },
                    ))
                    .alignment(Alignment::Center),
                );
                if let Some(error) = error {
                    lines.extend(
                        wrap_words(error, inner.width as usize)
                            .into_iter()
                            .map(|line| {
                                Line::from(Span::styled(line, Style::default().fg(palette.danger)))
                                    .alignment(Alignment::Center)
                            }),
                    );
                }
                lines.extend(
                    wrap_words(settings_hint(*dropdown_open, *focus), inner.width as usize)
                        .into_iter()
                        .map(|line| {
                            Line::from(line)
                                .style(Style::default().fg(muted))
                                .alignment(Alignment::Center)
                        }),
                );
                f.render_widget(
                    Paragraph::new(lines)
                        .wrap(Wrap { trim: false })
                        .style(Style::default().fg(ink)),
                    inner,
                );
            }
            crate::app::Modal::Create(form) | crate::app::Modal::Edit { form, .. } => {
                let editing = matches!(modal, crate::app::Modal::Edit { .. });
                let epic_toggle = !editing && crate::beads::BEAD_TYPES[form.issue_type] == "epic";
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Length(if matches!(form.dropdown, Some(2 | 3)) {
                            7
                        } else {
                            3
                        }),
                        Constraint::Length(if form.dropdown == Some(6) { 7 } else { 3 }),
                        Constraint::Min(3),
                        Constraint::Length(u16::from(epic_toggle)),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ])
                    .split(inner);
                let title_area = Rect::new(rows[0].x, rows[0].y, rows[0].width, 3);
                f.render_widget(
                    Paragraph::new("")
                        .block(
                            Block::default()
                                .title(match &form.parent_id {
                                    Some(parent) => format!(" Title · child of {parent} "),
                                    None => " Title ".to_owned(),
                                })
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(if form.focus == 0 {
                                    teal
                                } else {
                                    muted
                                })),
                        )
                        .style(Style::default().fg(ink))
                        .scroll((0, 0)),
                    title_area,
                );
                let inner_title = Rect::new(
                    title_area.x + 1,
                    title_area.y + 1,
                    title_area.width.saturating_sub(2),
                    1,
                );
                let caret_width =
                    Span::raw(&form.title[..form.cursor.min(form.title.len())]).width();
                let title_view = inner_title.width.saturating_sub(1) as usize;
                let desired_scroll = caret_width.saturating_sub(title_view.saturating_sub(1));
                let (title_slice, title_scroll) =
                    display_window(&form.title, desired_scroll, inner_title.width as usize);
                f.render_widget(
                    Paragraph::new(title_slice).style(Style::default().fg(ink)),
                    inner_title,
                );
                if form.focus == 0 {
                    f.set_cursor_position(Position::new(
                        inner_title.x
                            + caret_width
                                .saturating_sub(title_scroll)
                                .min(inner_title.width.saturating_sub(1) as usize)
                                as u16,
                        inner_title.y,
                    ));
                }
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                    .split(rows[1]);
                let type_area = cols[0];
                let priority_area = cols[1];
                let dropdown = form.dropdown;
                let box_height = type_area.height;
                for (which, area) in [(2usize, type_area), (3usize, priority_area)] {
                    let selected = if which == 2 {
                        form.issue_type
                    } else {
                        form.priority
                    };
                    let title = if which == 2 {
                        format!(
                            "Type: {}{}",
                            capitalize(bead_type_name(selected)),
                            if dropdown == Some(which) { "" } else { "  ▾" }
                        )
                    } else {
                        format!(
                            "Priority: P{selected}{}",
                            if dropdown == Some(which) { "" } else { "  ▾" }
                        )
                    };
                    let color = if which == 2 {
                        crate::theme::issue_type_color(
                            app.theme,
                            crate::beads::BEAD_TYPES[selected],
                        )
                    } else {
                        crate::theme::priority_color(app.theme, selected as u64)
                    };
                    let border = if form.focus == which || dropdown == Some(which) {
                        teal
                    } else {
                        muted
                    };
                    let block = Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border));
                    f.render_widget(block, area);
                    if dropdown == Some(which) {
                        let options = if which == 2 {
                            crate::beads::BEAD_TYPES
                                .iter()
                                .enumerate()
                                .map(|(i, x)| {
                                    (
                                        i,
                                        x.to_string(),
                                        crate::theme::issue_type_color(app.theme, x),
                                    )
                                })
                                .collect::<Vec<_>>()
                        } else {
                            (0..=4)
                                .map(|i| {
                                    (
                                        i,
                                        format!("P{i}"),
                                        crate::theme::priority_color(app.theme, i as u64),
                                    )
                                })
                                .collect()
                        };
                        let start = if which == 2 {
                            selected.saturating_sub(box_height.saturating_sub(2) as usize - 1)
                        } else {
                            0
                        };
                        for (row, (idx, label, col)) in options
                            .iter()
                            .skip(start)
                            .take(box_height.saturating_sub(2) as usize)
                            .enumerate()
                        {
                            let y = area.y + 1 + row as u16;
                            let line = Rect::new(area.x + 1, y, area.width.saturating_sub(2), 1);
                            let style = if *idx == selected {
                                Style::default()
                                    .fg(palette.selected_foreground)
                                    .bg(palette.selected_background)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(*col)
                            };
                            f.render_widget(
                                Paragraph::new(format!(
                                    "{} {label}",
                                    if *idx == selected { "›" } else { " " }
                                ))
                                .style(style),
                                line,
                            );
                        }
                    } else {
                        let line =
                            Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1);
                        f.render_widget(
                            Paragraph::new(if which == 2 {
                                capitalize(bead_type_name(selected))
                            } else {
                                format!("P{selected}")
                            })
                            .style(Style::default().fg(color)),
                            line,
                        );
                    }
                }
                let harness_area = rows[2];
                let harness_name = if form.harness == 0 {
                    "Unassigned"
                } else {
                    &form.harnesses[form.harness - 1]
                };
                let harness_border = if form.focus == 6 || form.dropdown == Some(6) {
                    teal
                } else {
                    muted
                };
                f.render_widget(
                    Block::default()
                        .title(format!("Agent harness (tasks): {harness_name}  ▾"))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(harness_border)),
                    harness_area,
                );
                if form.dropdown == Some(6) {
                    let options = std::iter::once("Unassigned")
                        .chain(form.harnesses.iter().map(String::as_str))
                        .collect::<Vec<_>>();
                    let count = harness_area.height.saturating_sub(2) as usize;
                    let start = form.harness.saturating_sub(count.saturating_sub(1));
                    for (offset, label) in options.iter().enumerate().skip(start).take(count) {
                        let style = if offset == form.harness {
                            Style::default()
                                .fg(palette.selected_foreground)
                                .bg(palette.selected_background)
                        } else {
                            Style::default().fg(ink)
                        };
                        f.render_widget(
                            Paragraph::new(format!(
                                "{} {label}",
                                if offset == form.harness { "›" } else { " " }
                            ))
                            .style(style),
                            Rect::new(
                                harness_area.x + 1,
                                harness_area.y + 1 + (offset - start) as u16,
                                harness_area.width.saturating_sub(2),
                                1,
                            ),
                        );
                    }
                } else {
                    f.render_widget(
                        Paragraph::new(if form.harness_loading {
                            "Checking Herdr integrations…"
                        } else if form.harness_error.is_some() {
                            "Unassigned · Herdr unavailable"
                        } else if form.harnesses.is_empty() && !editing {
                            "Unassigned · no current integrations"
                        } else {
                            harness_name
                        })
                        .style(Style::default().fg(ink)),
                        Rect::new(
                            harness_area.x + 1,
                            harness_area.y + 1,
                            harness_area.width.saturating_sub(2),
                            1,
                        ),
                    );
                }
                let desc_area = rows[3];
                debug_assert_eq!(
                    desc_area.width.saturating_sub(2) as usize,
                    FORM_DESCRIPTION_WIDTH
                );
                let (wrapped_description, cx, cy) = wrapped_description(
                    &form.description,
                    form.description_cursor,
                    desc_area.width.saturating_sub(2).max(1) as usize,
                );
                let desc_scroll = cy.saturating_sub(desc_area.height.saturating_sub(3) as usize);
                f.render_widget(
                    Paragraph::new(wrapped_description)
                        .block(
                            Block::default()
                                .title(if editing {
                                    " Description · Ctrl+V image · Shift+Enter newline · Enter Save "
                                } else {
                                    " Description · Ctrl+V image · Shift+Enter newline · Enter Create "
                                })
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(if form.focus == 1 {
                                    teal
                                } else {
                                    muted
                                })),
                        )
                        .style(Style::default().fg(ink))
                        .scroll((desc_scroll as u16, 0)),
                    desc_area,
                );
                if form.focus == 1 {
                    let row = cy.saturating_sub(desc_scroll) as u16;
                    if row < desc_area.height.saturating_sub(2) {
                        f.set_cursor_position(Position::new(
                            desc_area.x
                                + 1
                                + cx.min(desc_area.width.saturating_sub(2) as usize) as u16,
                            desc_area.y + 1 + row,
                        ));
                    }
                }
                if epic_toggle {
                    let label = format!(
                        "[{}] Generate child tasks with a Herdr agent",
                        if form.generate_children { "x" } else { " " }
                    );
                    f.render_widget(
                        Paragraph::new(label).style(if form.focus == 7 {
                            Style::default()
                                .fg(palette.selected_foreground)
                                .bg(palette.selected_background)
                        } else {
                            Style::default().fg(teal)
                        }),
                        rows[4],
                    );
                }
                let actions = Paragraph::new(Line::from(vec![
                    Span::styled(
                        if editing { "[ Save ]" } else { "[ Create ]" },
                        if app.mutation_busy {
                            Style::default().fg(muted)
                        } else if form.focus == 4 {
                            Style::default()
                                .fg(palette.selected_foreground)
                                .bg(palette.selected_background)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(teal)
                        },
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "[ Cancel ]",
                        if app.mutation_busy {
                            Style::default().fg(muted)
                        } else if form.focus == 5 {
                            Style::default()
                                .fg(palette.selected_foreground)
                                .bg(gold)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(gold)
                        },
                    ),
                    Span::raw("  ·  Enter choose · Esc cancel"),
                ]));
                f.render_widget(actions, rows[5]);
                let feedback = if app.mutation_busy {
                    Some(if editing {
                        "Saving… · form locked until the command finishes"
                    } else {
                        "Creating… · form locked until the command finishes"
                    })
                } else {
                    app.notice.as_deref().or(form.harness_error.as_deref())
                };
                let feedback = truncate_display(
                    feedback.unwrap_or(
                        "Tab moves · ←/→ cycles focused choices · Enter opens · Esc cancels",
                    ),
                    rows[6].width as usize,
                );
                f.render_widget(
                    Paragraph::new(feedback).style(Style::default().fg(
                        if app.notice.is_some() || form.harness_error.is_some() || app.mutation_busy
                        {
                            gold
                        } else {
                            muted
                        },
                    )),
                    rows[6],
                );
            }
            crate::app::Modal::ConfirmDelete { id, title } => {
                let text = if app.mutation_busy {
                    format!("Deleting bead\nID: {id}\n\nCommand running · dialog locked")
                } else {
                    delete_lines
                        .unwrap_or_else(|| vec![format!("ID: {id}"), format!("Title: {title}")])
                        .join("\n")
                };
                f.render_widget(
                    Paragraph::new(text)
                        .block(Block::default())
                        .style(Style::default().fg(ink)),
                    inner,
                );
            }
            crate::app::Modal::ConfirmWorktree { id, action } => {
                let action = match action {
                    0 => {
                        "Request merge resolution from the recorded agent? This is pending, not merged."
                    }
                    1 => "Close the integrated Herdr workspace?",
                    2 => {
                        "Remove integrated checkout and close its Herdr workspace? Branch is preserved."
                    }
                    _ => {
                        "Restart this task from scratch?\n\nDiscard all uncommitted checkout changes.\nThe current Herdr workspace will close.\nThe old branch is kept. A fresh worktree\nand agent will start for this Bead."
                    }
                };
                f.render_widget(
                    Paragraph::new(format!(
                        "{action}\n\nWorktree: {id}\n\ny confirm · n / Esc cancel"
                    ))
                    .wrap(Wrap { trim: false })
                    .style(Style::default().fg(ink)),
                    inner,
                );
            }
        }
    }
    render_toast(f, app);
    render_help(f, app);
}
fn render_help(f: &mut Frame, app: &mut App) {
    let Some(help) = app.help.as_mut() else {
        return;
    };
    let screen = f.area();
    if screen.width < 18 || screen.height < 5 {
        f.render_widget(Clear, screen);
        f.render_widget(
            Paragraph::new("Expand terminal to view hotkeys · Esc closes")
                .alignment(Alignment::Center),
            screen,
        );
        return;
    }
    let palette = crate::theme::definition(app.theme).palette;
    let width = screen.width.saturating_sub(2).min(82);
    let height = screen.height.saturating_sub(2).min(30);
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    let inner = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(5),
    );
    let key_width = help
        .context
        .entries()
        .iter()
        .map(|(key, _)| Span::raw(*key).width())
        .max()
        .unwrap_or(12)
        .min(20)
        .min((inner.width as usize).saturating_sub(6).max(1));
    let action_width = (inner.width as usize).saturating_sub(key_width + 2).max(1);
    let mut lines = Vec::new();
    for (key, action) in help.context.entries() {
        let keys = wrap_words(key, key_width);
        let actions = wrap_words(action, action_width);
        for row in 0..keys.len().max(actions.len()) {
            let label = keys.get(row).map(String::as_str).unwrap_or("");
            let padding = key_width.saturating_sub(Span::raw(label).width());
            let description = actions.get(row).map(String::as_str).unwrap_or("");
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{label}{}  ", " ".repeat(padding)),
                    Style::default().fg(palette.accent),
                ),
                Span::styled(
                    description.to_owned(),
                    Style::default().fg(palette.foreground),
                ),
            ]));
        }
    }
    let viewport = usize::from(inner.height).max(1);
    help.offset = help.offset.min(lines.len().saturating_sub(viewport));
    f.render_widget(Clear, area);
    f.render_widget(
        Block::default()
            .title(format!(" HOTKEYS · {} ", help.context.title()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.panel)),
        area,
    );
    f.render_widget(
        Paragraph::new(lines).scroll((help.offset.min(u16::MAX as usize) as u16, 0)),
        inner,
    );
    let footer = if area.width < 45 {
        "Esc close · ↑↓ scroll"
    } else {
        "Esc / Ctrl+? close · ↑↓ / PgUp PgDn scroll"
    };
    f.render_widget(
        Paragraph::new(truncate_display(
            footer,
            area.width.saturating_sub(4) as usize,
        ))
        .style(Style::default().fg(palette.muted)),
        Rect::new(
            area.x + 2,
            area.y + area.height - 2,
            area.width.saturating_sub(4),
            1,
        ),
    );
}
fn render_toast(f: &mut Frame, app: &App) {
    let Some(toast) = app
        .toast
        .as_ref()
        .filter(|toast| toast.visible(std::time::Instant::now()))
    else {
        return;
    };
    let area = f.area();
    let width = area.width.saturating_sub(2).min(56);
    if width < 4 || area.height < 3 {
        return;
    }
    let text_width = usize::from(width.saturating_sub(4)).max(1);
    let mut lines = wrap_words(&toast.message, text_width);
    let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(1));
    let visible_lines = usize::from(height.saturating_sub(2));
    if lines.len() > visible_lines && visible_lines > 0 {
        lines.truncate(visible_lines);
        lines[visible_lines - 1] = truncate_display("… press w for handoffs", text_width);
    }
    let colors = crate::theme::definition(app.theme).palette;
    let border = if toast.error {
        colors.danger
    } else {
        colors.accent
    };
    let box_area = Rect::new(area.x + area.width - width, area.y, width, height);
    f.render_widget(Clear, box_area);
    f.render_widget(
        Paragraph::new(lines.join("\n"))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" HERDR ")
                    .border_style(Style::default().fg(border)),
            )
            .style(Style::default().fg(colors.foreground).bg(colors.panel)),
        box_area,
    );
}
fn footer_text(width: usize) -> String {
    let mut footer = truncate_display(
        " n new · e edit · d delete · h hand off · r ready · q quit",
        width,
    );
    for hint in [
        " · N child",
        " · s settings",
        " · w worktrees",
        " · u update",
        " · Ctrl+? keys",
    ] {
        if Span::raw(footer.as_str()).width() + Span::raw(hint).width() <= width {
            footer.push_str(hint);
        }
    }
    footer
}
fn worktree_footer_text(width: usize) -> String {
    let mut footer = String::new();
    for hint in [
        " Enter switch",
        " · m merge",
        " · c close",
        " · x remove",
        " · R restart",
        " · r refresh",
        " · w back",
        " · ↑/↓ select",
        " · PgUp/PgDn scroll",
        " · q quit",
        " · Ctrl+? keys",
        " · k/j select",
        " · Esc/Backspace back",
        " · Ctrl+C quit",
    ] {
        if Span::raw(footer.as_str()).width() + Span::raw(hint).width() <= width {
            footer.push_str(hint);
        }
    }
    footer
}
fn bead_type_name(index: usize) -> &'static str {
    crate::beads::BEAD_TYPES
        .get(index)
        .copied()
        .unwrap_or("task")
}
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|first| first.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}
fn display_window(s: &str, desired_start: usize, width: usize) -> (String, usize) {
    let mut out = String::new();
    let mut cell = 0;
    let mut actual_start = 0;
    let mut visible = 0;
    for ch in s.chars() {
        let cw = Span::raw(ch.to_string()).width();
        if cell < desired_start && cell + cw > desired_start {
            actual_start = cell + cw;
        }
        if cell >= desired_start && visible + cw <= width {
            out.push(ch);
            visible += cw;
        }
        cell += cw;
        if cell <= desired_start {
            actual_start = cell;
        }
        if visible >= width {
            break;
        }
    }
    (out, actual_start)
}
pub(crate) const FORM_DESCRIPTION_WIDTH: usize = 64;

fn description_layout(
    s: &str,
    cursor: usize,
    width: usize,
) -> (String, usize, usize, Vec<(usize, usize, usize)>) {
    let width = width.max(1);
    let cursor = cursor.min(s.len());
    let mut caret = None;
    let mut positions = Vec::with_capacity(s.chars().count() + 1);
    let mut out = String::new();
    let mut row = 0;
    let mut col = 0;
    for (byte, ch) in s.char_indices() {
        if ch == '\n' {
            let position = if col == width {
                (0, row + 1)
            } else {
                (col, row)
            };
            positions.push((byte, position.0, position.1));
            if byte == cursor {
                caret = Some(position);
            }
            out.push('\n');
            row += 1;
            col = 0;
            continue;
        }
        if !ch.is_whitespace()
            && (byte == 0
                || s[..byte]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace))
        {
            let word = s[byte..].split(char::is_whitespace).next().unwrap_or("");
            if col > 0 && col + Span::raw(word).width() > width {
                out.push('\n');
                row += 1;
                col = 0;
            }
        }
        // A separator at the right edge belongs to the next visual line, but
        // need not occupy a cell there before the next word.
        if ch == ' '
            && col == width
            && s[byte + ch.len_utf8()..]
                .chars()
                .next()
                .is_some_and(|next| !next.is_whitespace())
        {
            out.push('\n');
            row += 1;
            col = 0;
            positions.push((byte, col, row));
            if byte == cursor {
                caret = Some((col, row));
            }
            continue;
        }
        let cw = Span::raw(ch.to_string()).width();
        if col == width || col + cw > width {
            out.push('\n');
            row += 1;
            col = 0;
        }
        positions.push((byte, col, row));
        if byte == cursor {
            caret = Some((col, row));
        }
        out.push(ch);
        col += cw;
    }
    let end = if col == width {
        (0, row + 1)
    } else {
        (col, row)
    };
    positions.push((s.len(), end.0, end.1));
    if cursor == s.len() {
        caret = Some(end);
    }
    let (x, y) = caret.unwrap_or((col, row));
    (out, x, y, positions)
}

fn wrapped_description(s: &str, cursor: usize, width: usize) -> (String, usize, usize) {
    let (text, x, y, _) = description_layout(s, cursor, width);
    (text, x, y)
}

pub(crate) fn move_description_vertical(
    s: &str,
    cursor: usize,
    down: bool,
    preferred_column: &mut Option<usize>,
) -> usize {
    let (_, x, y, positions) = description_layout(s, cursor, FORM_DESCRIPTION_WIDTH);
    let column = *preferred_column.get_or_insert(x);
    let target_row = if down { y + 1 } else { y.saturating_sub(1) };
    if target_row == y {
        return cursor;
    }
    positions
        .into_iter()
        .filter(|(_, _, row)| *row == target_row)
        .min_by_key(|(byte, x, _)| (x.abs_diff(column), std::cmp::Reverse(*byte)))
        .map_or(cursor, |(byte, _, _)| byte)
}
fn wrapped_lines(s: &str, width: usize) -> Vec<String> {
    wrapped_description(s, 0, width)
        .0
        .split('\n')
        .map(str::to_owned)
        .collect()
}
fn wrap_words(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for paragraph in s.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if Span::raw(word).width() > width {
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                lines.extend(wrapped_lines(word, width));
            } else if line.is_empty() {
                line.push_str(word);
            } else if Span::raw(&line).width() + 1 + Span::raw(word).width() <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut line));
                line.push_str(word);
            }
        }
        if !line.is_empty() || paragraph.is_empty() {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
fn truncate_display(s: &str, width: usize) -> String {
    if Span::raw(s).width() <= width {
        return s.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let cw = Span::raw(ch.to_string()).width();
        if used + cw + 1 > width {
            break;
        }
        out.push(ch);
        used += cw;
    }
    out.push('…');
    out
}
fn delete_dialog_lines(
    id: &str,
    title: &str,
    width: usize,
    error: Option<&str>,
    busy: bool,
) -> Vec<String> {
    if busy {
        return vec![
            "Deleting bead".into(),
            format!("ID: {id}"),
            "Command running · dialog locked".into(),
        ];
    }
    let mut lines = vec!["Permanently delete this bead?".into()];
    lines.extend(wrapped_lines(&format!("ID: {id}"), width));
    lines.push(format!(
        "Title: {}",
        truncate_display(title, width.saturating_sub(7))
    ));
    lines.push("Cannot be undone.".into());
    lines.extend(wrapped_lines(
        "Dependency links are removed; dependents may be orphaned.",
        width,
    ));
    lines.extend(wrapped_lines(
        "No cascade: other beads will not be deleted.",
        width,
    ));
    if let Some(error) = error {
        lines.push(truncate_display(&format!("Delete error: {error}"), width));
    }
    lines.push("[y] Delete · [n / Enter] Cancel".into());
    lines
}
#[cfg(test)]
fn type_color(index: usize) -> Color {
    [
        Color::Rgb(73, 211, 182),
        Color::Rgb(255, 123, 123),
        Color::Rgb(103, 177, 255),
        Color::Rgb(197, 145, 255),
        Color::Rgb(178, 192, 196),
        Color::Rgb(255, 192, 115),
        Color::Rgb(119, 204, 219),
        Color::Rgb(240, 141, 197),
        Color::Rgb(168, 207, 125),
    ][index.min(8)]
}
#[cfg(test)]
fn priority_color(index: usize) -> Color {
    [
        Color::Rgb(255, 105, 105),
        Color::Rgb(255, 159, 105),
        Color::Rgb(239, 183, 99),
        Color::Rgb(138, 197, 169),
        Color::Rgb(123, 144, 148),
    ][index.min(4)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beads::Bead;
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;
    use std::sync::mpsc;
    #[test]
    fn tracked_child_row_is_indented_under_parent() {
        let mut app = app();
        app.worktrees_view = true;
        let parent = crate::worktrees::ManagedWorktree {
            id: "parent".into(),
            bead_id: "parent".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/parent".into(),
            target_branch: "trunk".into(),
            workspace_id: "wParent".into(),
            status: crate::worktrees::WorktreeStatus::Unmerged,
            detail: None,
        };
        let child = crate::worktrees::ManagedWorktree {
            id: "child".into(),
            bead_id: "child".into(),
            parent_bead_id: Some("parent".into()),
            branch: "ontrack/child".into(),
            target_branch: "ontrack/parent".into(),
            ..parent.clone()
        };
        app.worktrees = vec![parent, child];
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|frame| render(frame, Duration::from_secs(2), &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let row = |y| {
            (0..app.list_rect.width)
                .map(|x| buffer[(app.list_rect.x + x, y)].symbol())
                .collect::<String>()
        };
        assert!(row(app.list_rect.y + 1).contains("› parent"));
        assert!(row(app.list_rect.y + 3).contains("    child"));
    }
    #[test]
    fn issue_rows_indent_nested_children_and_identify_split_parents() {
        let mut app = app();
        let bead = |id: &str, title: &str, parent: Option<&str>, status: &str| Bead {
            id: id.into(),
            title: title.into(),
            status: status.into(),
            priority: 2,
            data: json!({"issue_type": "task", "parent": parent}),
        };
        app.beads = vec![
            bead("child", "Child", Some("parent"), "open"),
            bead("grandchild", "Grandchild", Some("child"), "open"),
            bead("parent", "Parent", None, "open"),
            bead("split", "Split child", Some("parent"), "closed"),
        ];
        app.selected = Some(2);
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal
            .draw(|frame| render(frame, Duration::from_secs(2), &mut app))
            .unwrap();
        let layout = crate::app::list_layout(&app.beads, &app.collapsed);
        let row = |index: usize| {
            let target = layout
                .targets
                .iter()
                .find(|target| target.row == DisplayRow::Bead(index))
                .unwrap();
            let y = app.list_rect.y + (target.y + 1 - app.list_offset) as u16;
            (0..app.list_rect.width)
                .map(|x| terminal.backend().buffer()[(app.list_rect.x + x, y)].symbol())
                .collect::<String>()
        };
        assert!(row(2).contains("  Parent"));
        assert!(row(0).contains("  ↳ Child"));
        assert!(row(1).contains("    ↳ Grandchild"));
        assert!(row(3).contains("↳ parent · Split child"));

        let mut compact = Terminal::new(TestBackend::new(40, 16)).unwrap();
        compact
            .draw(|frame| render(frame, Duration::from_secs(2), &mut app))
            .unwrap();
    }
    fn app() -> App {
        let (tx, _) = mpsc::channel();
        let (_, responses) = mpsc::channel();
        App {
            beads: vec![Bead {
                id: "demo-1".into(),
                title: "Title is visible".into(),
                status: "closed".into(),
                priority: 1,
                data: json!({"issue_type":"task","unknown":{"n":5},"comments":[{"body":"hello comment"}],"unicode":"界界界界界界界界界界"}),
            }],
            collapsed: std::collections::HashSet::new(),
            closed_default_pending: false,
            collapse_context: None,
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
            edit_seq: 0,
            harness_seq: 0,
            launch_seq: 0,
            handoff: Default::default(),
            handoff_capacity: Default::default(),
            toast: None,
            release_result: None,
            release_inflight: false,
            update_preparing: false,
            release_cancel: None,
            update_available: None,
            list_rect: Rect::default(),
            detail_rect: Rect::default(),
            modal: None,
            help: None,
            settings: crate::settings::Settings::default(),
            theme: crate::settings::Theme::Dark,
            settings_path: None,
            settings_load_error: None,
            worktrees: Vec::new(),
            accepted_handoffs: std::collections::HashSet::new(),
            known_handoffs: std::collections::HashSet::new(),
            scheduler_attempted: std::collections::HashSet::new(),
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
            worktrees_next_refresh: std::time::Instant::now(),
            agent_observation_inflight: false,
            next_agent_observation: std::time::Instant::now(),
            notice: None,
            mutation_busy: false,
            modal_ready: true,
            paste_seq: 0,
            paste_pending: false,
            created_screenshots: Vec::new(),
            preferred_id: None,
            refresh_inflight: false,
            refresh_pending: false,
            next_refresh: std::time::Instant::now(),
            requests: tx,
            responses,
        }
    }
    #[test]
    fn release_dialog_shows_version_link_and_error() {
        let mut state = app();
        state.modal = Some(crate::app::Modal::Release);
        state.release_result = Some(Ok(crate::release::CheckResult::Available {
            version: semver::Version::parse("1.2.0").unwrap(),
            url: "https://github.com/ThomasJRyan/OnTrack/releases/tag/v1.2.0".into(),
        }));
        let screen = draw((100, 24), &mut state).0;
        assert!(screen.contains("Update available: v1.2.0"));
        assert!(screen.contains("https://github.com/ThomasJRyan/OnTrack/releases/tag/v1.2.0"));
        assert!(screen.contains("Press i to prepare installation"));
        state.release_result = Some(Err("offline".into()));
        assert!(
            draw((100, 24), &mut state)
                .0
                .contains("Update check failed: offline")
        );
    }
    #[test]
    fn update_confirmation_and_install_progress_show_actions() {
        let mut state = app();
        let release = crate::update::Available {
            version: "1.2.0".into(),
            page: "https://github.com/ThomasJRyan/OnTrack/releases/tag/v1.2.0".into(),
            asset_name: "archive.tar.gz".into(),
            asset_url: String::new(),
            checksum_url: String::new(),
            digest: None,
        };
        state.modal = Some(crate::app::Modal::ConfirmUpdate(release));
        let screen = draw((100, 24), &mut state).0;
        assert!(screen.contains("Install the verified release?"));
        assert!(screen.contains("y confirm"));
        state.modal = Some(crate::app::Modal::InstallingUpdate("1.2.0".into()));
        let screen = draw((100, 24), &mut state).0;
        assert!(screen.contains("Esc or c cancels before replacement"));
    }
    fn draw(size: (u16, u16), a: &mut App) -> (String, Rect) {
        let mut t = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
        t.draw(|f| render(f, Duration::from_secs(2), a)).unwrap();
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
    fn draw_at(size: (u16, u16), elapsed: Duration, app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
        terminal.draw(|frame| render(frame, elapsed, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }
    #[test]
    fn splash_animates_girl_without_moving_her_or_delaying_browser() {
        let mut app = app();
        let idle = draw_at((100, 30), Duration::ZERO, &mut app);
        let wave = draw_at((100, 30), Duration::from_millis(300), &mut app);
        let blink = draw_at((100, 30), Duration::from_millis(600), &mut app);
        assert!(idle.contains("███████"));
        assert!(idle.contains("o   o"));
        assert!(wave.contains("o/"));
        assert!(blink.contains("-   -"));
        assert_eq!(
            idle.find(".-\"\"\"-."),
            wave.find(".-\"\"\"-."),
            "the character should stay centered between poses"
        );
        assert_eq!(idle.find(".-\"\"\"-."), blink.find(".-\"\"\"-."));
        let browser = draw_at((100, 30), SPLASH_DURATION, &mut app);
        assert!(browser.contains("Title is visible"));
        assert!(!browser.contains(".-\"\"\"-."));
    }
    #[test]
    fn splash_uses_character_and_title_when_compact_and_text_when_tiny() {
        let mut app = app();
        let compact = draw_at((60, 18), Duration::from_millis(300), &mut app);
        assert!(compact.contains("ONTRACK"));
        assert!(compact.contains(".-\"\"\"-."));
        assert!(!compact.contains("███████"));
        let tiny = draw_at((24, 7), Duration::from_millis(300), &mut app);
        assert!(tiny.contains("ONTRACK"));
        assert!(!tiny.contains(".-\"\"\"-."));
        app.help = Some(crate::hotkeys::Help {
            context: crate::hotkeys::Context::Issues,
            offset: 0,
        });
        let help = draw_at((60, 18), Duration::ZERO, &mut app);
        assert!(help.contains("HOTKEYS"));
        assert!(!help.contains(".-\"\"\"-."));
    }
    #[test]
    fn update_badge_fits_headers_and_survives_a_handoff_toast() {
        let mut app = app();
        app.update_available = Some("0.2.0".into());
        let wide = draw_at((100, 24), SPLASH_DURATION, &mut app);
        assert!(wide.contains("UPDATE AVAILABLE · v0.2.0"));
        assert!(wide.contains("ONTRACK  /  WORKSPACE ISSUES"));

        let compact = draw_at((24, 7), SPLASH_DURATION, &mut app);
        assert!(compact.contains("UPDATE"));
        assert!(compact.contains("BEADS"));

        app.worktrees_view = true;
        let worktrees = draw_at((100, 24), SPLASH_DURATION, &mut app);
        assert!(worktrees.contains("UPDATE AVAILABLE · v0.2.0"));
        assert!(worktrees.contains("WORKTREES"));

        app.toast = Some(crate::app::Toast {
            message: "Agent started".into(),
            error: false,
            expires_at: None,
        });
        let with_toast = draw_at((100, 24), SPLASH_DURATION, &mut app);
        assert!(with_toast.contains("HERDR"));
        assert!(with_toast.contains("Agent started"));
        app.toast = None;
        assert!(
            draw_at((100, 24), SPLASH_DURATION, &mut app).contains("UPDATE AVAILABLE · v0.2.0")
        );
    }
    #[test]
    fn hotkey_reference_has_columns_and_scrolls_in_compact_terminals() {
        let mut a = app();
        a.help = Some(crate::hotkeys::Help {
            context: crate::hotkeys::Context::Form,
            offset: 0,
        });
        let (wide, _) = draw((80, 15), &mut a);
        assert!(wide.contains("HOTKEYS · Create / edit form"));
        assert!(wide.contains("Text / Space"));
        assert!(wide.contains("Type in title or description"));
        a.help.as_mut().unwrap().offset = usize::MAX;
        let (compact, _) = draw((30, 10), &mut a);
        assert!(a.help.unwrap().offset > 0);
        assert!(compact.contains("HOTKEYS"));
        assert!(compact.contains("Esc"));
    }
    #[test]
    fn renders_exact_two_line_issue_with_status_and_full_nested_data() {
        let mut a = app();
        let (s, _) = draw((120, 40), &mut a);
        assert!(s.contains("task     P1  closed"));
        assert!(!s.contains("demo-1  P1"));
        assert!(s.contains(" demo-1 · closed · P1 "));
        assert!(!s.contains("id: demo-1"));
        assert!(!s.contains("status: closed"));
        assert!(!s.contains("priority: P1"));
        assert!(s.contains("Title is visible"));
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|f| render(f, Duration::from_secs(2), &mut a))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let line = |y: u16| {
            (0..120)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        };
        let bead_row = a.list_rect.y + 1;
        assert!(line(bead_row).contains("task     P1  closed"));
        assert!(line(bead_row + 1).contains("Title is visible"));
        assert_eq!(buffer[(a.list_rect.x, a.list_rect.y)].symbol(), "╭");
        assert_eq!(
            buffer[(a.list_rect.x, a.list_rect.y)].fg,
            status_color("closed")
        );
        assert!(line(a.list_rect.y).contains("CLOSED"));
        assert_eq!(
            buffer[(a.list_rect.x + 3, bead_row)].fg,
            row_type_color("task")
        );
        assert_eq!(
            buffer[(a.list_rect.x + 12, bead_row)].fg,
            crate::theme::priority_color(a.theme, 1)
        );
        assert_eq!(
            buffer[(a.list_rect.x + 16, bead_row)].fg,
            status_color("closed")
        );
        assert_eq!(
            buffer[(a.list_rect.x + 3, bead_row + 1)].fg,
            crate::theme::definition(a.theme).palette.foreground
        );
        assert_eq!(buffer[(a.detail_rect.x, a.detail_rect.y)].symbol(), "╭");
        assert_eq!(
            buffer[(a.detail_rect.x, a.detail_rect.y)].bg,
            crate::theme::definition(a.theme).palette.background
        );
        assert_eq!(
            buffer[(a.detail_rect.x + 1, a.detail_rect.y + 1)].bg,
            crate::theme::definition(a.theme).palette.background
        );
        assert_eq!(
            buffer[(a.list_rect.x + 2, bead_row)].bg,
            buffer[(a.detail_rect.x + 2, a.detail_rect.y + 1)].bg,
            "list and detail text rows share the rendered background"
        );
        assert_eq!(
            buffer[(a.list_rect.x + 2, a.list_rect.y + 4)].bg,
            buffer[(a.detail_rect.x + 2, a.detail_rect.y + 4)].bg,
            "unused list and detail space share the rendered background"
        );
        let group = crate::app::list_layout(&a.beads, &a.collapsed)
            .groups
            .remove(0);
        let bottom = a.list_rect.y + (group.y + group.height - 1 - a.list_offset) as u16;
        assert_eq!(buffer[(a.list_rect.x, bottom)].symbol(), "╰");
        assert!(line(bottom).contains("╯"));
        for x in ["unknown", "comments", "hello comment", "界", "5"] {
            assert!(s.contains(x), "{x}")
        }
    }

    #[test]
    fn open_issue_shows_confirmed_agent_handoff_only() {
        let mut a = app();
        a.beads[0].status = "open".into();
        a.beads[0].data["issue_type"] = json!("task");
        a.beads[0].data["labels"] = json!([crate::beads::READY_LABEL]);
        a.worktrees.push(crate::worktrees::ManagedWorktree {
            id: "wt-1".into(),
            bead_id: "demo-1".into(),
            parent_bead_id: None,
            handoff_accepted: false,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/demo-1".into(),
            target_branch: "main".into(),
            workspace_id: "ws".into(),
            status: crate::worktrees::WorktreeStatus::NeedsAttention,
            detail: None,
        });
        assert!(!draw((80, 24), &mut a).0.contains("· sent"));
        a.worktrees[0].handoff_accepted = true;
        assert!(draw((80, 24), &mut a).0.contains("ready · sent"));
        a.worktrees.clear();
        a.accepted_handoffs.insert("demo-1".into());
        assert!(draw((80, 24), &mut a).0.contains("ready · sent"));
        a.beads[0].status = "in_progress".into();
        assert!(draw((80, 24), &mut a).0.contains("in_progress"));
        assert!(!draw((80, 24), &mut a).0.contains("· sent"));
    }
    #[test]
    fn list_and_detail_backgrounds_match_across_themes() {
        for theme in Theme::ALL {
            let mut a = app();
            a.theme = theme;
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            terminal
                .draw(|f| render(f, Duration::from_secs(2), &mut a))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let panel = crate::theme::definition(theme).palette.background;
            assert_eq!(buffer[(a.detail_rect.x, a.detail_rect.y)].symbol(), "╭");
            assert_eq!(buffer[(a.detail_rect.x, a.detail_rect.y)].bg, panel);
            assert_eq!(buffer[(a.list_rect.x + 2, a.list_rect.y + 1)].bg, panel);
            assert_eq!(buffer[(a.detail_rect.x + 2, a.detail_rect.y + 1)].bg, panel);
            assert_eq!(buffer[(a.list_rect.x + 2, a.list_rect.y + 4)].bg, panel);
            assert_eq!(buffer[(a.detail_rect.x + 2, a.detail_rect.y + 4)].bg, panel);
        }
    }
    #[test]
    fn collapsed_header_focus_is_visible_without_bead_detail_identity() {
        let mut a = app();
        a.collapsed.insert("closed".into());
        a.collapse_context = Some("closed".into());
        a.selected = None;
        a.detail = None;
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|f| render(f, Duration::from_secs(2), &mut a))
            .unwrap();
        let palette = crate::theme::definition(a.theme).palette;
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(a.list_rect.x, a.list_rect.y)].symbol(), "╭");
        assert!(
            (a.list_rect.x + 1..a.list_rect.x + a.list_rect.width - 1)
                .any(|x| buffer[(x, a.list_rect.y)].bg == palette.selected_background)
        );
        assert!(a.selected.is_none());
        assert!(a.detail.is_none());
        assert!(a.modal.is_none());
    }
    #[test]
    fn issue_type_fallback_priority_bounds_and_title_color_are_safe() {
        assert_eq!(row_type_color("task"), TEAL);
        assert_eq!(row_type_color("bug"), Color::Rgb(255, 123, 123));
        assert_eq!(row_type_color("feature"), Color::Rgb(103, 177, 255));
        assert_eq!(row_type_color("epic"), Color::Rgb(197, 145, 255));
        assert_eq!(row_type_color("chore"), Color::Rgb(168, 207, 125));
        assert_eq!(row_type_color("private-kind"), MUTED);
        assert_eq!(row_issue_type(&app().beads[0]), "task");
        let mut bead = app().beads[0].clone();
        bead.data = json!({"type":"bespoke"});
        assert_eq!(row_issue_type(&bead), "bespoke");
        bead.data = json!({"issue_type":null,"type":"bug"});
        assert_eq!(row_issue_type(&bead), "bug");
        bead.data = json!({"issue_type":"  ","type":" feature "});
        assert_eq!(row_issue_type(&bead), "feature");
        bead.data = json!({"issue_type":null});
        assert_eq!(row_issue_type(&bead), "unknown");
        assert_eq!(row_priority_color(u64::MAX), MUTED);
        let mut a = app();
        a.selected = None;
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal
            .draw(|f| render(f, Duration::from_secs(2), &mut a))
            .unwrap();
        let y = a.list_rect.y + 2;
        assert_eq!(
            terminal.backend().buffer()[(a.list_rect.x + 3, y)].fg,
            crate::theme::definition(a.theme).palette.foreground
        );
    }
    #[test]
    fn metadata_columns_align_across_core_custom_and_unknown_types() {
        let mut a = app();
        let kinds = [
            Some("task"),
            Some("feature"),
            Some("custom-extremely-long-type"),
            None,
            Some("bug"),
        ];
        a.beads = kinds
            .iter()
            .enumerate()
            .map(|(i, kind)| crate::beads::Bead {
                id: format!("aligned-{i}"),
                title: format!("title-{i}"),
                status: "open".into(),
                priority: [0, 4, 12, 2, 1][i],
                data: kind.map_or_else(|| json!({}), |kind| json!({"issue_type":kind})),
            })
            .collect();
        a.selected = Some(0);
        let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
        terminal
            .draw(|f| render(f, Duration::from_secs(2), &mut a))
            .unwrap();
        let buf = terminal.backend().buffer();
        let layout = crate::app::list_layout(&a.beads, &a.collapsed);
        let mut priority_columns = Vec::new();
        let mut status_columns = Vec::new();
        for i in 0..a.beads.len() {
            let target = layout
                .targets
                .iter()
                .find(|t| t.row == DisplayRow::Bead(i))
                .unwrap();
            let y = a.list_rect.y + (target.y - a.list_offset) as u16;
            let line = (0..a.list_rect.width)
                .map(|x| buf[(a.list_rect.x + x, y)].symbol())
                .collect::<String>();
            let priority_byte = line.find(&format!("P{}", a.beads[i].priority)).unwrap();
            let status_byte = line.find("open").unwrap();
            priority_columns.push(Span::raw(&line[..priority_byte]).width());
            status_columns.push(Span::raw(&line[..status_byte]).width());
        }
        assert!(
            priority_columns.iter().all(|x| *x == priority_columns[0]),
            "priority columns: {priority_columns:?}"
        );
        assert!(status_columns.iter().all(|x| *x == status_columns[0]));
        assert!(buf.content().iter().any(|cell| cell.symbol() == "…"));
    }
    #[test]
    fn stable_status_colors_and_scrolled_last_group_render_in_small_viewport() {
        assert_eq!(status_color("custom"), Color::Rgb(125, 168, 235));
        assert_eq!(status_color("open"), TEAL);
        assert_eq!(status_color("closed"), Color::Rgb(255, 123, 123));
        let mut a = app();
        a.beads = (0..15)
            .map(|i| crate::beads::Bead {
                id: format!("b{i}"),
                title: format!("item {i}"),
                status: if i == 14 { "closed" } else { "open" }.into(),
                priority: 2,
                data: json!({}),
            })
            .collect();
        a.selected = Some(14);
        let (shown, _) = draw((52, 14), &mut a);
        assert!(shown.contains("CLOSED"));
        assert!(shown.contains("b14"));
        assert!(a.list_offset > 0);
    }
    #[test]
    fn expanded_panes_enclose_rows_and_follow_status_order() {
        let mut a = app();
        a.beads[0].status = "closed".into();
        a.beads.push(crate::beads::Bead {
            id: "open-2".into(),
            title: "open title".into(),
            status: "open".into(),
            priority: 2,
            data: json!({}),
        });
        a.selected = Some(0);
        let layout = crate::app::list_layout(&a.beads, &a.collapsed);
        assert_eq!(
            layout
                .groups
                .iter()
                .map(|g| g.status.as_str())
                .collect::<Vec<_>>(),
            ["open", "closed"]
        );
        assert_eq!(
            layout.groups[0].height,
            2 + 2 * layout.groups[0].indices.len()
        );
        assert_eq!(
            layout.groups[1].y,
            layout.groups[0].y + layout.groups[0].height
        );
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal
            .draw(|f| render(f, Duration::from_secs(2), &mut a))
            .unwrap();
        let buf = terminal.backend().buffer();
        let closed_y = a.list_rect.y + (layout.groups[1].y - a.list_offset) as u16;
        let line = (0..100)
            .map(|x| buf[(x, closed_y)].symbol())
            .collect::<String>();
        assert!(line.contains("CLOSED"));
        let bottom = closed_y + layout.groups[1].height as u16 - 1;
        assert_eq!(buf[(a.list_rect.x, bottom)].symbol(), "╰");
        assert_eq!(buf[(a.list_rect.x, bottom)].fg, status_color("closed"));
        assert!(buf[(a.list_rect.x, closed_y + 1)].symbol() == "│");
    }
    #[test]
    fn partial_scrolled_issue_line_has_no_fabricated_pane_edges() {
        let mut a = app();
        a.beads = (0..8)
            .map(|i| crate::beads::Bead {
                id: format!("long-{i}"),
                title: format!("visible-title-{i}"),
                status: "open".into(),
                priority: 2,
                data: json!({}),
            })
            .collect();
        a.selected = None;
        a.detail = None;
        let layout = crate::app::list_layout(&a.beads, &a.collapsed);
        let target = layout
            .targets
            .iter()
            .find(|t| t.row == DisplayRow::Bead(0))
            .unwrap();
        a.list_offset = target.y + 1;
        let mut terminal = Terminal::new(TestBackend::new(70, 14)).unwrap();
        terminal
            .draw(|f| render(f, Duration::from_secs(2), &mut a))
            .unwrap();
        let buf = terminal.backend().buffer();
        let first_visible = a.list_rect.y;
        let line = (0..a.list_rect.width)
            .map(|x| buf[(a.list_rect.x + x, first_visible)].symbol())
            .collect::<String>();
        assert!(line.contains("visible-title-0"));
        assert!(!line.contains("╭") && !line.contains("╰"));
        assert_eq!(buf[(a.list_rect.x, first_visible)].symbol(), "│");
    }
    #[test]
    fn markdown_description_is_readable_and_styled_in_detail_pane() {
        let mut a = app();
        a.detail = Some(json!({"description":"# Overview\n\nA **bold** point and `code`."}));
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal
            .draw(|f| render(f, Duration::from_secs(2), &mut a))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let cells = buffer.content();
        assert!(
            !cells
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .contains("**bold**")
        );
        let visible = cells.iter().map(|cell| cell.symbol()).collect::<String>();
        assert!(visible.contains("Overview"));
        assert!(
            cells
                .iter()
                .any(|cell| cell.symbol() == "b" && cell.modifier.contains(Modifier::BOLD))
        );
    }
    #[test]
    fn tiny_terminal_startup_and_wrapped_detail_are_safe() {
        let mut a = app();
        a.list_rect = Rect::new(1, 1, 10, 10);
        assert!(!draw((18, 5), &mut a).0.is_empty());
        assert_eq!(a.list_rect, Rect::default());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|f| render(f, Duration::ZERO, &mut a))
            .unwrap();
        assert_eq!(a.list_rect, Rect::default());
        a.list_loading = true;
        let (s, _) = draw((80, 24), &mut a);
        assert!(!s.contains("REFRESHING"));
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
        a.beads[0].data =
            json!({"unknown_multiline": format!("{}FINAL_SENTINEL", "word\n".repeat(800))});
        a.detail = Some(a.beads[0].data.clone());
        a.detail_offset = u16::MAX;
        let (rendered, rect) = draw((50, 14), &mut a);
        assert!(a.detail_offset > 0);
        assert!(rendered.contains("FINAL_SENTINEL"));
        assert!(rect.height > 0);
        let (resized, _) = draw((90, 24), &mut a);
        assert!(resized.contains("FINAL_SENTINEL"));
    }
    #[test]
    fn create_and_delete_modals_render_at_normal_and_tiny_sizes() {
        let mut a = app();
        a.notice = Some("Created previous".into());
        a.list_error = Some("current failure".into());
        let status = draw((120, 40), &mut a).0;
        assert!(status.contains("LIST ERROR"));
        a.list_error = None;
        a.notice = None;
        let status = draw((120, 40), &mut a).0;
        assert!(status.contains("n new"));
        assert!(status.contains("e edit"));
        assert!(status.contains("d delete"));
        assert!(status.contains("h hand off"));
        a.modal = Some(crate::app::Modal::Create(crate::app::CreateForm::default()));
        let normal = draw((100, 30), &mut a).0;
        assert!(normal.contains("NEW BEAD"));
        assert!(normal.contains("Task"));
        assert!(normal.contains("P2"));
        assert!(!normal.contains("Title · required"));
        a.modal = Some(crate::app::Modal::Create(crate::app::CreateForm {
            title: "Visible title".into(),
            cursor: 13,
            ..Default::default()
        }));
        let entered = draw((100, 30), &mut a).0;
        assert!(entered.contains("Visible title"));
        assert!(!entered.contains("Title · required"));
        a.modal = Some(crate::app::Modal::Edit {
            id: "demo-1".into(),
            form: crate::app::CreateForm {
                title: "Edited title".into(),
                cursor: 12,
                description: "Existing description".into(),
                ..Default::default()
            },
        });
        let edit = draw((100, 30), &mut a).0;
        assert!(edit.contains("EDIT BEAD"));
        assert!(edit.contains("Edited title"));
        assert!(edit.contains("Existing description"));
        assert!(edit.contains("[ Save ]"));
        a.modal = Some(crate::app::Modal::EditLoading {
            id: "demo-1".into(),
            seq: 1,
            error: Some("offline".into()),
        });
        let loading_error = draw((100, 30), &mut a).0;
        assert!(loading_error.contains("offline"));
        assert!(loading_error.contains("retry"));
        a.modal = Some(crate::app::Modal::Create(crate::app::CreateForm::default()));
        let tiny = draw((12, 5), &mut a).0;
        assert!(tiny.contains("Resize"));
        assert!(!a.modal_ready);
        a.modal = Some(crate::app::Modal::ConfirmDelete {
            id: "demo-1".into(),
            title: "Title is visible".into(),
        });
        let deletion = draw((100, 30), &mut a).0;
        assert!(deletion.contains("Cannot be undone"));
        assert!(deletion.contains("orphaned"));
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        a.modal = Some(crate::app::Modal::Create(crate::app::CreateForm {
            focus: 2,
            dropdown: Some(2),
            issue_type: 8,
            ..Default::default()
        }));
        terminal
            .draw(|f| render(f, Duration::ZERO, &mut a))
            .unwrap();
        let buf = terminal.backend().buffer();
        let text = buf.content().iter().map(|c| c.symbol()).collect::<String>();
        assert!(text.contains("Milestone"));
        assert!(text.contains("NEW BEAD"));
        assert!(a.modal_ready);
        assert!(buf.content().iter().any(|cell| {
            cell.bg
                == crate::theme::definition(a.theme)
                    .palette
                    .selected_background
        }));
        assert_eq!(buf[(17, 9)].fg, TEAL);
        a.notice = None;
        a.modal = Some(crate::app::Modal::Create(crate::app::CreateForm {
            focus: 5,
            ..Default::default()
        }));
        let normal_80 = draw((80, 24), &mut a).0;
        assert!(normal_80.contains("Cancel"));
        assert!(normal_80.contains("Create"));
        a.modal = Some(crate::app::Modal::ConfirmDelete {
            id: "demo-1".into(),
            title: "Title is visible".into(),
        });
        let delete_80 = draw((80, 24), &mut a).0;
        assert!(delete_80.contains("[y] Delete"));
        assert!(delete_80.contains("orphaned"));
        a.modal = Some(crate::app::Modal::ConfirmDelete {
            id: "long-id".into(),
            title: "A very long title ".repeat(12),
        });
        let long_delete = draw((50, 12), &mut a).0;
        assert!(a.modal_ready);
        assert!(long_delete.contains("long-id"));
        assert!(long_delete.contains("Cannot be undone"));
        assert!(long_delete.contains("Dependency links"));
        assert!(long_delete.contains("No cascade"));
        assert!(long_delete.contains("[y] Delete"));
        assert!(long_delete.contains("…"));
        a.notice = Some("permission denied".into());
        let error_delete = draw((80, 24), &mut a).0;
        assert!(error_delete.contains("Delete error"));
        assert!(error_delete.contains("permission denied"));
        assert!(error_delete.contains("[y] Delete"));
    }
    #[test]
    fn epic_generation_toggle_is_visible_only_when_creating_epic() {
        let mut app = app();
        let mut form = crate::app::CreateForm {
            issue_type: crate::beads::BEAD_TYPES
                .iter()
                .position(|kind| *kind == "epic")
                .unwrap(),
            generate_children: true,
            focus: 7,
            ..Default::default()
        };
        app.modal = Some(crate::app::Modal::Create(form.clone()));
        let epic = draw((100, 30), &mut app).0;
        assert!(epic.contains("[x] Generate child tasks"));
        assert!(epic.contains("Agent harness"));
        assert!(epic.contains("Description"));
        assert!(epic.contains("[ Create ]"));
        form.issue_type = 0;
        app.modal = Some(crate::app::Modal::Create(form.clone()));
        assert!(!draw((100, 30), &mut app).0.contains("Generate child tasks"));
        form.issue_type = crate::beads::BEAD_TYPES
            .iter()
            .position(|kind| *kind == "epic")
            .unwrap();
        app.modal = Some(crate::app::Modal::Edit {
            id: "e1".into(),
            form,
        });
        assert!(!draw((100, 30), &mut app).0.contains("Generate child tasks"));
    }
    #[test]
    fn create_harness_picker_shows_only_discovered_kinds_and_discovery_states() {
        let mut app = app();
        app.modal = Some(crate::app::Modal::Create(crate::app::CreateForm {
            harnesses: vec!["opencode".into()],
            harness: 1,
            focus: 6,
            dropdown: Some(6),
            ..Default::default()
        }));
        let choices = draw((100, 30), &mut app).0;
        assert!(choices.contains("opencode"));
        assert!(choices.contains("Unassigned"));
        assert!(!choices.contains("claude"));
        app.modal = Some(crate::app::Modal::Create(crate::app::CreateForm {
            harness_loading: true,
            ..Default::default()
        }));
        assert!(
            draw((100, 30), &mut app)
                .0
                .contains("Checking Herdr integrations")
        );
        app.modal = Some(crate::app::Modal::Create(crate::app::CreateForm {
            harness_error: Some("Herdr integration discovery failed: offline".into()),
            ..Default::default()
        }));
        let failed = draw((100, 30), &mut app).0;
        assert!(failed.contains("Herdr unavailable"));
        assert!(failed.contains("offline"));
    }

    #[test]
    fn restart_dialog_shows_destructive_warning_and_handoff_id() {
        let mut app = app();
        app.worktrees_view = true;
        app.modal = Some(crate::app::Modal::ConfirmWorktree {
            id: "ot-selected".into(),
            action: 3,
        });
        let rendered = draw((100, 30), &mut app).0;
        assert!(rendered.contains("RESTART TASK?"));
        assert!(rendered.contains("Discard all uncommitted checkout changes."));
        assert!(rendered.contains("Worktree: ot-selected"));
        assert!(rendered.contains("y confirm"));
    }
    #[test]
    fn browser_footer_keeps_action_hints_without_routine_state_or_arrow_tips() {
        for width in [64, 90, 120] {
            let footer = footer_text(width);
            for action in ["n new", "d delete", "h hand off", "r ready", "q quit"] {
                assert!(
                    footer.contains(action),
                    "{action} missing at width {width}: {footer}"
                );
            }
            assert!(Span::raw(footer.as_str()).width() <= width);
            assert!(!footer.contains("↑"));
            assert!(!footer.contains("←"));
            assert!(!footer.contains("READY"));
        }
        assert!(footer_text(120).contains("N child"));
        assert!(footer_text(120).contains("u update"));
        assert!(footer_text(120).contains("Ctrl+? keys"));
        let mut app = app();
        let original = draw((120, 24), &mut app).0;
        app.list_error = Some("current failure".into());
        app.notice = Some("Updated bead".into());
        let with_feedback = draw((120, 24), &mut app).0;
        assert_eq!(
            original.chars().skip(120 * 23).collect::<String>(),
            with_feedback.chars().skip(120 * 23).collect::<String>()
        );
        assert!(with_feedback.contains("LIST ERROR · current failure"));
        app.list_error = None;
        app.detail_error = Some("detail unavailable".into());
        let with_detail_error = draw((120, 24), &mut app).0;
        assert_eq!(
            original.chars().skip(120 * 23).collect::<String>(),
            with_detail_error.chars().skip(120 * 23).collect::<String>()
        );
        assert!(with_detail_error.contains("DETAIL ERROR · detail unavailable"));
        app.detail_error = None;
        let with_notice = draw((120, 24), &mut app).0;
        assert_eq!(
            original.chars().skip(120 * 23).collect::<String>(),
            with_notice.chars().skip(120 * 23).collect::<String>()
        );
        assert!(with_notice.contains("Updated bead"));
    }
    #[test]
    fn outstanding_worktrees_render_target_state_and_pending_dispatch_on_compact_terminal() {
        let mut app = app();
        app.worktrees_view = true;
        app.worktrees.push(crate::worktrees::ManagedWorktree {
            id: "handoff-1".into(),
            bead_id: "closed-1".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: Some("Review handoff".into()),
            bead_description: Some("First line\nSecond line".into()),
            branch: "ontrack/closed-1".into(),
            target_branch: "main".into(),
            workspace_id: "ws-7".into(),
            status: crate::worktrees::WorktreeStatus::MergeRequested,
            detail: Some("waiting for verified ancestry".into()),
        });
        let screen = draw((80, 16), &mut app).0;
        assert!(screen.contains("WORKTREES"));
        assert!(screen.contains("closed-1"));
        assert!(screen.contains("MERGE PENDING"));
        assert!(screen.contains("main"));
        assert!(screen.contains("waiting for verified ancestry"));
        assert!(screen.contains("Git integration"));
        let footer = screen.chars().skip(80 * 15).collect::<String>();
        assert!(footer.contains("Enter switch"));
        assert!(footer.contains("m merge"));
        assert!(footer.contains("c close"));
        assert!(footer.contains("x remove"));
        assert!(!footer.contains("MERGE PENDING"));
        assert!(!footer.contains("waiting for verified ancestry"));
        app.worktrees_error = Some("scan failed".into());
        let with_error = draw((80, 16), &mut app).0;
        let error_footer = with_error.chars().skip(80 * 15).collect::<String>();
        assert_eq!(footer, error_footer);
        assert!(with_error.contains("ERROR · scan failed"));
        app.worktrees_focus_feedback = Some(crate::app::WorktreeFocusFeedback::Success(
            "Switched workspace".into(),
        ));
        app.worktrees_error = None;
        let with_focus = draw((80, 16), &mut app).0;
        assert_eq!(footer, with_focus.chars().skip(80 * 15).collect::<String>());
        assert!(with_focus.contains("Switched workspace"));
        assert_eq!(app.detail_rect.height, 13);
    }
    #[test]
    fn worktree_footer_lists_all_view_actions_on_wide_terminal_and_fits_one_row() {
        let hints = worktree_footer_text(200);
        for key in [
            "Enter switch",
            "m merge",
            "c close",
            "x remove",
            "R restart",
            "r refresh",
            "w back",
            "↑/↓ select",
            "PgUp/PgDn scroll",
            "k/j select",
            "Esc/Backspace back",
            "q quit",
            "Ctrl+C quit",
            "Ctrl+? keys",
        ] {
            assert!(hints.contains(key), "missing {key}: {hints}");
        }
        for width in [24, 64, 80, 120, 200] {
            let footer = worktree_footer_text(width);
            assert!(Span::raw(footer.as_str()).width() <= width);
            assert!(!footer.contains('\n'));
        }
    }
    #[test]
    fn worktree_status_text_uses_theme_colours_in_list_and_detail() {
        use crate::worktrees::{ManagedWorktree, WorktreeStatus};
        assert_eq!(
            crate::app::worktree_status_label(WorktreeStatus::Unmerged),
            "READY TO MERGE"
        );
        let statuses = [
            WorktreeStatus::Reserved,
            WorktreeStatus::InProgress,
            WorktreeStatus::Unmerged,
            WorktreeStatus::MergeQueued,
            WorktreeStatus::MergeRequested,
            WorktreeStatus::NeedsResolution,
            WorktreeStatus::Integrated,
            WorktreeStatus::WorkspaceClosed,
            WorktreeStatus::CheckoutRemoved,
            WorktreeStatus::DirtyCheckout,
            WorktreeStatus::NeedsAttention,
        ];
        let locate = |buffer: &ratatui::buffer::Buffer, rect: Rect, label: &str| {
            let chars = label.chars().collect::<Vec<_>>();
            for y in rect.y..rect.bottom() {
                for x in rect.x..rect.right().saturating_sub(chars.len() as u16) {
                    if chars.iter().enumerate().all(|(offset, ch)| {
                        buffer[(x + offset as u16, y)].symbol() == ch.to_string()
                    }) {
                        return (x, y);
                    }
                }
            }
            panic!("{label} missing from {rect:?}")
        };
        for theme in Theme::ALL {
            let mut app = app();
            app.theme = theme;
            app.worktrees_view = true;
            app.worktrees = statuses
                .iter()
                .enumerate()
                .map(|(index, &status)| ManagedWorktree {
                    id: format!("handoff-{index}"),
                    bead_id: format!("bead-{index}"),
                    parent_bead_id: None,
                    handoff_accepted: true,
                    bead_title: None,
                    bead_description: None,
                    branch: format!("ontrack/bead-{index}"),
                    target_branch: "main".into(),
                    workspace_id: format!("ws-{index}"),
                    status,
                    detail: None,
                })
                .collect();
            let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
            terminal
                .draw(|f| render(f, Duration::from_secs(2), &mut app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            for status in statuses {
                let label = crate::app::worktree_status_label(status);
                let (x, y) = locate(buffer, app.list_rect, label);
                for offset in 0..label.len() as u16 {
                    let cell = &buffer[(x + offset, y)];
                    assert_eq!(cell.fg, crate::theme::worktree_status_color(theme, status));
                    assert_eq!(cell.bg, crate::theme::definition(theme).palette.panel);
                    assert!(cell.modifier.contains(Modifier::BOLD));
                }
            }
            assert_eq!(
                buffer[(app.list_rect.x + 1, app.list_rect.y + 1)].bg,
                crate::theme::definition(theme).palette.selected_background
            );
            for (index, status) in statuses.into_iter().enumerate() {
                app.selected_worktree = index;
                terminal
                    .draw(|f| render(f, Duration::from_secs(2), &mut app))
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let label = crate::app::worktree_status_label(status);
                let (x, y) = locate(buffer, app.detail_rect, label);
                for offset in 0..label.len() as u16 {
                    let cell = &buffer[(x + offset, y)];
                    assert_eq!(cell.fg, crate::theme::worktree_status_color(theme, status));
                    assert_eq!(cell.bg, crate::theme::definition(theme).palette.background);
                    assert!(cell.modifier.contains(Modifier::BOLD));
                }
            }
        }
    }
    #[test]
    fn selected_worktree_shows_its_captured_title_and_multiline_description() {
        let mut app = app();
        app.worktrees_view = true;
        let first = crate::worktrees::ManagedWorktree {
            id: "handoff-1".into(),
            bead_id: "bead-1".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: Some("First task".into()),
            bead_description: Some("Line one\nLine two".into()),
            branch: "ontrack/bead-1".into(),
            target_branch: "main".into(),
            workspace_id: "ws-1".into(),
            status: crate::worktrees::WorktreeStatus::Unmerged,
            detail: None,
        };
        app.worktrees.push(first.clone());
        app.worktrees.push(crate::worktrees::ManagedWorktree {
            id: "handoff-2".into(),
            bead_id: "bead-2".into(),
            parent_bead_id: None,
            bead_title: Some("Second task".into()),
            bead_description: Some("Another description".into()),
            ..first
        });
        let first_screen = draw((100, 23), &mut app).0;
        for value in [
            "First task",
            "Line one",
            "Line two",
            "Branch: ontrack/bead-1",
        ] {
            assert!(first_screen.contains(value), "{value} missing");
        }
        app.selected_worktree = 1;
        let second_screen = draw((100, 23), &mut app).0;
        for value in ["Second task", "Another description"] {
            assert!(second_screen.contains(value), "{value} missing");
        }
        assert!(!second_screen.contains("Line one"));
    }
    #[test]
    fn worktree_description_scrolls_and_missing_context_has_fallback() {
        let mut app = app();
        app.worktrees_view = true;
        app.worktrees.push(crate::worktrees::ManagedWorktree {
            id: "legacy".into(),
            bead_id: "old-bead".into(),
            parent_bead_id: None,
            handoff_accepted: false,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/old-bead".into(),
            target_branch: "main".into(),
            workspace_id: "ws".into(),
            status: crate::worktrees::WorktreeStatus::Unmerged,
            detail: None,
        });
        let legacy = draw((100, 20), &mut app).0;
        assert!(legacy.contains("old-bead"));
        assert!(legacy.contains("not recorded at handoff"));
        app.worktrees[0].bead_description = Some(
            (0..30)
                .map(|index| format!("Detail line {index}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        app.worktree_detail_offset = u16::MAX;
        let scrolled = draw((100, 20), &mut app).0;
        assert!(app.worktree_detail_offset > 0);
        assert!(scrolled.contains("Detail line 29"));
    }
    #[test]
    fn current_bead_text_updates_worktree_details_and_snapshot_survives_bead_removal() {
        let mut app = app();
        app.worktrees_view = true;
        app.worktrees.push(crate::worktrees::ManagedWorktree {
            id: "handoff".into(),
            bead_id: "demo-1".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: Some("Original title".into()),
            bead_description: Some("Original description".into()),
            branch: "ontrack/demo-1".into(),
            target_branch: "main".into(),
            workspace_id: "ws".into(),
            status: crate::worktrees::WorktreeStatus::Unmerged,
            detail: None,
        });
        app.beads[0].title = "Updated title".into();
        app.beads[0].data["description"] = json!("Updated description");
        let current = draw((100, 23), &mut app).0;
        assert!(current.contains("Updated title"));
        assert!(current.contains("Updated description"));
        assert!(!current.contains("Original description"));
        app.beads.clear();
        let fallback = draw((100, 23), &mut app).0;
        assert!(fallback.contains("Original title"));
        assert!(fallback.contains("Original description"));
    }
    #[test]
    fn selected_worktree_and_confirmation_stay_visible_with_long_lists() {
        let mut app = app();
        app.worktrees_view = true;
        for index in 0..12 {
            app.worktrees.push(crate::worktrees::ManagedWorktree {
                id: format!("wt-{index}"),
                bead_id: format!("bead-{index}"),
                parent_bead_id: None,
                handoff_accepted: true,
                bead_title: None,
                bead_description: None,
                branch: format!("ontrack/bead-{index}"),
                target_branch: "main".into(),
                workspace_id: format!("ws-{index}"),
                status: crate::worktrees::WorktreeStatus::Unmerged,
                detail: None,
            });
        }
        app.selected_worktree = 11;
        app.modal = Some(crate::app::Modal::ConfirmWorktree {
            id: "wt-11".into(),
            action: 0,
        });
        let screen = draw((80, 9), &mut app).0;
        assert!(screen.contains("Worktree: wt-11"));
        assert!(screen.contains("Request merge"));
        assert!(screen.contains("recorded agent"));
        assert!(screen.contains("confirm"));
        assert!(
            screen
                .chars()
                .skip(80 * 8)
                .collect::<String>()
                .contains("m merge")
        );
        assert!(app.worktree_offset > 0);
    }
    #[test]
    fn remove_confirmation_names_herdr_closure_and_keeps_consent_visible_compactly() {
        let mut state = app();
        state.worktrees_view = true;
        state.worktrees.push(crate::worktrees::ManagedWorktree {
            id: "wt-remove".into(),
            bead_id: "b-closed".into(),
            parent_bead_id: None,
            handoff_accepted: true,
            bead_title: None,
            bead_description: None,
            branch: "ontrack/b".into(),
            target_branch: "main".into(),
            workspace_id: "ws".into(),
            status: crate::worktrees::WorktreeStatus::Integrated,
            detail: None,
        });
        state.settings.auto_close_workspace = false;
        state.modal = Some(crate::app::Modal::ConfirmWorktree {
            id: "wt-remove".into(),
            action: 2,
        });
        let screen = draw((80, 9), &mut state).0;
        assert!(screen.contains("Remove integrated checkout"));
        assert!(screen.contains("Herdr workspace?"));
        assert!(screen.contains("y confirm · n / Esc cancel"));
        let mut settings = app();
        settings.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings {
                auto_remove_checkout: true,
                ..Default::default()
            },
            error: None,
            dropdown_open: false,
            focus: 3,
        });
        let screen = draw((80, 24), &mut settings).0;
        assert!(screen.contains("Remove checkout + close Herdr workspace"));
    }
    #[test]
    fn wrapping_and_caret_share_unicode_line_map() {
        let (s, x, y) = wrapped_description("abcdefghij\nxy", 12, 4);
        assert_eq!(s, "abcd\nefgh\nij\nxy");
        assert_eq!((x, y), (1, 3));
        let (s, x, y) = wrapped_description("abc\n", 3, 3);
        assert_eq!(s, "abc\n");
        assert_eq!((x, y), (0, 1));
        let (_, x, y) = wrapped_description("abc\n", 4, 3);
        assert_eq!((x, y), (0, 1));
        let (s, x, y) = wrapped_description("ab界x", 5, 3);
        assert_eq!(s, "ab\n界x");
        assert_eq!((x, y), (2, 1));
        let (_, x, y) = wrapped_description("ab界x", 6, 3);
        assert_eq!((x, y), (0, 2));
        let (_, x, y) = wrapped_description("one\ntwo\nthree\nfour", 19, 5);
        let scroll = y.saturating_sub(2);
        assert_eq!(y - scroll, 2);
        assert!(x < 5);
    }
    #[test]
    fn description_wraps_words_and_keeps_cursor_on_the_rendered_line() {
        let (s, x, y) = wrapped_description("hello world", 6, 8);
        assert_eq!(s, "hello \nworld");
        assert_eq!((x, y), (0, 1));
        let (s, x, y) = wrapped_description("hello world", 11, 5);
        assert_eq!(s, "hello\nworld");
        assert_eq!((x, y), (0, 2));
        let (s, x, y) = wrapped_description("ab longword\n界", 7, 4);
        assert_eq!(s, "ab \nlong\nword\n界");
        assert_eq!((x, y), (0, 2));
        let (s, x, y) = wrapped_description("a 界界", 2, 4);
        assert_eq!(s, "a \n界界");
        assert_eq!((x, y), (0, 1));
    }
    #[test]
    fn settings_dialog_and_light_theme_render_with_contrast_at_small_sizes() {
        let mut a = app();
        a.theme = crate::settings::Theme::Light;
        let storage_error = "Could not save settings to '/home/alexander/.config/ontrack/settings.json': Permission denied while replacing the destination file. Check the parent directory permissions and try again.";
        a.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings {
                theme: crate::settings::Theme::Light,
                ..crate::settings::Settings::default()
            },
            error: Some(storage_error.into()),
            dropdown_open: true,
            focus: 0,
        });
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|f| render(f, Duration::from_secs(2), &mut a))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("SETTINGS"));
        assert!(text.contains("Light"));
        assert!(text.contains("› OnTrack Light"));
        assert!(text.contains("Close"));
        assert!(!text.contains("[ Save ]"));
        assert!(!text.contains("Cancel"));
        assert!(text.contains("settings.json"));
        assert!(text.contains("Permission") && text.contains("denied"));
        assert!(text.contains("try again"));
        assert!(text.contains("← / → choice"));
        assert!(text.contains("Dark"));
        assert!(text.contains("Enter / Space select"));
        assert!(text.contains("Esc") && text.contains("close"));
        assert!(a.modal_ready);
        assert_eq!(buffer[(0, 3)].bg, Color::Rgb(246, 248, 247));
        assert!(a.modal_ready);
        a.theme = Theme::Dark;
        a.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings {
                theme: crate::settings::Theme::Dark,
                ..crate::settings::Settings::default()
            },
            error: None,
            dropdown_open: false,
            focus: 0,
        });
        let closed = draw((80, 24), &mut a).0;
        assert!(closed.contains("Dark"));
        assert!(closed.contains("Enter / Space open"));
        a.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings {
                theme: crate::settings::Theme::Light,
                ..crate::settings::Settings::default()
            },
            error: None,
            dropdown_open: true,
            focus: 0,
        });
        assert!(draw((18, 5), &mut a).0.contains("Resize"));
        assert!(!a.modal_ready);
    }
    #[test]
    fn settings_rows_keep_wider_left_titles_and_centered_values() {
        fn locate(buffer: &ratatui::buffer::Buffer, text: &str) -> (u16, u16) {
            let chars: Vec<String> = text.chars().map(|ch| ch.to_string()).collect();
            for y in 0..buffer.area.height {
                for x in 0..buffer.area.width.saturating_sub(chars.len() as u16) {
                    if chars
                        .iter()
                        .enumerate()
                        .all(|(offset, ch)| buffer[(x + offset as u16, y)].symbol() == ch)
                    {
                        return (x, y);
                    }
                }
            }
            panic!("could not find {text:?}");
        }

        let mut state = app();
        state.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings {
                auto_merge: true,
                auto_remove_checkout: true,
                ..Default::default()
            },
            error: None,
            dropdown_open: true,
            focus: 0,
        });
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|f| render(f, Duration::from_secs(2), &mut state))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(settings_columns(60), (40, 20));
        let title_x = locate(buffer, "Theme").0;
        for label in [
            "Agent limit",
            "Auto-merge top-level tasks",
            "Auto-merge child tasks into parent",
            "Close Herdr workspace after integration",
            "Remove checkout + close Herdr workspace",
        ] {
            assert_eq!(locate(buffer, label).0, title_x);
        }
        assert_eq!(title_x, 10);
        assert_eq!(locate(buffer, "OnTrack Dark ▾").0, 53);
        assert_eq!(locate(buffer, "› OnTrack Dark").0, 53);
        for label in [
            "Auto-merge top-level tasks",
            "Auto-merge child tasks into parent",
            "Remove checkout + close Herdr workspace",
        ] {
            let y = locate(buffer, label).1;
            assert_eq!(buffer[(58, y)].symbol(), "[");
            assert_eq!(buffer[(59, y)].symbol(), "x");
        }
        let workspace_y = locate(buffer, "Close Herdr workspace after integration").1;
        assert_eq!(buffer[(58, workspace_y)].symbol(), "[");
        assert_eq!(buffer[(59, workspace_y)].symbol(), " ");

        state.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings::default(),
            error: None,
            dropdown_open: false,
            focus: 2,
        });
        let compact = draw((40, 18), &mut state).0;
        assert!(state.modal_ready);
        assert!(compact.contains("Theme"));
        assert!(compact.contains("Remove checkout + clo…"));
        assert!(compact.contains("[ ]"));
    }
    #[test]
    fn settings_agent_picker_shows_saved_choice_and_scrolls_to_later_kinds() {
        let mut a = app();
        a.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings {
                default_agent: crate::settings::DefaultAgent::Kind("hermes"),
                ..Default::default()
            },
            error: None,
            dropdown_open: true,
            focus: 1,
        });
        let screen = draw((80, 24), &mut a).0;
        assert!(screen.contains("Default agent for new tasks"));
        assert!(screen.contains("› hermes"));
        assert!(screen.contains("↑ earlier agents"));
        assert!(screen.contains("↓ later agents"));
    }
    #[test]
    fn light_create_type_dropdown_uses_catalog_selection_colors() {
        let mut a = app();
        a.theme = Theme::Light;
        a.modal = Some(crate::app::Modal::Create(crate::app::CreateForm {
            focus: 2,
            dropdown: Some(2),
            issue_type: 0,
            ..Default::default()
        }));
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|f| render(f, Duration::ZERO, &mut a))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(buffer.content().iter().any(|cell| {
            cell.bg
                == crate::theme::definition(Theme::Light)
                    .palette
                    .selected_background
        }));
        assert!(!buffer.content().iter().any(|cell| cell.bg == type_color(0)));
        assert!(
            !buffer
                .content()
                .iter()
                .any(|cell| cell.fg == priority_color(2))
        );
    }
    #[test]
    fn markdown_detail_colors_and_inline_code_backgrounds_follow_theme() {
        let mut a = app();
        a.detail = Some(json!({"description":"# Overview\n\nReadable text with `inline code`."}));
        let draw_detail = |app: &mut App| {
            let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
            terminal
                .draw(|frame| render(frame, Duration::from_secs(2), app))
                .unwrap();
            terminal.backend().buffer().clone()
        };
        let inline_code_position = |buffer: &ratatui::buffer::Buffer, rect: Rect| {
            (rect.y + 1..rect.bottom() - 1)
                .find_map(|y| {
                    let row = (rect.x + 1..rect.right() - 1)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>();
                    row.find("inline code")
                        .map(|offset| (rect.x + 1 + row[..offset].chars().count() as u16, y))
                })
                .expect("detail text should contain the inline code row")
        };
        a.theme = Theme::Dark;
        let dark = draw_detail(&mut a);
        let dark_code_position = inline_code_position(&dark, a.detail_rect);
        a.theme = Theme::Light;
        let light = draw_detail(&mut a);
        let light_code_position = inline_code_position(&light, a.detail_rect);
        let dark_palette = crate::theme::definition(Theme::Dark).palette;
        let light_palette = crate::theme::definition(Theme::Light).palette;
        let dark_code = &dark[dark_code_position];
        let light_code = &light[light_code_position];
        assert_eq!(dark_code.fg, dark_palette.warning);
        assert_eq!(light_code.fg, light_palette.warning);
        assert_ne!(dark_code.fg, light_code.fg);
        assert_eq!(dark_code.bg, dark_palette.panel);
        assert_eq!(light_code.bg, light_palette.panel);
        assert_eq!(
            crate::theme::markdown_foreground(Theme::SolarizedLight, Color::Indexed(1)),
            crate::theme::definition(Theme::SolarizedLight)
                .palette
                .danger
        );
    }
    #[test]
    fn every_theme_renders_splash_and_browser_at_normal_and_compact_sizes() {
        for theme in Theme::ALL {
            let palette = crate::theme::definition(theme).palette;
            let mut app = app();
            app.theme = theme;
            for size in [(100, 30), (60, 18)] {
                let rendered = draw(size, &mut app).0;
                assert!(
                    rendered.contains("Title is visible"),
                    "{theme:?} at {size:?}"
                );
                assert!(rendered.contains("ONTRACK"), "{theme:?} at {size:?}");
                app.modal = Some(crate::app::Modal::Settings {
                    draft: crate::settings::Settings {
                        theme,
                        ..crate::settings::Settings::default()
                    },
                    error: None,
                    dropdown_open: true,
                    focus: 0,
                });
                let settings = draw(size, &mut app).0;
                assert!(settings.contains("SETTINGS"), "{theme:?} at {size:?}");
                assert!(
                    app.modal_ready,
                    "settings modal should fit for {theme:?} at {size:?}"
                );
                app.modal = None;
            }
            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|frame| render(frame, Duration::from_secs(2), &mut app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            assert!(
                buffer
                    .content()
                    .iter()
                    .any(|cell| cell.bg == palette.background)
            );
            assert!(
                buffer
                    .content()
                    .iter()
                    .any(|cell| cell.fg == palette.success)
            );
            assert_eq!(
                crate::theme::markdown_foreground(theme, Color::Rgb(220, 228, 229)),
                palette.foreground
            );
        }
    }
    #[test]
    fn theme_dropdown_keeps_late_selection_visible_and_marks_more_options() {
        let mut app = app();
        app.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings {
                theme: crate::settings::Theme::RosePine,
                ..crate::settings::Settings::default()
            },
            error: None,
            dropdown_open: true,
            focus: 0,
        });
        let late = draw((80, 24), &mut app).0;
        assert!(late.contains("Rosé Pine"));
        assert!(late.contains("↑ earlier themes"));
        assert!(!late.contains("OnTrack Dark"));
        assert!(app.modal_ready);

        app.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings {
                theme: crate::settings::Theme::TokyoNight,
                ..crate::settings::Settings::default()
            },
            error: None,
            dropdown_open: true,
            focus: 0,
        });
        let early = draw((80, 24), &mut app).0;
        assert!(early.contains("Tokyo Night"));
        assert!(early.contains("↓ later themes"));
        assert!(early.contains("Close"));
        assert!(!early.contains("Save"));
        assert!(!early.contains("Cancel"));
        assert!(app.modal_ready);
    }
    #[test]
    fn agent_limit_dropdown_shows_unlimited_then_increasing_counts() {
        let mut app = app();
        app.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings::default(),
            error: None,
            dropdown_open: true,
            focus: 2,
        });
        let screen = draw((80, 30), &mut app).0;
        let labels = ["› 5", "  Unlimited", "  1", "  2", "  3", "  4"];
        for label in labels {
            assert!(screen.contains(label), "missing {label:?}");
        }
        assert!(screen.find("Unlimited").unwrap() < screen.find("› 5").unwrap());
        assert!(screen.contains("↓ later limits"));

        app.modal = Some(crate::app::Modal::Settings {
            draft: crate::settings::Settings {
                agent_limit: crate::settings::AgentLimit::Limited(12),
                ..Default::default()
            },
            error: None,
            dropdown_open: true,
            focus: 2,
        });
        let later = draw((80, 24), &mut app).0;
        assert!(later.contains("› 12"));
        assert!(later.contains("↑ earlier limits"));
        assert!(later.contains("↓ later limits"));
    }
    #[test]
    fn wide_title_horizontal_scroll_starts_at_character_boundary() {
        let title = "界".repeat(40);
        let caret = Span::raw(title.as_str()).width();
        let desired = caret.saturating_sub(9);
        let (window, start) = display_window(&title, desired, 10);
        assert_eq!(start % 2, 0);
        assert!(Span::raw(window.as_str()).width() <= 10);
        assert!(caret - start < 10);
        let (_, start) = display_window("a界b", 2, 3);
        assert_eq!(start, 3);
    }
    #[test]
    fn handoff_toast_is_top_right_and_does_not_open_a_dialog() {
        for size in [(80, 24), (30, 9)] {
            let mut app = app();
            app.toast = Some(crate::app::Toast {
                message: "Finding Herdr agent…".into(),
                error: false,
                expires_at: None,
            });
            let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
            terminal
                .draw(|frame| render(frame, Duration::from_secs(2), &mut app))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let top = (0..size.0)
                .map(|x| buffer[(x, 0)].symbol())
                .collect::<String>();
            assert!(
                top.contains("HERDR"),
                "toast must touch the top edge at {size:?}"
            );
            assert!(
                top.find("HERDR").unwrap() > 0,
                "toast must be on the right at {size:?}"
            );
            assert!(app.modal.is_none());
            let screen = buffer
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(screen.contains("Finding"));
        }
    }
    #[test]
    fn expired_handoff_toast_disappears_and_compact_error_stays_readable() {
        let mut app = app();
        app.toast = Some(crate::app::Toast {
            message: "Handoff failed: inspect recorded worktree before retrying".into(),
            error: true,
            expires_at: Some(std::time::Instant::now() + Duration::from_secs(5)),
        });
        let visible = draw((40, 12), &mut app).0;
        assert!(visible.contains("Handoff"));
        assert!(visible.contains("failed"));
        app.toast.as_mut().unwrap().expires_at =
            Some(std::time::Instant::now() - Duration::from_secs(1));
        let expired = draw((40, 12), &mut app).0;
        assert!(!expired.contains("Handoff failed"));
    }
}
