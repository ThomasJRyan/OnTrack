use crate::settings::Theme;
use crate::worktrees::WorktreeStatus;
use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub background: Color,
    pub panel: Color,
    pub panel_alt: Color,
    pub foreground: Color,
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub warning: Color,
    pub danger: Color,
    pub success: Color,
    pub info: Color,
    pub code_background: Color,
    pub selected_background: Color,
    pub selected_foreground: Color,
}

#[derive(Clone, Copy, Debug)]
pub struct ThemeDefinition {
    pub theme: Theme,
    pub id: &'static str,
    pub name: &'static str,
    pub is_light: bool,
    pub palette: Palette,
}

const fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::Rgb(red, green, blue)
}

pub const CATALOG: [ThemeDefinition; 12] = [
    ThemeDefinition {
        theme: Theme::Dark,
        id: "dark",
        name: "OnTrack Dark",
        is_light: false,
        palette: Palette {
            background: rgb(13, 23, 28),
            panel: rgb(20, 33, 38),
            panel_alt: rgb(28, 43, 48),
            foreground: rgb(220, 228, 229),
            muted: rgb(123, 144, 148),
            border: rgb(73, 211, 182),
            accent: rgb(73, 211, 182),
            accent_alt: rgb(145, 170, 220),
            warning: rgb(239, 183, 99),
            danger: rgb(255, 123, 123),
            success: rgb(73, 211, 182),
            info: rgb(125, 168, 235),
            code_background: rgb(9, 17, 21),
            selected_background: rgb(73, 211, 182),
            selected_foreground: rgb(13, 23, 28),
        },
    },
    ThemeDefinition {
        theme: Theme::Light,
        id: "light",
        name: "OnTrack Light",
        is_light: true,
        palette: Palette {
            background: rgb(246, 248, 247),
            panel: rgb(232, 239, 237),
            panel_alt: rgb(222, 232, 229),
            foreground: rgb(32, 43, 45),
            muted: rgb(76, 98, 101),
            border: rgb(83, 101, 104),
            accent: rgb(0, 113, 94),
            accent_alt: rgb(55, 91, 142),
            warning: rgb(139, 86, 0),
            danger: rgb(150, 45, 45),
            success: rgb(0, 112, 91),
            info: rgb(0, 100, 112),
            code_background: rgb(238, 243, 241),
            selected_background: rgb(0, 113, 94),
            selected_foreground: rgb(246, 248, 247),
        },
    },
    ThemeDefinition {
        theme: Theme::TokyoNight,
        id: "tokyo-night",
        name: "Tokyo Night",
        is_light: false,
        palette: Palette {
            background: rgb(26, 27, 38),
            panel: rgb(36, 40, 59),
            panel_alt: rgb(41, 46, 66),
            foreground: rgb(192, 202, 245),
            muted: rgb(127, 137, 174),
            border: rgb(65, 72, 104),
            accent: rgb(122, 162, 247),
            accent_alt: rgb(187, 154, 247),
            warning: rgb(224, 175, 104),
            danger: rgb(247, 118, 142),
            success: rgb(158, 206, 106),
            info: rgb(125, 207, 255),
            code_background: rgb(31, 35, 53),
            selected_background: rgb(122, 162, 247),
            selected_foreground: rgb(26, 27, 38),
        },
    },
    ThemeDefinition {
        theme: Theme::Nord,
        id: "nord",
        name: "Nord",
        is_light: false,
        palette: Palette {
            background: rgb(46, 52, 64),
            panel: rgb(59, 66, 82),
            panel_alt: rgb(67, 76, 94),
            foreground: rgb(236, 239, 244),
            muted: rgb(177, 186, 201),
            border: rgb(76, 86, 106),
            accent: rgb(136, 192, 208),
            accent_alt: rgb(180, 142, 173),
            warning: rgb(235, 203, 139),
            danger: rgb(191, 97, 106),
            success: rgb(163, 190, 140),
            info: rgb(129, 161, 193),
            code_background: rgb(43, 49, 60),
            selected_background: rgb(136, 192, 208),
            selected_foreground: rgb(46, 52, 64),
        },
    },
    ThemeDefinition {
        theme: Theme::Dracula,
        id: "dracula",
        name: "Dracula",
        is_light: false,
        palette: Palette {
            background: rgb(40, 42, 54),
            panel: rgb(52, 55, 70),
            panel_alt: rgb(68, 71, 90),
            foreground: rgb(248, 248, 242),
            muted: rgb(189, 189, 204),
            border: rgb(98, 101, 128),
            accent: rgb(189, 147, 249),
            accent_alt: rgb(255, 121, 198),
            warning: rgb(241, 250, 140),
            danger: rgb(255, 85, 85),
            success: rgb(80, 250, 123),
            info: rgb(139, 233, 253),
            code_background: rgb(33, 34, 44),
            selected_background: rgb(189, 147, 249),
            selected_foreground: rgb(40, 42, 54),
        },
    },
    ThemeDefinition {
        theme: Theme::GruvboxDark,
        id: "gruvbox-dark",
        name: "Gruvbox Dark",
        is_light: false,
        palette: Palette {
            background: rgb(40, 40, 40),
            panel: rgb(60, 56, 54),
            panel_alt: rgb(80, 73, 69),
            foreground: rgb(235, 219, 178),
            muted: rgb(189, 174, 147),
            border: rgb(102, 92, 84),
            accent: rgb(250, 189, 47),
            accent_alt: rgb(211, 134, 155),
            warning: rgb(250, 189, 47),
            danger: rgb(251, 73, 52),
            success: rgb(184, 187, 38),
            info: rgb(131, 165, 152),
            code_background: rgb(29, 32, 33),
            selected_background: rgb(250, 189, 47),
            selected_foreground: rgb(40, 40, 40),
        },
    },
    ThemeDefinition {
        theme: Theme::GruvboxLight,
        id: "gruvbox-light",
        name: "Gruvbox Light",
        is_light: true,
        palette: Palette {
            background: rgb(251, 241, 199),
            panel: rgb(242, 229, 188),
            panel_alt: rgb(235, 219, 178),
            foreground: rgb(60, 56, 54),
            muted: rgb(102, 92, 84),
            border: rgb(168, 153, 132),
            accent: rgb(175, 58, 3),
            accent_alt: rgb(143, 63, 113),
            warning: rgb(181, 118, 20),
            danger: rgb(157, 0, 6),
            success: rgb(121, 116, 14),
            info: rgb(7, 102, 120),
            code_background: rgb(247, 236, 194),
            selected_background: rgb(175, 58, 3),
            selected_foreground: rgb(251, 241, 199),
        },
    },
    ThemeDefinition {
        theme: Theme::CatppuccinMocha,
        id: "catppuccin-mocha",
        name: "Catppuccin Mocha",
        is_light: false,
        palette: Palette {
            background: rgb(30, 30, 46),
            panel: rgb(49, 50, 68),
            panel_alt: rgb(69, 71, 90),
            foreground: rgb(205, 214, 244),
            muted: rgb(166, 173, 200),
            border: rgb(88, 91, 112),
            accent: rgb(203, 166, 247),
            accent_alt: rgb(245, 194, 231),
            warning: rgb(249, 226, 175),
            danger: rgb(243, 139, 168),
            success: rgb(166, 227, 161),
            info: rgb(137, 220, 235),
            code_background: rgb(24, 24, 37),
            selected_background: rgb(203, 166, 247),
            selected_foreground: rgb(30, 30, 46),
        },
    },
    ThemeDefinition {
        theme: Theme::CatppuccinLatte,
        id: "catppuccin-latte",
        name: "Catppuccin Latte",
        is_light: true,
        palette: Palette {
            background: rgb(239, 241, 245),
            panel: rgb(230, 233, 239),
            panel_alt: rgb(220, 224, 232),
            foreground: rgb(76, 79, 105),
            muted: rgb(91, 94, 115),
            border: rgb(156, 160, 176),
            accent: rgb(136, 57, 239),
            accent_alt: rgb(173, 50, 137),
            warning: rgb(150, 90, 0),
            danger: rgb(210, 15, 57),
            success: rgb(43, 110, 27),
            info: rgb(0, 110, 150),
            code_background: rgb(232, 235, 241),
            selected_background: rgb(136, 57, 239),
            selected_foreground: rgb(239, 241, 245),
        },
    },
    ThemeDefinition {
        theme: Theme::SolarizedDark,
        id: "solarized-dark",
        name: "Solarized Dark",
        is_light: false,
        palette: Palette {
            background: rgb(0, 43, 54),
            panel: rgb(7, 54, 66),
            panel_alt: rgb(19, 73, 83),
            foreground: rgb(147, 161, 161),
            muted: rgb(147, 161, 161),
            border: rgb(45, 105, 117),
            accent: rgb(42, 161, 152),
            accent_alt: rgb(108, 113, 196),
            warning: rgb(181, 137, 0),
            danger: rgb(220, 50, 47),
            success: rgb(133, 153, 0),
            info: rgb(80, 170, 230),
            code_background: rgb(0, 35, 44),
            selected_background: rgb(42, 161, 152),
            selected_foreground: rgb(0, 43, 54),
        },
    },
    ThemeDefinition {
        theme: Theme::SolarizedLight,
        id: "solarized-light",
        name: "Solarized Light",
        is_light: true,
        palette: Palette {
            background: rgb(253, 246, 227),
            panel: rgb(238, 232, 213),
            panel_alt: rgb(226, 218, 194),
            foreground: rgb(65, 85, 90),
            muted: rgb(65, 85, 90),
            border: rgb(147, 161, 161),
            accent: rgb(0, 90, 140),
            accent_alt: rgb(108, 113, 196),
            warning: rgb(125, 95, 0),
            danger: rgb(220, 50, 47),
            success: rgb(85, 100, 0),
            info: rgb(0, 105, 99),
            code_background: rgb(246, 239, 220),
            selected_background: rgb(0, 90, 140),
            selected_foreground: rgb(253, 246, 227),
        },
    },
    ThemeDefinition {
        theme: Theme::RosePine,
        id: "rose-pine",
        name: "Rosé Pine",
        is_light: false,
        palette: Palette {
            background: rgb(25, 23, 36),
            panel: rgb(38, 35, 58),
            panel_alt: rgb(47, 44, 69),
            foreground: rgb(224, 222, 244),
            muted: rgb(144, 140, 170),
            border: rgb(86, 82, 110),
            accent: rgb(196, 167, 231),
            accent_alt: rgb(235, 111, 146),
            warning: rgb(246, 193, 119),
            danger: rgb(235, 111, 146),
            success: rgb(156, 207, 216),
            info: rgb(49, 116, 143),
            code_background: rgb(31, 29, 46),
            selected_background: rgb(196, 167, 231),
            selected_foreground: rgb(25, 23, 36),
        },
    },
];

