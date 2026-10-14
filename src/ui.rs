use crate::app::{App, DisplayRow};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::time::Duration;
const BG: Color = Color::Rgb(13, 23, 28);
const INK: Color = Color::Rgb(220, 228, 229);
const MUTED: Color = Color::Rgb(123, 144, 148);
const TEAL: Color = Color::Rgb(73, 211, 182);
const GOLD: Color = Color::Rgb(239, 183, 99);
const ASCII_LOGO: [[&str; 7]; 5] = [
    [
        " ### ", "#   #", "#####", "#### ", " ### ", " ####", "#   #",
    ],
    [
        "#   #", "##  #", "  #  ", "#   #", "#   #", "#    ", "#  # ",
    ],
    [
        "#   #", "# # #", "  #  ", "#### ", "#####", "#    ", "###  ",
    ],
    [
        "#   #", "#  ##", "  #  ", "#  # ", "#   #", "#    ", "#  # ",
    ],
    [
        " ### ", "#   #", "  #  ", "#   #", "#   #", " ####", "#   #",
    ],
];
fn status_color(status: &str) -> Color {
    match status {
        "open" => TEAL,
        "closed" => Color::Rgb(255, 123, 123),
        "in_progress" => GOLD,
        "blocked" => Color::Rgb(255, 123, 123),
        "deferred" => Color::Rgb(145, 170, 220),
        "unknown" | "" => MUTED,
        _ => Color::Rgb(125, 168, 235),
    }
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
fn row_priority_color(priority: u64) -> Color {
    match priority {
        0 => Color::Rgb(255, 105, 105),
        1 => Color::Rgb(255, 159, 105),
        2 => GOLD,
        3 => Color::Rgb(138, 197, 169),
        4 => MUTED,
        _ => MUTED,
    }
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
    f.render_widget(Block::default().style(Style::default().bg(BG)), a);
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
            .style(Style::default().fg(MUTED))
            .alignment(Alignment::Center),
            a,
        );
        return;
    }
    if app.modal.is_none() && elapsed < Duration::from_millis(1200) {
        let large = a.width >= 45 && a.height >= 9;
        let lines = if large {
            let colors = [TEAL, Color::Rgb(99, 205, 207), INK, GOLD, TEAL];
            ASCII_LOGO
                .iter()
                .zip(colors)
                .map(|(letters, color)| {
                    Line::from(Span::styled(
                        letters.join(" "),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ))
                })
                .chain([
                    Line::from(""),
                    Line::from(Span::styled(
                        "BEADS WORKSPACE BROWSER",
                        Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
                    )),
                ])
                .collect::<Vec<_>>()
        } else {
            vec![
                Line::from(Span::styled(
                    "ONTRACK",
                    Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "BEADS WORKSPACE BROWSER",
                    Style::default().fg(MUTED),
                )),
            ]
        };
        let height = lines.len() as u16;
        f.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .style(Style::default().fg(INK)),
            Rect::new(
                a.x,
                a.y + a.height.saturating_sub(height) / 2,
                a.width,
                height,
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
    let layout = crate::app::list_layout(&app.beads, &app.collapsed);
    let (type_width, priority_width) = metadata_column_widths(&app.beads);
    let viewport = split[0].height as usize;
    let focused_target = app
        .focused_group
        .as_ref()
        .and_then(|status| {
            layout
                .targets
                .iter()
                .find(|t| matches!(&t.row, DisplayRow::Group(s) if s == status))
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
        let color = status_color(&group.status);
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
            let focused = app.focused_group.as_deref() == Some(group.status.as_str());
            let marker = if group.collapsed { "▸" } else { "▾" };
            let title_style = if focused {
                Style::default()
                    .fg(BG)
                    .bg(color)
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
                                .fg(if selected { TEAL } else { MUTED })
                                .add_modifier(if selected {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                        Span::styled(
                            kind,
                            Style::default().fg(row_type_color(&row_issue_type(bead))),
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
                            Style::default().fg(row_priority_color(bead.priority)),
                        ),
                        Span::raw("  "),
                        Span::styled(status, Style::default().fg(status_color(status))),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(bead.title.clone(), Style::default().fg(Color::White)),
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
    let detail_text = detail
        .map(|v| crate::detail::render(v, selected_metadata, inner_width))
        .unwrap_or_else(|| {
            ratatui::text::Text::from(if app.beads.is_empty() {
                "No issues found in this workspace.\n\nPress r to refresh."
            } else {
                "Select an issue to inspect its full record."
            })
        });
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
    } else if let Some(notice) = &app.notice {
        notice.clone()
    } else {
        "READY".into()
    };
    f.render_widget(
        Paragraph::new(format!(
            " {state}  ·  ↑↓ move  ← fold  → unfold  ·  Tab panes  ·  n new  ·  d delete  ·  r refresh  ·  q quit"
        ))
        .style(Style::default().fg(MUTED)),
        v[2],
    );
    if let Some(modal) = &app.modal {
        let width = match modal {
            crate::app::Modal::Create(_) => 70,
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
        };
        let min_width = if matches!(modal, crate::app::Modal::Create(_)) {
            70
        } else {
            50
        };
        if a.width < min_width || a.height < height {
            app.modal_ready = false;
            f.render_widget(Clear, a);
            let prompt = if a.width < min_width {
                format!("Expand terminal width to {min_width} columns · Esc cancels")
            } else {
                format!("Expand terminal height to {height} rows · Esc cancels")
            };
            f.render_widget(
                Paragraph::new(prompt)
                    .style(Style::default().fg(GOLD))
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
                _ => " DELETE BEAD? ",
            })
            .borders(Borders::ALL)
            .border_style(Style::default().fg(TEAL))
            .style(Style::default().bg(Color::Rgb(20, 33, 38)));
        f.render_widget(frame, area);
        let inner = Rect::new(
            area.x + 2,
            area.y + 1,
            area.width.saturating_sub(4),
            area.height.saturating_sub(2),
        );
        match modal {
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
                                    TEAL
                                } else {
                                    MUTED
                                })),
                        )
                        .style(Style::default().fg(INK))
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
                    Paragraph::new(title_slice).style(Style::default().fg(INK)),
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
                        type_color(selected)
                    } else {
                        priority_color(selected)
                    };
                    let border = if form.focus == which || dropdown == Some(which) {
                        TEAL
                    } else {
                        MUTED
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
                                .map(|(i, x)| (i, x.to_string(), type_color(i)))
                                .collect::<Vec<_>>()
                        } else {
                            (0..=4)
                                .map(|i| (i, format!("P{i}"), priority_color(i)))
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
                                    .fg(BG)
                                    .bg(*col)
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
                            .style(Style::default().fg(BG).bg(color)),
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
                                    TEAL
                                } else {
                                    MUTED
                                })),
                        )
                        .style(Style::default().fg(INK))
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
                            Style::default().fg(MUTED)
                        } else if form.focus == 4 {
                            Style::default()
                                .fg(BG)
                                .bg(TEAL)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(TEAL)
                        },
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "[ Cancel ]",
                        if app.mutation_busy {
                            Style::default().fg(MUTED)
                        } else if form.focus == 5 {
                            Style::default()
                                .fg(BG)
                                .bg(GOLD)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(GOLD)
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
                        "Tab / Shift+Tab moves · Enter opens or activates · Esc cancels",
                    ),
                    rows[4].width as usize,
                );
                f.render_widget(
                    Paragraph::new(feedback).style(Style::default().fg(
                        if app.notice.is_some() || app.mutation_busy {
                            GOLD
                        } else {
                            MUTED
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
                        .style(Style::default().fg(INK)),
                    inner,
                );
            }
        }
    }
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
            focused_group: None,
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
            modal: None,
            notice: None,
            mutation_busy: false,
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
        t.draw(|f| render(f, Duration::from_secs(1), a)).unwrap();
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
            .draw(|f| render(f, Duration::from_secs(1), &mut a))
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
            row_priority_color(1)
        );
        assert_eq!(
            buffer[(a.list_rect.x + 16, bead_row)].fg,
            status_color("closed")
        );
        assert_eq!(buffer[(a.list_rect.x + 3, bead_row + 1)].fg, Color::White);
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
            .draw(|f| render(f, Duration::from_secs(1), &mut a))
            .unwrap();
        let y = a.list_rect.y + 2;
        assert_eq!(
            terminal.backend().buffer()[(a.list_rect.x + 3, y)].fg,
            Color::White
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
            .draw(|f| render(f, Duration::from_secs(1), &mut a))
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
            .draw(|f| render(f, Duration::from_secs(1), &mut a))
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
            .draw(|f| render(f, Duration::from_secs(1), &mut a))
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
            .draw(|f| render(f, Duration::from_secs(1), &mut a))
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
        assert!(buf.content().iter().any(|cell| cell.bg == type_color(8)));
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
}
