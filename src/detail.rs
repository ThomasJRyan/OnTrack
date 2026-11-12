use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use serde_json::Value;
use tui_markdown::{Options, StyleSheet, from_str_with_options};

const INK: Color = Color::Rgb(220, 228, 229);
const MUTED: Color = Color::Rgb(123, 144, 148);
const TEAL: Color = Color::Rgb(73, 211, 182);
const GOLD: Color = Color::Rgb(239, 183, 99);

#[derive(Clone, Copy, Debug)]
struct BeadStyles;

impl StyleSheet for BeadStyles {
    fn heading(&self, level: u8) -> Style {
        Style::default()
            .fg(if level == 1 { GOLD } else { TEAL })
            .add_modifier(Modifier::BOLD)
    }
    fn heading_marker(&self, _: u8) -> &str {
        ""
    }
    fn code(&self) -> Style {
        Style::default().fg(GOLD)
    }
    fn link(&self) -> Style {
        Style::default().fg(TEAL).add_modifier(Modifier::UNDERLINED)
    }
    fn blockquote(&self) -> Style {
        Style::default().fg(MUTED).add_modifier(Modifier::ITALIC)
    }
    fn table_header(&self) -> Style {
        Style::default().fg(TEAL).add_modifier(Modifier::BOLD)
    }
    fn table_border(&self) -> Style {
        Style::default().fg(MUTED)
    }
    fn list_marker(&self) -> Style {
        Style::default().fg(GOLD)
    }
}

fn md(source: &str, width: u16) -> Text<'static> {
    let opts = Options::new(BeadStyles).table_width(width.max(1));
    let borrowed = from_str_with_options(source, &opts);
    Text::from(
        borrowed
            .lines
            .iter()
            .map(|line| {
                Line::from(
                    line.spans
                        .iter()
                        .map(|span| Span::styled(span.content.to_string(), span.style))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn heading(label: &str) -> Line<'static> {
    Line::from(Span::styled(
        label.to_owned(),
        Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
    ))
}

fn metadata(key: &str, value: &Value) -> Vec<Line<'static>> {
    let raw = match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    };
    let mut lines = raw.lines().map(str::to_owned).collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(String::new());
    }
    let mut output = Vec::with_capacity(lines.len());
    for (index, line) in lines.into_iter().enumerate() {
        output.push(if index == 0 {
            Line::from(vec![
                Span::styled(format!("{key}: "), Style::default().fg(MUTED)),
                Span::styled(line, Style::default().fg(INK)),
            ])
        } else {
            Line::from(Span::styled(format!("  {line}"), Style::default().fg(INK)))
        });
    }
    output
}

fn json_value(v: &Value, width: u16) -> Vec<Line<'static>> {
    match v {
        Value::String(s) => md(s, width).lines,
        _ => {
            let raw = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
            md(&format!("```json\n{raw}\n```"), width).lines
        }
    }
}

fn narrative_value(v: &Value, width: u16) -> Vec<Line<'static>> {
    match v {
        Value::String(s) => md(s, width).lines,
        _ => {
            let raw = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
            metadata("value", &Value::String(raw))
        }
    }
}

fn entries(v: &Value) -> Vec<&Value> {
    v.as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_else(|| vec![v])
}

fn card_row(spans: Vec<Span<'static>>, cells: usize, width: usize, border: Style) -> Line<'static> {
    let mut row = vec![Span::styled("│ ", border)];
    row.extend(spans);
    row.push(Span::raw(" ".repeat(width.saturating_sub(cells))));
    row.push(Span::styled(" │", border));
    Line::from(row)
}

fn card_text(line: Line<'static>, width: usize, border: Style) -> Vec<Line<'static>> {
    let mut rows = Vec::new();
    let mut spans = Vec::new();
    let mut cells = 0;
    for span in line.spans {
        let mut chunk = String::new();
        for ch in span.content.chars() {
            let size = Span::raw(ch.to_string()).width();
            if cells + size > width && cells > 0 {
                if !chunk.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut chunk), span.style));
                }
                rows.push(card_row(std::mem::take(&mut spans), cells, width, border));
                cells = 0;
            }
            chunk.push(ch);
            cells += size;
        }
        if !chunk.is_empty() {
            spans.push(Span::styled(chunk, span.style));
        }
    }
    rows.push(card_row(spans, cells, width, border));
    rows
}