pub fn definition(theme: Theme) -> &'static ThemeDefinition {
    CATALOG
        .iter()
        .find(|entry| entry.theme == theme)
        .expect("each Theme variant must have one catalog entry")
}

pub fn find(id: &str) -> Option<Theme> {
    CATALOG
        .iter()
        .find(|entry| entry.id == id)
        .map(|entry| entry.theme)
}

pub fn status_color(theme: Theme, status: &str) -> Color {
    let p = definition(theme).palette;
    match status {
        "open" => p.success,
        "closed" | "blocked" => p.danger,
        "in_progress" => p.warning,
        "deferred" => p.info,
        "unknown" | "" => p.muted,
        _ => p.accent_alt,
    }
}

pub fn worktree_status_color(theme: Theme, status: WorktreeStatus) -> Color {
    let p = definition(theme).palette;
    match status {
        WorktreeStatus::Reserved => p.info,
        WorktreeStatus::Unmerged => p.info,
        WorktreeStatus::MergeRequested => p.warning,
        WorktreeStatus::NeedsResolution | WorktreeStatus::NeedsAttention => p.danger,
        WorktreeStatus::Integrated => p.success,
        WorktreeStatus::WorkspaceClosed => p.accent_alt,
        WorktreeStatus::CheckoutRemoved => p.muted,
    }
}

