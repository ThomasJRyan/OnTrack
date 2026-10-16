use crate::app::{App, DisplayRow};
use crate::settings::Theme;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
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
fn settings_hint(dropdown_open: bool, focus: usize) -> &'static str {
    if dropdown_open {
        "↑ / ↓ move · ← / → theme · Enter / Space toggle · Tab moves · Esc close"
    } else if focus == 1 {
        "↑ / ↓ move · Enter / Space toggle · Tab moves · Esc close"
    } else {
        "↑ / ↓ move · ← / → theme · Enter / Space toggle · Tab moves"
    }
}
const MAX_VISIBLE_THEMES: usize = 6;
fn visible_theme_window(theme: Theme, available_height: u16) -> (usize, usize) {
    let total = crate::theme::CATALOG.len();
    let visible = usize::from(available_height)
        .saturating_sub(20)
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
        return;
    }
    if app.modal.is_none() && elapsed < Duration::from_millis(1200) {
        let logo_width = ASCII_LOGO
            .iter()
            .map(|line| Span::raw(*line).width())
            .max()
            .unwrap_or(0);
        let large = usize::from(a.width) >= logo_width && a.height >= ASCII_LOGO.len() as u16;
        let lines = if large {
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
                Constraint::Length(3),
            ])
            .split(a);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " WORKTREES ",
                    Style::default()
                        .fg(palette.selected_foreground)
                        .bg(palette.selected_background)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  ONTRACK  /  Git integration, not Bead status"),
            ]))
            .style(Style::default().fg(ink)),
            areas[0],
        );
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(areas[1]);
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
                "No OnTrack-tracked handoffs."
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
            rows.push(Line::from(Span::styled(
                truncate_display(
                    &format!(
                        "{} {} · {}",
                        if selected { "›" } else { " " },
                        item.bead_id,
                        crate::app::worktree_status_label(item.status)
                    ),
                    panes[0].width.saturating_sub(2) as usize,
                ),
                style,
            )));
            rows.push(Line::from(Span::styled(
                truncate_display(
                    &format!("  {} → {}", item.branch, item.target_branch),
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
            let mut lines = vec![
                Line::from(Span::styled(
                    format!(
                        "{} · {}",
                        item.bead_id,
                        crate::app::worktree_status_label(item.status)
                    ),
                    Style::default().fg(teal).add_modifier(Modifier::BOLD),
                )),
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
            lines
        } else {
            vec![Line::from("Select a tracked handoff to inspect its state.")]
        };
        f.render_widget(
            Paragraph::new(detail)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" HANDOFF DETAILS ")
                        .border_style(Style::default().fg(palette.border)),
                )
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(ink).bg(bg)),
            panes[1],
        );
        let footer = if let Some(crate::app::Modal::ConfirmWorktree { id, action }) = &app.modal {
            let operation = match action {
                0 => "MERGE REQUEST · dispatch only; not integrated",
                1 => "CLOSE WORKSPACE · integrated only",
                _ => "REMOVE CHECKOUT AND CLOSE HERDR WORKSPACE · integrated only; branch kept",
            };
            format!("y confirm / Esc cancel · {operation} · {id}")
        } else if let Some(error) = &app.worktrees_error {
            format!("ERROR · {error} · w back")
        } else if let Some(item) = selected.filter(|item| item.detail.is_some()) {
            format!(
                "{} · {} · r refresh",
                crate::app::worktree_status_label(item.status),
                item.detail.as_deref().unwrap_or_default()
            )
        } else {
            "↑/↓ select · m request merge · c close · x remove · r refresh · w back".to_owned()
        };
        let footer_lines = wrap_words(&footer, areas[2].width as usize)
            .into_iter()
            .take(usize::from(areas[2].height))
            .collect::<Vec<_>>();
        f.render_widget(
            Paragraph::new(footer_lines.join("\n")).style(
                Style::default()
                    .fg(if app.worktrees_error.is_some() || app.modal.is_some() {
                        gold
                    } else {
                        muted
                    })
                    .bg(palette.panel_alt),
            ),
            areas[2],
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
                    .fg(palette.selected_foreground)
                    .bg(palette.selected_background)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  ONTRACK / ISSUES · TASKS {}/{} ACTIVE (t)",
                app.tasks.len(),
                app.tasks.iter().filter(|task| task.active).count()
            )),
        ]))
        .style(Style::default().fg(ink)),
        v[0],
    );
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(v[1]);
    app.list_rect = split[0];
    app.detail_rect = split[1];
    let layout = crate::app::list_layout(&app.beads, &app.collapsed);
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
                    let status = if bead.status.trim().is_empty() {
                        "unknown"
                    } else {
                        bead.status.as_str()
                    };
                    Line::from(vec![
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
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("  "),
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
    let selected_metadata = selected.map(|b| {
        (
            b.id.as_str(),
            b.title.as_str(),
            b.status.as_str(),
            b.priority as i64,
        )
    });
    let mut detail_text = detail
        .map(|v| crate::detail::render(v, selected_metadata, inner_width))
        .unwrap_or_else(|| {
            ratatui::text::Text::from(if app.beads.is_empty() {
                "No issues found in this workspace.\n\nPress r to refresh."
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
        .map(|b| format!(" {} · {} ", b.id, b.status))
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
    let state = if let Some(e) = &app.list_error {
        format!("LIST ERROR · {e}")
    } else if let Some(e) = &app.detail_error {
        format!("DETAIL ERROR · {e}")
    } else if app.list_loading {
        "REFRESHING ISSUES…".into()
    } else if app.detail_loading {
        "LOADING DETAILS…".into()
    } else if let Some(notice) = &app.notice {
        notice.clone()
    } else {
        "READY".into()
    };
    f.render_widget(
        Paragraph::new(footer_text(&state, v[2].width as usize))
            .style(Style::default().fg(muted).bg(palette.panel_alt)),
        v[2],
    );
    if let Some(modal) = &app.modal {
        let width = match modal {
            crate::app::Modal::Create(_) => 70,
            crate::app::Modal::Settings { .. } => 64,
            crate::app::Modal::Launch(_) => 58,
            crate::app::Modal::Tasks { .. } => 72,
            crate::app::Modal::ReleaseTask { .. } => 68,
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
                    let (start, count) = visible_theme_window(draft.theme, a.height);
                    count
                        + usize::from(start > 0)
                        + usize::from(start + count < crate::theme::CATALOG.len())
                } else {
                    0
                };
                (9 + option_lines + error_lines + hint_lines + 2) as u16
            }
            crate::app::Modal::Create(form) => {
                if form.dropdown.is_some() {
                    19
                } else {
                    15
                }
            }
            crate::app::Modal::ConfirmDelete { .. } => delete_lines
                .as_ref()
                .map_or(10, |lines| (lines.len() + 2).max(10) as u16),
            crate::app::Modal::Launch(launch) => launch_dialog_lines(
                launch,
                app.notice.as_deref(),
                app.mutation_busy,
                palette,
                width.saturating_sub(4) as usize,
            )
            .len()
            .saturating_add(3)
            .min(usize::from(a.height.saturating_sub(2)))
            .max(7) as u16,
            crate::app::Modal::Tasks { .. } => (app.tasks.len().max(1) * 5 + 5)
                .min(usize::from(a.height.saturating_sub(2)))
                .max(7) as u16,
            crate::app::Modal::ReleaseTask { .. } => 12,
            crate::app::Modal::ConfirmWorktree { .. } => 8,
        };
        let min_width = if matches!(modal, crate::app::Modal::Create(_)) {
            70
        } else if matches!(
            modal,
            crate::app::Modal::Settings { .. }
                | crate::app::Modal::Launch(_)
                | crate::app::Modal::Tasks { .. }
        ) {
            38
        } else {
            50
        };
        if a.width < min_width || a.height < height {
            app.modal_ready = false;
            f.render_widget(Clear, a);
            let escape_hint = if matches!(
                modal,
                crate::app::Modal::Settings { .. } | crate::app::Modal::Tasks { .. }
            ) {
                "Esc closes"
            } else {
                "Esc cancels"
            };
            let prompt = if matches!(modal, crate::app::Modal::Launch(_)) && a.width < min_width {
                format!("Widen to {min_width} · Esc cancels")
            } else if matches!(modal, crate::app::Modal::Launch(_)) {
                format!("Need {height} rows · Esc cancels")
            } else if a.width < min_width {
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
                crate::app::Modal::Create(_) => " NEW BEAD ",
                crate::app::Modal::Settings { .. } => " SETTINGS ",
                crate::app::Modal::Launch(_) => " HAND OFF BEAD ",
                crate::app::Modal::Tasks { .. } => " BACKGROUND TASKS ",
                crate::app::Modal::ReleaseTask { .. } => " RELEASE LOCAL PROTECTION? ",
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
            crate::app::Modal::Settings {
                draft,
                error,
                dropdown_open,
                focus,
            } => {
                let mut lines = vec![
                    Line::from("Tracked worktree cleanup preferences"),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("Theme  [ {}  ▾ ]", draft.theme.label()),
                        if *focus == 0 {
                            Style::default()
                                .fg(palette.selected_foreground)
                                .bg(palette.selected_background)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(teal).add_modifier(Modifier::BOLD)
                        },
                    )),
                ];
                if *dropdown_open {
                    let (start, count) = visible_theme_window(draft.theme, a.height);
                    let end = start + count;
                    if start > 0 {
                        lines.push(
                            Line::from("  ↑ earlier themes").style(Style::default().fg(muted)),
                        );
                    }
                    for theme in Theme::ALL.iter().skip(start).take(count).copied() {
                        let selected = draft.theme == theme;
                        lines.push(Line::from(Span::styled(
                            format!("  {} {}", if selected { "›" } else { " " }, theme.label()),
                            if selected {
                                Style::default()
                                    .fg(palette.selected_foreground)
                                    .bg(palette.selected_background)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(ink)
                            },
                        )));
                    }
                    if end < crate::theme::CATALOG.len() {
                        lines
                            .push(Line::from("  ↓ later themes").style(Style::default().fg(muted)));
                    }
                }
                for (index, (label, enabled)) in [
                    ("Auto-request merge", draft.auto_merge),
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
                    let focused = *focus == index + 1;
                    lines.push(Line::from(Span::styled(
                        format!(
                            "{} [{}] {label}",
                            if focused { "›" } else { " " },
                            if enabled { "x" } else { " " }
                        ),
                        if focused {
                            Style::default()
                                .fg(palette.selected_foreground)
                                .bg(palette.selected_background)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(ink)
                        },
                    )));
                }
                lines.push(Line::from(Span::styled(
                    "Removing a checkout also closes its Herdr workspace.",
                    Style::default().fg(muted),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "[ Close ]",
                    if *focus == 4 {
                        Style::default()
                            .fg(palette.selected_foreground)
                            .bg(palette.selected_background)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(teal).add_modifier(Modifier::BOLD)
                    },
                )));
                if let Some(error) = error {
                    lines.extend(
                        wrap_words(error, inner.width as usize)
                            .into_iter()
                            .map(|line| {
                                Line::from(Span::styled(line, Style::default().fg(palette.danger)))
                            }),
                    );
                }
                lines.extend(
                    wrap_words(settings_hint(*dropdown_open, *focus), inner.width as usize)
                        .into_iter()
                        .map(|line| Line::from(line).style(Style::default().fg(muted))),
                );
                f.render_widget(
                    Paragraph::new(lines)
                        .alignment(Alignment::Center)
                        .wrap(Wrap { trim: false })
                        .style(Style::default().fg(ink)),
                    inner,
                );
            }
            crate::app::Modal::Create(form) => {
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Length(if form.dropdown.is_some() { 7 } else { 3 }),
                        Constraint::Min(3),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ])
                    .split(inner);
                let title_area = Rect::new(rows[0].x, rows[0].y, rows[0].width, 3);
                f.render_widget(
                    Paragraph::new("")
                        .block(
                            Block::default()
                                .title(" Title ")
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
                let desc_area = rows[2];
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
                                .title(" Description · Enter for newline ")
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
                let actions = Paragraph::new(Line::from(vec![
                    Span::styled(
                        "[ Create ]",
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
                f.render_widget(actions, rows[3]);
                let feedback = if app.mutation_busy {
                    Some("Creating… · form locked until the command finishes")
                } else {
                    app.notice.as_deref()
                };
                let feedback = truncate_display(
                    feedback.unwrap_or(
                        "Tab moves · ←/→ cycles focused choices · Enter opens · Esc cancels",
                    ),
                    rows[4].width as usize,
                );
                f.render_widget(
                    Paragraph::new(feedback).style(Style::default().fg(
                        if app.notice.is_some() || app.mutation_busy {
                            gold
                        } else {
                            muted
                        },
                    )),
                    rows[4],
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
            crate::app::Modal::Launch(launch) => {
                let lines = launch_dialog_lines(
                    launch,
                    app.notice.as_deref(),
                    app.mutation_busy,
                    palette,
                    inner.width as usize,
                );
                let viewport = inner.height.saturating_sub(1);
                let max_scroll = lines.len().saturating_sub(usize::from(viewport));
                let mut offset = usize::from(launch.scroll).min(max_scroll);
                if !launch.finished
                    && !app.mutation_busy
                    && let Some(selected_line) = launch_selected_line(launch, inner.width as usize)
                {
                    if selected_line < offset {
                        offset = selected_line;
                    } else if selected_line >= offset + usize::from(viewport) {
                        offset = selected_line + 1 - usize::from(viewport);
                    }
                    offset = offset.min(max_scroll);
                }
                let hint = if inner.width < 46 && launch.finished {
                    "PgUp/Dn · Esc close"
                } else if inner.width < 46 && launch.error.is_some() {
                    "PgUp/Dn · h retry · Esc"
                } else if inner.width < 46 {
                    "PgUp/Dn · ↑/↓ · Enter · Esc"
                } else if launch.finished {
                    "PgUp/PgDn scroll · Esc close"
                } else if launch.error.is_some() {
                    "PgUp/PgDn scroll · h rediscover · Esc close"
                } else if inner.width < 64 {
                    "PgUp/Dn scroll · ↑/↓ agents · Enter · Esc"
                } else {
                    "PgUp/PgDn scroll · ↑/↓ agents · Enter launch · Esc cancel"
                };
                let above = offset > 0;
                let below = offset + usize::from(viewport) < lines.len();
                let scroll_mark = match (above, below) {
                    (true, true) if inner.width < 64 => "↕ ",
                    (true, false) if inner.width < 64 => "↑ ",
                    (false, true) if inner.width < 64 => "↓ ",
                    (true, true) => "↑↓ more · ",
                    (true, false) => "↑ more · ",
                    (false, true) => "↓ more · ",
                    (false, false) => "",
                };
                let content = Rect::new(inner.x, inner.y, inner.width, viewport);
                f.render_widget(
                    Paragraph::new(lines)
                        .scroll((offset.min(u16::MAX as usize) as u16, 0))
                        .style(Style::default().fg(ink)),
                    content,
                );
                f.render_widget(
                    Paragraph::new(format!("{scroll_mark}{hint}"))
                        .style(Style::default().fg(muted)),
                    Rect::new(inner.x, inner.y + viewport, inner.width, 1),
                );
            }
            crate::app::Modal::Tasks { scroll } => {
                let mut lines = Vec::new();
                if app.tasks.is_empty() {
                    lines.push(Line::from("No handoffs in this session."));
                    lines.push(Line::from("Choose a bead and press h to start one."));
                } else {
                    for task in &app.tasks {
                        let state = if task.active {
                            "ACTIVE"
                        } else {
                            "ENDED · verify"
                        };
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("{}  ", state),
                                Style::default()
                                    .fg(if task.active { teal } else { muted })
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!("{} · {}", task.bead_id, task.title),
                                Style::default().fg(ink).add_modifier(Modifier::BOLD),
                            ),
                        ]));
                        lines.push(Line::from(format!(
                            "Agent: {} · {}",
                            task.agent, task.status
                        )));
                        for value in [&task.detail] {
                            if !value.is_empty() {
                                push_wrapped(&mut lines, value, inner.width as usize);
                            }
                        }
                        lines.push(Line::from(""));
                    }
                }
                lines.push(Line::from(
                    "Session-local only · handoff is not completion proof.",
                ));
                let visible = inner.height.saturating_sub(1);
                let max_scroll = lines.len().saturating_sub(usize::from(visible));
                let offset = usize::from(*scroll).min(max_scroll);
                f.render_widget(
                    Paragraph::new(lines)
                        .scroll((offset as u16, 0))
                        .style(Style::default().fg(ink)),
                    Rect::new(inner.x, inner.y, inner.width, visible),
                );
                f.render_widget(
                    Paragraph::new("↑/↓ · PgUp/PgDn scroll · Esc close")
                        .style(Style::default().fg(muted)),
                    Rect::new(inner.x, inner.y + visible, inner.width, 1),
                );
            }
            crate::app::Modal::ReleaseTask { bead_id, .. } => {
                let warning = format!(
                    "Bead: {bead_id}\nInspect Herdr and Git worktrees first.\nRelease clears only this session's duplicate guard.\nDoes not stop remote agents.\nDoes not delete worktrees or launch anything.\nReleasing active remote work can duplicate it.\n[y] Release protection   [n / Enter / Esc] Cancel"
                );
                f.render_widget(
                    Paragraph::new(warning)
                        .wrap(Wrap { trim: false })
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
                    _ => {
                        "Remove integrated checkout and close its Herdr workspace? Branch is preserved."
                    }
                };
                f.render_widget(
                    Paragraph::new(format!(
                        "{action}\n\nWorktree: {id}\n\ny confirm · n / Esc cancel"
                    ))
                    .style(Style::default().fg(ink)),
                    inner,
                );
            }
        }
    }
}
fn footer_text(state: &str, width: usize) -> String {
    // Keep the actions people need most before optional navigation hints so a
    // long status/error cannot push create, delete, or handoff offscreen. At
    // narrow widths, spend the remaining cells on the start of the status and
    // mark its truncation rather than letting Ratatui clip the actions.
    let actions = " · n new · d delete · h hand off · t tasks · r refresh · q quit";
    let state_width = width.saturating_sub(1 + Span::raw(actions).width());
    let state = truncate_display(state, state_width);
    let core = if state.is_empty() {
        actions.trim_start().to_owned()
    } else {
        format!(" {state}{actions}")
    };
    // Keep pane switching and worktree access discoverable before secondary
    // navigation; add each hint only when it fits rather than dropping the
    // whole navigation cluster at the first width overflow.
    let mut footer = core;
    for hint in [
        " · Tab panes",
        " · w worktrees",
        " · ↑↓ · ←/→ groups",
        " · s settings",
    ] {
        if Span::raw(footer.as_str()).width() + Span::raw(hint).width() <= width {
            footer.push_str(hint);
        }
    }
    footer
}
fn launch_dialog_lines(
    launch: &crate::app::LaunchModal,
    notice: Option<&str>,
    busy: bool,
    palette: crate::theme::Palette,
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    push_wrapped(
        &mut lines,
        &format!("Bead: {} · {}", launch.bead.id, launch.bead.title),
        width,
    );
    if let Some(context) = &launch.context {
        push_wrapped(
            &mut lines,
            &format!("Target workspace: {}", context.workspace.id),
            width,
        );
    } else {
        push_wrapped(&mut lines, "Target workspace: Discovering…", width);
    }
    lines.push(Line::from(""));
    if launch.finished {
        push_wrapped(&mut lines, "Launch finished.", width);
        push_wrapped(&mut lines, "Use Esc to close this result.", width);
    } else if let Some(error) = &launch.error {
        push_wrapped(&mut lines, &format!("Discovery failed: {error}"), width);
        push_wrapped(&mut lines, "Esc closes · h to try discovery again", width);
    } else if busy {
        push_wrapped(&mut lines, "Starting handoff in background…", width);
    } else if let Some(context) = &launch.context {
        if context.agents.is_empty() {
            push_wrapped(
                &mut lines,
                "No agents are available in this workspace.",
                width,
            );
        } else {
            for (index, agent) in context.agents.iter().enumerate() {
                let prefix = if index == launch.selected_agent {
                    "› "
                } else {
                    "  "
                };
                for (part, text) in wrap_words(&agent.label, width.saturating_sub(2))
                    .into_iter()
                    .enumerate()
                {
                    let style = if index == launch.selected_agent {
                        Style::default()
                            .fg(palette.selected_foreground)
                            .bg(palette.selected_background)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    lines.push(Line::from(Span::styled(
                        format!("{}{}", if part == 0 { prefix } else { "  " }, text),
                        style,
                    )));
                }
            }
        }
        push_wrapped(
            &mut lines,
            "↑/↓ choose · Enter launch / confirm · Esc cancel",
            width,
        );
    } else {
        push_wrapped(&mut lines, "Finding available agents… · Esc cancels", width);
    }
    if let Some(notice) = notice {
        push_wrapped(&mut lines, notice, width);
    }
    lines
}
fn launch_selected_line(launch: &crate::app::LaunchModal, width: usize) -> Option<usize> {
    let context = launch.context.as_ref()?;
    if context.agents.is_empty() || launch.selected_agent >= context.agents.len() {
        return None;
    }
    let width = width.max(1);
    let mut line = wrap_words(
        &format!("Bead: {} · {}", launch.bead.id, launch.bead.title),
        width,
    )
    .len();
    line += wrap_words(
        &format!("Target workspace: {}", context.workspace.id),
        width,
    )
    .len()
        + 1;
    for agent in context.agents.iter().take(launch.selected_agent) {
        line += wrap_words(&agent.label, width.saturating_sub(2)).len();
    }
    Some(line)
}
fn push_wrapped(lines: &mut Vec<Line<'static>>, text: &str, width: usize) {
    lines.extend(wrap_words(text, width).into_iter().map(Line::from));
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
fn wrapped_description(s: &str, cursor: usize, width: usize) -> (String, usize, usize) {
    let width = width.max(1);
    let cursor = cursor.min(s.len());
    let mut caret = None;
    let mut out = String::new();
    let mut row = 0;
    let mut col = 0;
    for (byte, ch) in s.char_indices() {
        if byte == cursor {
            caret = Some(if col == width {
                (0, row + 1)
            } else {
                (col, row)
            });
        }
        if ch == '\n' {
            out.push('\n');
            row += 1;
            col = 0;
            continue;
        }
        let cw = Span::raw(ch.to_string()).width();
        if col == width || col + cw > width {
            out.push('\n');
            row += 1;
            col = 0;
        }
        out.push(ch);
        col += cw;
    }
    if cursor == s.len() {
        caret = Some(if col == width {
            (0, row + 1)
        } else {
            (col, row)
        });
    }
    let (x, y) = caret.unwrap_or((col, row));
    (out, x, y)
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
            launch_seq: 0,
            list_rect: Rect::default(),
            detail_rect: Rect::default(),
            modal: None,
            settings: crate::settings::Settings::default(),
            theme: crate::settings::Theme::Dark,
            settings_path: None,
            settings_load_error: None,
            worktrees: Vec::new(),
            selected_worktree: 0,
            worktree_offset: 0,
            worktrees_view: false,
            worktrees_loading: false,
            worktrees_error: None,
            worktrees_inflight: false,
            worktrees_discovered: true,
            worktree_action_pending: false,
            worktrees_next_refresh: std::time::Instant::now(),
            notice: None,
            mutation_busy: false,
            tasks: vec![],
            task_states: std::collections::HashMap::new(),
            modal_ready: true,
            preferred_id: None,
            refresh_inflight: false,
            refresh_pending: false,
            next_refresh: std::time::Instant::now(),
            requests: tx,
            responses,
        }
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
    #[test]
    fn renders_exact_two_line_issue_with_status_and_full_nested_data() {
        let mut a = app();
        let (s, _) = draw((120, 40), &mut a);
        assert!(s.contains("task     P1  closed"));
        assert!(!s.contains("demo-1  P1"));
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
        assert!(status.contains("n new"));
        assert!(status.contains("d delete"));
        assert!(status.contains("h hand off"));
        a.list_error = None;
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
    fn footer_keeps_mutation_and_handoff_shortcuts_ahead_of_optional_navigation() {
        for width in [64, 90, 120] {
            let footer = footer_text("LIST ERROR · current failure", width);
            for action in [
                "n new",
                "d delete",
                "h hand off",
                "t tasks",
                "r refresh",
                "q quit",
            ] {
                assert!(
                    footer.contains(action),
                    "{action} missing at width {width}: {footer}"
                );
            }
            assert!(Span::raw(footer.as_str()).width() <= width);
        }
        let wide = footer_text("READY", 120);
        assert!(wide.contains("Tab panes"));
        assert!(wide.contains("w worktrees"));
        let medium = footer_text("READY", 90);
        assert!(medium.contains("Tab panes"));
    }
    #[test]
    fn outstanding_worktrees_render_target_state_and_pending_dispatch_on_compact_terminal() {
        let mut app = app();
        app.worktrees_view = true;
        app.worktrees.push(crate::worktrees::ManagedWorktree {
            id: "handoff-1".into(),
            bead_id: "closed-1".into(),
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
    }
    #[test]
    fn selected_worktree_and_confirmation_stay_visible_with_long_lists() {
        let mut app = app();
        app.worktrees_view = true;
        for index in 0..12 {
            app.worktrees.push(crate::worktrees::ManagedWorktree {
                id: format!("wt-{index}"),
                bead_id: format!("bead-{index}"),
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
        assert!(screen.contains("bead-11"));
        assert!(screen.contains("MERGE REQUEST"));
        assert!(screen.contains("confirm"));
        assert!(app.worktree_offset > 0);
    }
    #[test]
    fn remove_confirmation_names_herdr_closure_and_keeps_consent_visible_compactly() {
        let mut state = app();
        state.worktrees_view = true;
        state.worktrees.push(crate::worktrees::ManagedWorktree {
            id: "wt-remove".into(),
            bead_id: "b-closed".into(),
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
        assert!(screen.contains("REMOVE CHECKOUT AND CLOSE HERDR WORKSPACE"));
        assert!(screen.contains("y confirm / Esc cancel"));
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
        assert!(text.contains("← / → theme"));
        assert!(text.contains("Dark"));
        assert!(text.contains("Enter / Space toggle"));
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
        assert!(closed.contains("Enter / Space toggle"));
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
    fn launch_modal() -> crate::app::Modal {
        crate::app::Modal::Launch(crate::app::LaunchModal {
            bead: Bead {
                id: "task-42".into(),
                title: "Investigate the launch flow".into(),
                status: "open".into(),
                priority: 2,
                data: json!({}),
            },
            seq: 7,
            context: Some(crate::herdr::LaunchContext {
                workspace: crate::herdr::Workspace {
                    id: "workspace-abc-123".into(),
                    label: "workspace-abc-123".into(),
                },
                agents: vec![crate::herdr::Agent {
                    id: "codex".into(),
                    label: "Review agent".into(),
                }],
            }),
            selected_agent: 0,
            error: None,
            scroll: 0,
            finished: false,
        })
    }
    #[test]
    fn launch_dialog_renders_target_identity_and_confirmation_at_normal_size() {
        let mut app = app();
        app.modal = Some(launch_modal());
        let text = draw((100, 30), &mut app).0;
        assert!(app.modal_ready);
        for value in [
            "HAND OFF BEAD",
            "task-42",
            "workspace-abc-123",
            "Review agent",
            "Enter launch",
        ] {
            assert!(text.contains(value), "missing {value}");
        }
        assert!(
            text.contains("h hand off"),
            "handoff shortcut belongs in the browser footer"
        );
    }
    #[test]
    fn task_summary_and_history_render_independent_states_errors_and_compact_scroll() {
        let mut app = app();
        app.tasks = vec![
            crate::app::BackgroundTask { seq: 1, bead_id: "bead-a".into(), title: "Investigate cache".into(), agent: "codex".into(), status: "running".into(), detail: "Workspace: /tmp/a · branch: ontrack/a · pane: pane-1".into(), active: true },
            crate::app::BackgroundTask { seq: 2, bead_id: "bead-b".into(), title: "Blocked follow-up".into(), agent: "opencode".into(), status: "blocked".into(), detail: "Polling error: connection refused; status unknown. Inspect worktree before retry.".into(), active: true },
            crate::app::BackgroundTask { seq: 3, bead_id: "bead-c".into(), title: "Ended handoff".into(), agent: "codex".into(), status: "done (unverified)".into(), detail: "Verify the bead; handoff is not completion proof.".into(), active: false },
        ];
        let summary = draw((72, 18), &mut app).0;
        assert!(summary.contains("TASKS 3") && summary.contains("2 ACTIVE"));
        assert!(summary.contains("t tasks"));
        app.modal = Some(crate::app::Modal::Tasks { scroll: 0 });
        let list = draw((72, 18), &mut app).0;
        assert!(list.contains("BACKGROUND TASKS"));
        assert!(list.contains("bead-a") && list.contains("running"));
        assert!(list.contains("bead-b") && list.contains("blocked"));
        assert!(list.contains("Polling error"));
        if let Some(crate::app::Modal::Tasks { scroll }) = app.modal.as_mut() {
            *scroll = 5;
        }
        let scrolled = draw((72, 18), &mut app).0;
        assert!(scrolled.contains("bead-c"));
        assert!(scrolled.contains("not completion proof"));
        let tiny = draw((52, 14), &mut app).0;
        assert!(tiny.contains("bead-c"));
        assert!(
            tiny.contains("(t)"),
            "task-list shortcut remains visible on compact screens"
        );
    }
    #[test]
    fn uncertain_handoff_release_requires_explicit_confirmation_and_warns_in_compact_layout() {
        let mut app = app();
        app.modal = Some(crate::app::Modal::ReleaseTask {
            seq: 9,
            bead_id: "bead-uncertain".into(),
        });
        let normal = draw((90, 24), &mut app).0;
        for text in [
            "RELEASE LOCAL PROTECTION",
            "bead-uncertain",
            "Inspect Herdr and Git worktrees first",
            "Does not stop",
            "remote agents",
            "delete worktrees",
            "launch anything",
            "[y] Release protection",
            "[n / Enter / Esc] Cancel",
        ] {
            assert!(normal.contains(text), "missing confirmation detail: {text}");
        }
        let compact = draw((52, 14), &mut app).0;
        assert!(
            app.modal_ready,
            "release warning should fit compact terminal"
        );
        assert!(compact.contains("Inspect Herdr"));
        assert!(compact.contains("[y] Release protection"));
        assert!(compact.contains("[n / Enter / Esc]"));
        assert!(compact.contains("Cancel"));
    }
    #[test]
    fn every_discovered_agent_is_rendered_and_selected_choice_scrolls_into_view() {
        let crate::app::Modal::Launch(mut launch) = launch_modal() else {
            unreachable!()
        };
        launch.context.as_mut().unwrap().agents = (0..14)
            .map(|i| crate::herdr::Agent {
                id: format!("agent-{i:02}"),
                label: format!("agent-{i:02}"),
            })
            .collect();
        launch.selected_agent = 12;
        let mut app = app();
        app.modal = Some(crate::app::Modal::Launch(launch));
        let rendered = draw((100, 16), &mut app).0;
        assert!(app.modal_ready);
        assert!(
            rendered.contains("agent-12"),
            "selected agent must remain visible"
        );
        assert!(rendered.contains("agent-13") || rendered.contains("agent-11"));
        let crate::app::Modal::Launch(mut launch) = launch_modal() else {
            unreachable!()
        };
        launch.context.as_mut().unwrap().agents = (0..14)
            .map(|i| crate::herdr::Agent {
                id: format!("agent-{i:02}"),
                label: format!("agent-{i:02}"),
            })
            .collect();
        assert!(
            launch_dialog_lines(
                &launch,
                None,
                false,
                crate::theme::definition(Theme::Dark).palette,
                46
            )
            .iter()
            .any(|line| line.to_string().contains("agent-13"))
        );
    }
    #[test]
    fn launch_dialog_has_actionable_compact_fallback_and_task_view_keeps_recovery_detail() {
        let mut app = app();
        app.modal = Some(launch_modal());
        let compact = draw((30, 14), &mut app).0;
        assert!(!app.modal_ready);
        assert!(compact.contains("Widen to 38"));
        assert!(compact.contains("Esc cancels"));

        app.modal = Some(crate::app::Modal::Launch(crate::app::LaunchModal {
            context: None,
            error: Some("Workspace discovery failed: the configured workspace could not be found; check the selected project directory and retry discovery with h.".into()),
            ..match launch_modal() { crate::app::Modal::Launch(m) => m, _ => unreachable!() }
        }));
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|frame| render(frame, Duration::from_secs(2), &mut app))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(app.modal_ready);
        for value in [
            "Workspace discovery failed",
            "project",
            "retry",
            "Esc closes",
        ] {
            assert!(
                text.contains(value),
                "missing readable recovery text: {value}"
            );
        }

        app.tasks.push(crate::app::BackgroundTask {
            seq: 7,
            bead_id: "task-42".into(),
            title: "Investigate the launch flow".into(),
            agent: "codex".into(),
            status: "unknown".into(),
            detail: "Agent started in the target worktree, but prompt dispatch failed after partial setup; inspect the worktree and retry only after confirming its state. workspace w-1 · pane p-1 · branch ontrack/task-42".into(),
            active: true,
        });
        app.modal = Some(crate::app::Modal::Tasks { scroll: 0 });
        terminal
            .draw(|frame| render(frame, Duration::from_secs(2), &mut app))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(app.modal_ready);
        for value in ["task-42", "partial setup"] {
            assert!(
                text.contains(value),
                "missing task recovery detail: {value}"
            );
        }
        app.modal = Some(crate::app::Modal::Tasks { scroll: u16::MAX });
        let tail = draw((100, 30), &mut app).0;
        for value in ["confirming its state", "workspace w-1", "ontrack/task-42"] {
            assert!(
                tail.contains(value),
                "missing scrolled recovery detail: {value}"
            );
        }
    }
    #[test]
    fn long_finished_launch_recovery_can_be_scrolled_to_its_tail_at_normal_and_narrow_sizes() {
        for size in [(80, 24), (40, 12)] {
            let mut app = app();
            let crate::app::Modal::Launch(mut launch) = launch_modal() else {
                unreachable!()
            };
            launch.finished = true;
            app.modal = Some(crate::app::Modal::Launch(launch));
            app.notice = Some(format!(
                "{}TAIL_RECOVERY_SENTINEL",
                "The worktree is only partly ready; inspect setup before retrying. ".repeat(45)
            ));
            let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
            terminal
                .draw(|frame| render(frame, Duration::from_secs(2), &mut app))
                .unwrap();
            assert!(
                app.modal_ready,
                "launch result should fit a scrollable dialog at {size:?}"
            );
            let initial = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(
                initial.contains("PgUp"),
                "scroll control missing at {size:?}"
            );
            assert!(
                !initial.contains("TAIL_RECOVERY_SENTINEL"),
                "fixture should require scrolling"
            );
            if let Some(crate::app::Modal::Launch(modal)) = app.modal.as_mut() {
                modal.scroll = u16::MAX;
            }
            terminal
                .draw(|frame| render(frame, Duration::from_secs(2), &mut app))
                .unwrap();
            let scrolled = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(
                scrolled.contains("TAIL_RECOVERY_SENTINEL"),
                "recovery tail must be reachable at {size:?}"
            );
        }
    }
}