fn managed_comment(body: &str) -> (bool, &str) {
    let Some((marker, text)) = body.split_once("\n\n") else {
        return (false, body);
    };
    let Some(marker) = marker
        .strip_prefix("[ontrack-message ")
        .and_then(|s| s.strip_suffix(']'))
    else {
        return (false, body);
    };
    let Ok(value) = serde_json::from_str::<Value>(marker) else {
        return (false, body);
    };
    if value["v"] != 1 || !value["id"].is_string() || !value["handoff"].is_string() {
        return (false, body);
    }
    match value["kind"].as_str() {
        Some("question" | "response") => (true, text),
        Some("save" | "user") => (false, text),
        _ => (false, body),
    }
}

fn comments(v: &Value, width: u16) -> Vec<Line<'static>> {
    let mut output = Vec::new();
    for (index, comment) in entries(v).into_iter().enumerate() {
        if index > 0 {
            output.push(Line::default());
        }
        let object = comment.as_object();
        let body = object
            .and_then(|o| o.get("body").or_else(|| o.get("text")))
            .or_else(|| comment.as_str().map(|_| comment))
            .and_then(Value::as_str)
            .unwrap_or("");
        let (agent, body) = managed_comment(body);
        let author = if agent {
            Some("Agent")
        } else {
            object
                .and_then(|o| o.get("author"))
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
        };
        let timestamp = object
            .and_then(|o| o.get("created_at").or_else(|| o.get("date")))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty());
        let border = Style::default().fg(if agent {
            Color::Indexed(4)
        } else {
            Color::Indexed(5)
        });
        let inner = usize::from(width.saturating_sub(4));
        if width < 6 {
            if let Some(author) = author {
                output.push(Line::styled(
                    author.to_owned(),
                    border.add_modifier(Modifier::BOLD),
                ));
            }
            if let Some(timestamp) = timestamp {
                output.push(Line::styled(
                    timestamp.to_owned(),
                    Style::default().fg(MUTED),
                ));
            }
            output.extend(md(body, width).lines);
            continue;
        }
        output.push(Line::styled(
            format!("╭{}╮", "─".repeat(usize::from(width) - 2)),
            border,
        ));
        if author.is_some() || timestamp.is_some() {
            let mut header = Vec::new();
            if let Some(author) = author {
                header.push(Span::styled(
                    author.to_owned(),
                    border.add_modifier(Modifier::BOLD),
                ));
            }
            if let Some(timestamp) = timestamp {
                if author.is_some() {
                    header.push(Span::raw(" · "));
                }
                header.push(Span::styled(
                    timestamp.to_owned(),
                    Style::default().fg(MUTED),
                ));
            }
            output.extend(card_text(Line::from(header), inner, border));
        }
        output.extend(
            md(body, inner as u16)
                .lines
                .into_iter()
                .flat_map(|line| card_text(line, inner, border)),
        );
        output.push(Line::styled(
            format!("╰{}╯", "─".repeat(usize::from(width) - 2)),
            border,
        ));
    }
    output
}

const RELATION_FIELDS: [&str; 4] = ["dependencies", "dependents", "blocks", "blocked_by"];

// Wrap by terminal cells, keeping literal labels out of the Markdown parser.
fn chart_wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(2);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut cells = 0;
    for ch in text.chars() {
        let size = Span::raw(ch.to_string()).width();
        if ch == '\n' || cells + size > width {
            lines.push(std::mem::take(&mut line));
            cells = 0;
        }
        if !ch.is_control() {
            line.push(ch);
            cells += size;
        }
    }
    lines.push(line);
    lines
}

// Indexed semantic colors are mapped to the active palette by the details pane.
fn node_style(value: &Value) -> Style {
    let color = match value.get("priority").and_then(Value::as_u64) {
        Some(0) => Color::Indexed(1),
        Some(1 | 2) => Color::Indexed(3),
        Some(3) => Color::Indexed(2),
        _ => MUTED,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn chart_row(value: &Value, prefix: &str, continuation: &str, width: usize) -> Vec<Line<'static>> {
    let label = node_label(value).unwrap_or_else(|| "Selected Bead".to_owned());
    // Tiny panes spend their limited width on content rather than indentation.
    let (prefix, continuation) = if width < 12 {
        ("", "")
    } else {
        (prefix, continuation)
    };
    let available = width.saturating_sub(Span::raw(prefix).width()).max(2);
    chart_wrap(&label, available)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            Line::from(vec![
                Span::styled(
                    if index == 0 { prefix } else { continuation }.to_owned(),
                    node_style(value),
                ),
                Span::styled(
                    line,
                    if index == 0 {
                        node_style(value)
                    } else {
                        Style::default().fg(INK)
                    },
                ),
            ])
        })
        .collect()
}