pub fn issue_type_color(theme: Theme, kind: &str) -> Color {
    let p = definition(theme).palette;
    match kind.to_ascii_lowercase().as_str() {
        "task" => p.success,
        "bug" => p.danger,
        "feature" | "story" => p.info,
        "epic" | "decision" => p.accent_alt,
        "chore" | "unknown" => p.muted,
        "spike" => p.warning,
        "milestone" => p.accent,
        _ => p.muted,
    }
}

pub fn priority_color(theme: Theme, priority: u64) -> Color {
    let p = definition(theme).palette;
    match priority {
        0 => p.danger,
        1 | 2 => p.warning,
        3 => p.success,
        4 => p.muted,
        _ => p.muted,
    }
}

/// Re-map the legacy markdown renderer's semantic colors into the active theme.
pub fn markdown_foreground(theme: Theme, color: Color) -> Color {
    let p = definition(theme).palette;
    match color {
        Color::Reset | Color::White | Color::Rgb(220, 228, 229) => p.foreground,
        Color::Black | Color::Rgb(13, 23, 28) => p.background,
        Color::DarkGray | Color::Gray | Color::Rgb(123, 144, 148) => p.muted,
        Color::Rgb(73, 211, 182) => p.accent,
        Color::Rgb(239, 183, 99) => p.warning,
        Color::Rgb(255, 123, 123) => p.danger,
        Color::Indexed(1 | 9) => p.danger,
        Color::Indexed(2 | 10) => p.success,
        Color::Indexed(3 | 11) => p.warning,
        Color::Indexed(4 | 12) => p.info,
        Color::Indexed(5 | 13) => p.accent_alt,
        Color::Indexed(6 | 14) => p.accent,
        Color::Indexed(_) => p.muted,
        other => other,
    }
}

