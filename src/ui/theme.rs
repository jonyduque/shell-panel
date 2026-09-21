use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::core::config::{Config, IconConfig};
use crate::engine::provider::{Suggestion, SuggestionKind};
use crate::ui::color::{parse_color_bg, parse_color_fg};

/// Prefix string displayed before the active/selected suggestion item.
pub const SELECTED_PREFIX: &str = "> ";

/// Prefix string displayed before unselected suggestion items.
pub const UNSELECTED_PREFIX: &str = "  ";

/// UI theme styling definitions and helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub selected_start: String,
    pub selected_end: String,
    pub desc_start: String,
    pub desc_end: String,
    pub unselected_fg_start: String,
    pub selected_prefix: String,
    pub unselected_prefix: String,
    pub icons: IconConfig,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            selected_start: "\x1b[7m".to_string(),
            selected_end: "\x1b[0m".to_string(),
            desc_start: "\x1b[90m".to_string(),
            desc_end: "\x1b[0m".to_string(),
            unselected_fg_start: String::new(),
            selected_prefix: SELECTED_PREFIX.to_string(),
            unselected_prefix: UNSELECTED_PREFIX.to_string(),
            icons: IconConfig::default(),
        }
    }
}

impl Theme {
    /// Prefix string displayed before the active/selected suggestion item.
    pub const SELECTED_PREFIX: &'static str = SELECTED_PREFIX;

    /// Prefix string displayed before unselected suggestion items.
    pub const UNSELECTED_PREFIX: &'static str = UNSELECTED_PREFIX;

    /// Formats a line or text segment with reverse video (invert) ANSI escape sequence.
    pub fn format_selected(text: &str) -> String {
        format!("\x1b[7m{}\x1b[0m", text)
    }

    /// Formats description text with dim / faint bright black ANSI escape sequence.
    pub fn format_description(text: &str) -> String {
        format!("\x1b[90m{}\x1b[0m", text)
    }

    /// Constructs a `Theme` from loaded `Config`.
    pub fn from_config(config: &Config) -> Self {
        let is_invert = config.colors.selected_bg.eq_ignore_ascii_case("invert")
            || config.colors.selected_bg.eq_ignore_ascii_case("reverse")
            || config.colors.selected_fg.eq_ignore_ascii_case("invert")
            || config.colors.selected_fg.eq_ignore_ascii_case("reverse");

        let selected_start = if is_invert {
            "\x1b[7m".to_string()
        } else {
            let bg = parse_color_bg(&config.colors.selected_bg);
            let fg = parse_color_fg(&config.colors.selected_fg);
            match (bg, fg) {
                (Some(b), Some(f)) => format!("{}{}", b, f),
                (Some(b), None) => b,
                (None, Some(f)) => f,
                (None, None) => "\x1b[7m".to_string(),
            }
        };

        let desc_start = parse_color_fg(&config.colors.description_fg)
            .unwrap_or_else(|| "\x1b[90m".to_string());

        let unselected_fg_start = parse_color_fg(&config.colors.unselected_fg).unwrap_or_default();

        Self {
            selected_start,
            selected_end: "\x1b[0m".to_string(),
            desc_start,
            desc_end: "\x1b[0m".to_string(),
            unselected_fg_start,
            selected_prefix: config.colors.selected_prefix.clone(),
            unselected_prefix: config.colors.unselected_prefix.clone(),
            icons: config.icons.clone(),
        }
    }

    /// Looks up the configured icon for a `SuggestionKind`.
    pub fn icon_for(&self, kind: SuggestionKind) -> &str {
        match kind {
            SuggestionKind::Directory => &self.icons.directory,
            SuggestionKind::File => &self.icons.file,
            SuggestionKind::Command => &self.icons.command,
            SuggestionKind::Subcommand => &self.icons.subcommand,
            SuggestionKind::Option => &self.icons.option,
            SuggestionKind::PowerShellCmdlet => &self.icons.powershell_cmdlet,
            SuggestionKind::Alias => &self.icons.alias,
            SuggestionKind::Other => &self.icons.other,
        }
    }

    /// Formats selected text using this theme's selected styling.
    pub fn format_selected_line(&self, text: &str) -> String {
        format!("{}{}{}", self.selected_start, text, self.selected_end)
    }

    /// Formats description text using this theme's description styling.
    pub fn format_description_text(&self, text: &str) -> String {
        format!("{}{}{}", self.desc_start, text, self.desc_end)
    }
}

/// Formats a line or text segment with reverse video (invert) ANSI escape sequence.
pub fn format_selected(text: &str) -> String {
    Theme::format_selected(text)
}