fn node_label(value: &Value) -> Option<String> {
    if let Some(id) = value.as_str().filter(|s| !s.trim().is_empty()) {
        return Some(id.to_owned());
    }
    let id = value.get("id")?.as_str().filter(|s| !s.trim().is_empty())?;
    let mut label = id.to_owned();
    if let Some(priority) = value.get("priority").and_then(Value::as_u64) {
        label.push_str(&format!(" · P{priority}"));
    }
    if let Some(status) = value.get("status").and_then(Value::as_str) {
        label.push_str(&format!(" [{status}]"));
    }
    if let Some(title) = value.get("title").and_then(Value::as_str) {
        label.push('\n');
        label.push_str(title);
    }
    Some(label)
}

fn relationship_chart(value: &Value, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width);
    let mut groups: Vec<(String, Vec<&Value>)> = Vec::new();
    for key in RELATION_FIELDS {
        let Some(relations) = value.get(key) else {
            continue;
        };
        for relation in entries(relations) {
            let Some(_) = node_label(relation) else {
                continue;
            };
            let kind_key = if relation
                .get("dependency_type")
                .and_then(Value::as_str)
                .is_some()
            {
                "dependency_type"
            } else {
                "type"
            };
            let kind = relation.get(kind_key).and_then(Value::as_str).unwrap_or(
                if matches!(key, "blocks" | "blocked_by") {
                    "blocks"
                } else {
                    "unspecified"
                },
            );
            let group = match (kind, key) {
                ("blocks", "dependencies" | "blocked_by") => {
                    "Blocked by ← prerequisites".to_owned()
                }
                ("blocks", _) => "Blocks → dependents".to_owned(),
                ("parent-child", "dependencies") => "Parents".to_owned(),
                ("parent-child", "dependents") => "Children".to_owned(),
                _ => format!("{kind} ({key})"),
            };
            if let Some((_, nodes)) = groups.iter_mut().find(|(label, _)| *label == group) {
                nodes.push(relation);
            } else {
                groups.push((group, vec![relation]));
            }
        }
    }
    let mut out = Vec::new();
    if !groups.is_empty() {
        out.extend(chart_row(value, "── ", "   ", width));
        // Blocking links come first, even when hierarchy data was returned first.
        groups.sort_by_key(|(label, _)| match label.as_str() {
            "Blocked by ← prerequisites" => 0,
            "Blocks → dependents" => 1,
            _ => 2,
        });
        for (label, nodes) in groups {
            out.extend(chart_wrap(&label, width).into_iter().map(|line| {
                Line::styled(line, Style::default().fg(TEAL).add_modifier(Modifier::BOLD))
            }));
            for (index, node) in nodes.iter().enumerate() {
                let last = index + 1 == nodes.len();
                out.extend(chart_row(
                    node,
                    if last { "└─ " } else { "├─ " },
                    if last { "   " } else { "│  " },
                    width,
                ));
            }
        }
    } else if RELATION_FIELDS
        .iter()
        .filter_map(|key| value.get(key))
        .all(|relations| relations.as_array().is_some_and(Vec::is_empty))
    {
        out.push(Line::from("No dependency links returned."));
    }
    out
}