pub fn markdown_background(theme: Theme, color: Color) -> Color {
    let p = definition(theme).palette;
    match color {
        Color::Reset => p.background,
        Color::Black | Color::DarkGray | Color::Rgb(13, 23, 28) => p.code_background,
        Color::Rgb(232, 239, 237) => p.panel,
        other => markdown_foreground(theme, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color) -> f64 {
        let Color::Rgb(r, g, b) = color else {
            panic!("theme palette should use RGB colors")
        };
        let linear = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }

    fn contrast(left: Color, right: Color) -> f64 {
        let (lighter, darker) = {
            let left = luminance(left);
            let right = luminance(right);
            if left > right {
                (left, right)
            } else {
                (right, left)
            }
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn catalog_ids_are_stable_unique_and_preserve_legacy_names() {
        let mut ids = CATALOG.iter().map(|entry| entry.id).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
        assert_eq!(
            Theme::ALL,
            CATALOG.map(|entry| entry.theme),
            "Theme::ALL must track the catalog exactly"
        );
        assert_eq!(find("dark"), Some(Theme::Dark));
        assert_eq!(find("light"), Some(Theme::Light));
        for entry in CATALOG {
            assert!(
                entry
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
                "theme ID must be lowercase kebab-case: {}",
                entry.id
            );
            assert_eq!(find(entry.id), Some(entry.theme));
        }
    }

    #[test]
    fn every_palette_has_readable_text_selection_and_semantic_accents() {
        for entry in CATALOG {
            let p = entry.palette;
            assert!(
                contrast(p.foreground, p.background) >= 4.5,
                "{} foreground contrast too low",
                entry.id
            );
            assert!(
                contrast(p.muted, p.background) >= 4.5,
                "{} muted contrast too low",
                entry.id
            );
            assert!(
                contrast(p.foreground, p.panel) >= 4.5,
                "{} panel text contrast too low",
                entry.id
            );
            assert!(
                contrast(p.selected_foreground, p.selected_background) >= 4.5,
                "{} selected text contrast too low",
                entry.id
            );
            for (role, color) in [
                ("accent", p.accent),
                ("secondary accent", p.accent_alt),
                ("warning", p.warning),
                ("danger", p.danger),
                ("success", p.success),
                ("info", p.info),
            ] {
                assert!(
                    contrast(color, p.background) >= 3.0,
                    "{} {role} contrast too low",
                    entry.id
                );
            }
        }
    }

    #[test]
    fn worktree_statuses_keep_pending_and_attention_distinct_from_integration() {
        for theme in Theme::ALL {
            let color = |status| worktree_status_color(theme, status);
            assert_ne!(
                color(WorktreeStatus::MergeRequested),
                color(WorktreeStatus::Integrated)
            );
            assert_ne!(
                color(WorktreeStatus::NeedsResolution),
                color(WorktreeStatus::Integrated)
            );
            assert_ne!(
                color(WorktreeStatus::NeedsAttention),
                color(WorktreeStatus::Integrated)
            );
            for status in [
                WorktreeStatus::Unmerged,
                WorktreeStatus::MergeRequested,
                WorktreeStatus::NeedsResolution,
                WorktreeStatus::Integrated,
                WorktreeStatus::WorkspaceClosed,
                WorktreeStatus::CheckoutRemoved,
                WorktreeStatus::NeedsAttention,
            ] {
                assert!(matches!(color(status), Color::Rgb(_, _, _)));
            }
        }
    }
}