/// Formats description text with dim / faint bright black ANSI escape sequence.
pub fn format_description(text: &str) -> String {
    Theme::format_description(text)
}

/// Formats a suggestion into a styled, aligned, padded/truncated line using the default theme.
pub fn format_suggestion_line(sug: &Suggestion, selected: bool, max_width: usize) -> String {
    let theme = Theme::default();
    format_suggestion_line_with_theme(sug, selected, max_width, &theme)
}

/// Formats a suggestion with custom minimum and maximum column widths using the default theme.
pub fn format_suggestion_line_with_min_width(
    sug: &Suggestion,
    selected: bool,
    min_width: usize,
    max_width: usize,
) -> String {
    let theme = Theme::default();
    format_suggestion_line_with_theme_and_min_width(sug, &theme, selected, min_width, max_width)
}

/// Formats a suggestion into a styled, aligned line using a specific theme.
pub fn format_suggestion_line_with_theme(
    sug: &Suggestion,
    selected: bool,
    max_width: usize,
    theme: &Theme,
) -> String {
    format_suggestion_line_with_theme_and_min_width(sug, theme, selected, 30.min(max_width), max_width)
}

/// Formats a suggestion with custom minimum and maximum column widths using a specific theme.
pub fn format_suggestion_line_with_theme_and_min_width(
    sug: &Suggestion,
    theme: &Theme,
    selected: bool,
    min_width: usize,
    max_width: usize,
) -> String {
    if max_width == 0 {
        return String::new();
    }

    let prefix = if selected {
        &theme.selected_prefix
    } else {
        &theme.unselected_prefix
    };
    let icon = theme.icon_for(sug.kind);
    let label = format!("{}{}{}", prefix, icon, sug.display);
    let label_width = label.as_str().width();

    if selected {
        let mut plain = label;
        if let Some(desc) = &sug.description {
            plain.push_str("  ");
            plain.push_str(desc);
        }

        let plain_width = plain.as_str().width();
        if plain_width < min_width {
            plain.push_str(&" ".repeat(min_width - plain_width));
        }
        let truncated = truncate_to_width(&plain, max_width);
        format!("{}{}{}", theme.selected_start, truncated, theme.selected_end)
    } else if let Some(desc) = &sug.description {
        let desc_with_gap = format!("  {}", desc);
        let desc_width = desc_with_gap.as_str().width();
        let total_width = label_width + desc_width;

        if total_width <= max_width {
            let padding = if total_width < min_width {
                " ".repeat(min_width - total_width)
            } else {
                String::new()
            };
            if theme.unselected_fg_start.is_empty() {
                format!(
                    "{}{}{}{}{}",
                    label,
                    theme.desc_start,
                    desc_with_gap,
                    theme.desc_end,
                    padding
                )
            } else {
                format!(
                    "{}{}\x1b[0m{}{}{}\x1b[0m{}{}\x1b[0m",
                    theme.unselected_fg_start,
                    label,
                    theme.desc_start,
                    desc_with_gap,
                    theme.desc_end,
                    theme.unselected_fg_start,
                    padding
                )
            }
        } else if label_width < max_width {
            let allowed_desc = max_width - label_width;
            let truncated_desc = truncate_to_width(&desc_with_gap, allowed_desc);
            if theme.unselected_fg_start.is_empty() {
                format!(
                    "{}{}{}{}",
                    label,
                    theme.desc_start,
                    truncated_desc,
                    theme.desc_end
                )
            } else {
                format!(
                    "{}{}\x1b[0m{}{}{}",
                    theme.unselected_fg_start,
                    label,
                    theme.desc_start,
                    truncated_desc,
                    theme.desc_end
                )
            }
        } else {
            let truncated = truncate_to_width(&label, max_width);
            if theme.unselected_fg_start.is_empty() {
                truncated
            } else {
                format!("{}{}\x1b[0m", theme.unselected_fg_start, truncated)
            }
        }
    } else {
        let mut plain = label;
        let p_width = plain.as_str().width();
        if p_width < min_width {
            plain.push_str(&" ".repeat(min_width - p_width));
        }
        let truncated = truncate_to_width(&plain, max_width);
        if theme.unselected_fg_start.is_empty() {
            truncated
        } else {
            format!("{}{}\x1b[0m", theme.unselected_fg_start, truncated)
        }
    }
}

/// Truncates string so that its visible unicode column width does not exceed `max_width`.
pub fn truncate_to_width(s: &str, max_width: usize) -> String {
    let mut current_width = 0;
    let mut out = String::new();

    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if current_width + w > max_width {
            break;
        }
        out.push(c);
        current_width += w;
    }

    out
}
