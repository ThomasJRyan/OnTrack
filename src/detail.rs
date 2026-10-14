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

fn comments(v: &Value, width: u16) -> Vec<Line<'static>> {
    let mut output = Vec::new();
    for (index, comment) in entries(v).into_iter().enumerate() {
        if index > 0 {
            output.push(Line::default());
        }
        if let Some(object) = comment.as_object() {
            for key in ["author", "created_at", "date"] {
                if let Some(value) = object.get(key) {
                    output.extend(metadata(key, value));
                }
            }
            let body_key = if object.contains_key("body") {
                "body"
            } else {
                "text"
            };
            if let Some(body) = object.get(body_key) {
                output.extend(narrative_value(body, width));
            }
            let extras: Vec<_> = object
                .iter()
                .filter(|(key, _)| {
                    !["author", "created_at", "date", body_key].contains(&key.as_str())
                })
                .collect();
            if !extras.is_empty() {
                output.extend(json_value(
                    &Value::Object(
                        extras
                            .into_iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    ),
                    width,
                ));
            }
        } else {
            output.extend(json_value(comment, width));
        }
    }
    output
}

fn relationships(v: &Value, width: u16) -> Vec<Line<'static>> {
    let mut output = Vec::new();
    for (index, relation) in entries(v).into_iter().enumerate() {
        if index > 0 {
            output.push(Line::default());
        }
        if let Some(object) = relation.as_object() {
            let mut fields: Vec<_> = object.iter().collect();
            fields.sort_by_key(|(key, _)| {
                (
                    if ["id", "type", "title"].contains(&key.as_str()) {
                        0
                    } else {
                        1
                    },
                    *key,
                )
            });
            for (key, value) in fields {
                output.extend(metadata(key, value));
            }
        } else {
            output.extend(json_value(relation, width));
        }
    }
    output
}

/// Human-readable presentation of the complete Beads response, retaining unrecognized fields.
pub fn render(
    value: &Value,
    selected: Option<(&str, &str, &str, i64)>,
    width: u16,
) -> Text<'static> {
    let mut out = Vec::new();
    let object = value.as_object();
    let narrative = ["description", "notes", "design", "acceptance_criteria"];
    let excluded = [
        "comments",
        "dependencies",
        "dependents",
        "blocks",
        "blocked_by",
    ];
    if let Some((id, title, status, priority)) = selected {
        let field = |name: &str| object.and_then(|o| o.get(name));
        let shown_title = field("title").and_then(Value::as_str).unwrap_or(title);
        out.push(Line::from(Span::styled(
            shown_title.to_owned(),
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )));
        for (key, fallback) in [
            ("id", id.to_owned()),
            ("status", status.to_owned()),
            ("priority", format!("P{priority}")),
        ] {
            if let Some(v) = field(key) {
                out.extend(metadata(key, v));
            } else {
                out.extend(metadata(key, &Value::String(fallback)));
            }
        }
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
        let mut relation_heading = false;
        for key in [
            "comments",
            "dependencies",
            "dependents",
            "blocks",
            "blocked_by",
        ] {
            if let Some(v) = obj.get(key) {
                if !out.is_empty() {
                    out.push(Line::default());
                }
                if key == "comments" {
                    out.push(heading("COMMENTS"));
                } else if !relation_heading {
                    out.push(heading("RELATIONSHIPS"));
                    relation_heading = true;
                }
                out.push(Line::from(Span::styled(
                    key.replace('_', " "),
                    Style::default().fg(MUTED),
                )));
                if key == "comments" {
                    out.extend(comments(v, width));
                } else {
                    out.extend(relationships(v, width));
                }
            }
        }
        let mut remaining: Vec<_> = obj
            .iter()
            .filter(|(k, _)| {
                !narrative.contains(&k.as_str())
                    && !excluded.contains(&k.as_str())
                    && !(selected.is_some() && ["id", "status", "priority"].contains(&k.as_str()))
                    && !(selected.is_some()
                        && k.as_str() == "title"
                        && obj.get("title").and_then(Value::as_str).is_some())
            })
            .collect();
        remaining.sort_by_key(|(k, _)| *k);
        if !remaining.is_empty() {
            if !out.is_empty() {
                out.push(Line::default());
            }
            out.push(heading("OTHER DETAILS"));
            for (key, v) in remaining {
                out.extend(metadata(key, v));
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
    fn markdown_markup_is_rendered_and_complete_record_stays_visible() {
        let value = json!({
            "description": "# Plan\n\nA **bold** and *emphasized* note with `code` and [guide](https://example.test).\n\n- first",
            "notes": "## Notes\n\n- retain this",
            "comments": [{"body": "hello comment", "author": "Ada"}],
            "dependencies": [{"id": "other-1", "type": "blocks"}],
            "unknown": {"nested": [true, null, 5]}
        });
        let text = render(&value, Some(("demo-1", "Title", "open", 2)), 50);
        let plain = text.to_string();
        assert!(plain.contains("Plan"));
        assert!(plain.contains("bold") && !plain.contains("**bold**"));
        assert!(plain.contains("code") && !plain.contains("`code`"));
        assert!(plain.contains("https://example.test"));
        for expected in [
            "hello comment",
            "other-1",
            "unknown",
            "nested",
            "true",
            "null",
            "5",
            "demo-1",
            "Title",
        ] {
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
    fn preserves_typed_optional_fields_and_multiline_unknown_metadata() {
        let value = serde_json::json!({
            "title": null,
            "description": null,
            "notes": false,
            "design": [],
            "acceptance_criteria": "",
            "unknown": {"nested": ["line one", "line two", "FINAL_SENTINEL"]}
        });
        for selected in [Some(("cached", "Cached", "open", 1)), None] {
            let rendered = render(&value, selected, 18);
            let plain = rendered.to_string();
            for expected in [
                "title",
                "null",
                "DESCRIPTION",
                "NOTES",
                "false",
                "DESIGN",
                "[]",
                "ACCEPTANCE CRITERIA",
                "unknown",
                "line one",
                "line two",
                "FINAL_SENTINEL",
            ] {
                assert!(plain.contains(expected), "missing {expected}: {plain}");
            }
            assert!(rendered.lines.len() > 8);
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
            "blocks",
            "Other",
            "extra",
            "kept",
            "Alternate body retained",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert!(!text.contains("**bold**"));
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