/// Present narrative fields, comments, and supported dependency links.
/// Miscellaneous issue metadata and relationship extras stay in the source JSON only.
pub fn render(value: &Value, selected_title: Option<&str>, width: u16) -> Text<'static> {
    let mut out = Vec::new();
    let object = value.as_object();
    let narrative = ["description", "notes", "design", "acceptance_criteria"];
    if let Some(title) = selected_title {
        let field = |name: &str| object.and_then(|o| o.get(name));
        let shown_title = field("title").and_then(Value::as_str).unwrap_or(title);
        out.push(Line::from(Span::styled(
            shown_title.to_owned(),
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )));
    }
    if let Some(obj) = object {
        for key in narrative {
            if let Some(v) = obj.get(key) {
                if !out.is_empty() {
                    out.push(Line::default());
                }
                out.push(heading(&key.replace('_', " ").to_uppercase()));
                out.extend(narrative_value(v, width));
            }
        }
        if let Some(v) = obj.get("comments") {
            if !out.is_empty() {
                out.push(Line::default());
            }
            out.push(heading("COMMENTS"));
            out.extend(comments(v, width));
        }
        if RELATION_FIELDS.iter().any(|key| obj.contains_key(*key)) {
            let chart = relationship_chart(value, width);
            if !chart.is_empty() {
                if !out.is_empty() {
                    out.push(Line::default());
                }
                out.push(heading("DEPENDENCY FLOW"));
                out.extend(chart);
            }
        }
    } else {
        out.extend(json_value(value, width));
    }
    Text::from(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn blocking_flow_points_from_prerequisite_to_dependent() {
        let value = json!({
            "id": "task", "title": "Ship", "status": "open",
            "dependencies": [{"id": "base", "dependency_type": "blocks", "title": "Build", "status": "closed"}],
            "dependents": [{"id": "next", "dependency_type": "blocks"}]
        });
        let chart = Text::from(relationship_chart(&value, 24)).to_string();
        assert_eq!(
            chart,
            concat!(
                "── task [open]\n",
                "   Ship\n",
                "Blocked by ← prerequisit\n",
                "es\n",
                "└─ base [closed]\n",
                "   Build\n",
                "Blocks → dependents\n",
                "└─ next"
            )
        );
    }

    #[test]
    fn eight_children_fit_one_screen_with_priority_and_solid_branches() {
        let children: Vec<_> = (0..8)
            .map(|i| {
                json!({
                    "id": format!("OCA_Genie-4ao.{i}"), "priority": i % 5,
                    "status": "open", "dependency_type": "parent-child",
                    "title": "Decode the complete recording into an ordered replay timeline"
                })
            })
            .collect();
        let value = json!({"id": "OCA_Genie-4ao", "priority": 1,
            "title": "Implement frame by frame replayability", "dependents": children});
        let chart = relationship_chart(&value, 100);
        assert_eq!(chart.len(), 19);
        let plain = Text::from(chart.clone()).to_string();
        assert_eq!(plain.matches("Children").count(), 1);
        assert_eq!(plain.matches("├─").count(), 7);
        assert_eq!(plain.matches("└─").count(), 1);
        assert!(!plain.contains("RELATIONSHIP DETAILS"));
        for (priority, color) in [
            (0, Color::Indexed(1)),
            (1, Color::Indexed(3)),
            (2, Color::Indexed(3)),
            (3, Color::Indexed(2)),
            (4, MUTED),
        ] {
            let row = chart
                .iter()
                .find(|line| {
                    line.to_string()
                        .contains(&format!("4ao.{priority} · P{priority}"))
                })
                .unwrap();
            assert_eq!(row.spans[0].style.fg, Some(color));
        }
        assert_eq!(node_style(&json!({"priority": "invalid"})).fg, Some(MUTED));
        assert_eq!(node_style(&json!({})).fg, Some(MUTED));
    }

    #[test]
    fn blockers_precede_hierarchy_and_nonblocking_groups_keep_source_direction() {
        let chart = Text::from(relationship_chart(
            &json!({"id": "task",
                "dependencies": [{"id": "parent", "dependency_type": "parent-child"}],
                "dependents": [{"id": "child", "dependency_type": "parent-child"}],
                "blocked_by": ["base"], "blocks": ["next"]
            }),
            100,
        ))
        .to_string();
        assert!(chart.find("Blocked by").unwrap() < chart.find("Blocks →").unwrap());
        assert!(chart.find("Blocks →").unwrap() < chart.find("Parents").unwrap());
        assert!(chart.contains("Parents\n└─ parent"));
        assert!(chart.contains("Children\n└─ child"));
    }

    #[test]
    fn aliases_and_nonblocking_types_do_not_invent_blockers() {
        let value = json!({"id": "task",
            "blocked_by": ["base"], "blocks": ["next"],
            "dependencies": [{"id": "parent", "dependency_type": "parent-child"},
                             {"id": "friend", "type": "related"}, {"id": "unknown"}]
        });
        let chart = Text::from(relationship_chart(&value, 60)).to_string();
        for expected in [
            "Blocked by ← prerequisites",
            "Blocks → dependents",
            "Parents",
            "related (dependencies)",
            "unspecified (dependencies)",
        ] {
            assert!(chart.contains(expected), "{chart}");
        }
        assert_eq!(chart.matches("prerequisites").count(), 1);
    }

    #[test]
    fn chart_wraps_literal_unicode_labels_and_omits_relationship_extras() {
        let value = json!({"id": "selected", "dependencies": [{
            "id": "base", "dependency_type": "blocks", "type": "task",
            "title": "**literal** 界界 é title with a long ending", "status": "open",
            "priority": 1, "extra": {"revision": "keep-me"}
        }, {"unexpected": "fallback"}, null, 7]});
        for width in [2, 8, 12, 18, 40, 80] {
            let chart = relationship_chart(&value, width);
            assert!(
                chart.iter().all(|line| line.width() <= usize::from(width)),
                "width {width}: {chart:?}"
            );
            let plain = Text::from(chart).to_string();
            for omitted in [
                "RELATIONSHIP DETAILS",
                "keep-me",
                "unexpected",
                "fallback",
                "null",
            ] {
                assert!(!plain.contains(omitted), "{plain}");
            }
            let label = "**literal** 界界 é title with a long ending";
            assert_eq!(chart_wrap(label, usize::from(width)).concat(), label);
        }
        let wide = Text::from(relationship_chart(&value, 80)).to_string();
        assert!(wide.contains("**literal**"));
        for width in [0, 1] {
            relationship_chart(&value, width);
        }
    }

    #[test]
    fn absent_and_empty_relationships_are_explicit_without_fabricated_nodes() {
        assert!(
            !render(&json!({"id": "task"}), None, 40)
                .to_string()
                .contains("DEPENDENCY FLOW")
        );
        let empty = render(&json!({"dependencies": [], "dependents": []}), None, 40).to_string();
        assert!(empty.contains("No dependency links returned."));
        assert!(!empty.contains("Selected Bead"));
        let malformed =
            render(&json!({"dependencies": {"unexpected": true}}), None, 40).to_string();
        assert!(malformed.is_empty());
        assert!(!malformed.contains("No dependency links"));
    }

    #[test]
    fn markdown_markup_comments_and_dependency_nodes_stay_visible() {
        let value = json!({
            "description": "# Plan\n\nA **bold** and *emphasized* note with `code` and [guide](https://example.test).\n\n- first",
            "notes": "## Notes\n\n- retain this",
            "comments": [{"body": "hello comment", "author": "Ada"}],
            "dependencies": [{"id": "other-1", "type": "blocks"}],
            "unknown": {"nested": [true, null, 5]}
        });
        let text = render(&value, Some("Title"), 50);
        let plain = text.to_string();
        assert!(plain.contains("Plan"));
        assert!(plain.contains("bold") && !plain.contains("**bold**"));
        assert!(plain.contains("code") && !plain.contains("`code`"));
        assert!(plain.contains("https://example.test"));
        for expected in ["hello comment", "other-1", "Title"] {
            assert!(plain.contains(expected), "missing {expected}: {plain}");
        }
        assert!(
            text.lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.content.contains("bold")
                    && span.style.add_modifier.contains(Modifier::BOLD))
        );
    }

    #[test]
    fn preserves_typed_narrative_fields() {
        let value = serde_json::json!({
            "title": null,
            "description": null,
            "notes": false,
            "design": [],
            "acceptance_criteria": "",
            "unknown": {"nested": ["line one", "line two", "FINAL_SENTINEL"]}
        });
        for selected in [Some("Cached"), None] {
            let rendered = render(&value, selected, 18);
            let plain = rendered.to_string();
            for expected in [
                "null",
                "DESCRIPTION",
                "NOTES",
                "false",
                "DESIGN",
                "[]",
                "ACCEPTANCE CRITERIA",
            ] {
                assert!(plain.contains(expected), "missing {expected}: {plain}");
            }
            assert!(rendered.lines.len() > 8);
        }
    }

    #[test]
    fn hides_miscellaneous_metadata_without_changing_source_json() {
        let value = json!({
            "id": "demo-1",
            "title": "Title",
            "revision": "stale-revision",
            "created_at": "today",
            "unknown": {"revision": "nested-revision"}
        });

        for selected in [Some("Title"), None] {
            let plain = render(&value, selected, 50).to_string();
            assert!(!plain.contains("stale-revision"), "{plain}");
            for omitted in [
                "OTHER DETAILS",
                "created_at",
                "today",
                "unknown",
                "nested-revision",
            ] {
                assert!(!plain.contains(omitted), "{plain}");
            }
        }
        assert_eq!(value["revision"], "stale-revision");
    }

    #[test]
    fn selected_issue_omits_repeated_header_metadata() {
        let value = json!({
            "id": "demo-1",
            "title": "Live title",
            "status": "in_progress",
            "priority": 3,
            "description": "Useful description",
            "owner": "Ada",
            "comments": [{"body": "Keep this comment"}],
            "dependencies": [{"id": "other-1", "type": "blocks"}]
        });
        let plain = render(&value, Some("Cached title"), 50).to_string();
        for expected in [
            "Live title",
            "Useful description",
            "Keep this comment",
            "other-1",
        ] {
            assert!(plain.contains(expected), "missing {expected}: {plain}");
        }
        for omitted in ["id: demo-1", "status: in_progress", "priority: 3"] {
            assert!(!plain.contains(omitted), "unexpected {omitted}: {plain}");
        }
    }

    #[test]
    fn comments_render_markdown_and_relationships_are_readable() {
        let value = serde_json::json!({
            "comments": [{"author":"Ada", "created_at":"today", "body":"A **bold** comment", "text":"Alternate body retained", "extra":{"x":1}}],
            "dependencies": [{"id":"other-1", "type":"blocks", "title":"Other", "extra":"kept"}]
        });
        let text = render(&value, None, 40).to_string();
        for expected in [
            "Ada",
            "today",
            "bold",
            "other-1",
            "Blocked by ← prerequisites",
            "Other",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert!(!text.contains("**bold**"));
        assert!(!text.contains("Alternate body retained"));
        assert!(!text.contains("extra"));
    }

    #[test]
    fn comment_cards_hide_internal_json_and_distinguish_agent_from_user() {
        let marker = |kind| {
            format!(
                "[ontrack-message {{\"v\":1,\"id\":\"handoff:m1\",\"handoff\":\"handoff\",\"kind\":\"{kind}\"}}]\n\nA **bold** comment with 界界界界界"
            )
        };
        let value = json!({"comments": [
            {"author":"Thomas", "created_at":"today", "body":marker("response"), "id":"hidden-id", "extra":{"secret":"hidden-json"}},
            {"author":"Thomas", "created_at":"tomorrow", "body":marker("user")},
            {"author":"Ada", "date":"later", "body":"Plain **note**"}
        ]});
        let lines = render(&value, None, 22).lines;
        let plain = Text::from(lines.clone()).to_string();
        for expected in [
            "Agent",
            "Thomas",
            "Ada",
            "today",
            "tomorrow",
            "later",
            "bold",
            "Plain note",
        ] {
            assert!(plain.contains(expected), "missing {expected}: {plain}");
        }
        for hidden in [
            "ontrack-message",
            "handoff:m1",
            "hidden-id",
            "hidden-json",
            "extra",
            "**bold**",
        ] {
            assert!(!plain.contains(hidden), "unexpected {hidden}: {plain}");
        }
        assert_eq!(plain.matches('╭').count(), 3);
        assert!(lines.iter().all(|line| line.width() <= 22));
        let agent = lines
            .iter()
            .find(|line| line.to_string().contains("Agent"))
            .unwrap();
        let user = lines
            .iter()
            .find(|line| line.to_string().contains("Thomas"))
            .unwrap();
        assert_eq!(agent.spans[0].style.fg, Some(Color::Indexed(4)));
        assert_eq!(agent.spans[1].style.fg, Some(Color::Indexed(4)));
        assert_eq!(user.spans[0].style.fg, Some(Color::Indexed(5)));
        assert_eq!(user.spans[1].style.fg, Some(Color::Indexed(5)));
        let narrow = render(&value, None, 5).to_string();
        assert!(narrow.contains("Agent") && narrow.contains("today"));
    }

    #[test]
    fn markdown_elements_style_and_fit_narrow_tables() {
        let text = md(
            "# Heading\n\n- *item*\n\n```rust\nlet x = 1;\n```\n\n| A | B |\n| --- | --- |\n| 界界 | value |",
            12,
        );
        let plain = text.to_string();
        assert!(
            plain.contains("Heading")
                && plain.contains("item")
                && plain.contains("let x")
                && plain.contains("va"),
            "{plain}"
        );
        assert!(text.lines.iter().all(|line| line.width() <= 12));
        assert!(
            text.lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.style.add_modifier.contains(Modifier::ITALIC))
        );
        assert!(
            text.lines
                .iter()
                .any(|line| line.to_string().contains("let x"))
        );
    }
}
