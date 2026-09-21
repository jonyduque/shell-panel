use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::engine::provider::Suggestion;

/// Prefix string displayed before the active/selected suggestion item.
pub const SELECTED_PREFIX: &str = "> ";

/// Prefix string displayed before unselected suggestion items.
pub const UNSELECTED_PREFIX: &str = "  ";

/// UI theme styling definitions and helpers.
pub struct Theme;

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
}

/// Formats a line or text segment with reverse video (invert) ANSI escape sequence.
pub fn format_selected(text: &str) -> String {
    Theme::format_selected(text)
}

/// Formats description text with dim / faint bright black ANSI escape sequence.
pub fn format_description(text: &str) -> String {
    Theme::format_description(text)
}

/// Formats a suggestion into a styled, aligned, padded/truncated line.
pub fn format_suggestion_line(sug: &Suggestion, selected: bool, max_width: usize) -> String {
    format_suggestion_line_with_min_width(sug, selected, 30.min(max_width), max_width)
}

/// Formats a suggestion with custom minimum and maximum column widths.
pub fn format_suggestion_line_with_min_width(
    sug: &Suggestion,
    selected: bool,
    min_width: usize,
    max_width: usize,
) -> String {
    if max_width == 0 {
        return String::new();
    }

    let prefix = if selected {
        SELECTED_PREFIX
    } else {
        UNSELECTED_PREFIX
    };
    let icon = sug.kind.icon();
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
        Theme::format_selected(&truncated)
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
            format!(
                "{}{}{}",
                label,
                Theme::format_description(&desc_with_gap),
                padding
            )
        } else if label_width < max_width {
            let allowed_desc = max_width - label_width;
            let truncated_desc = truncate_to_width(&desc_with_gap, allowed_desc);
            format!("{}{}", label, Theme::format_description(&truncated_desc))
        } else {
            truncate_to_width(&label, max_width)
        }
    } else {
        let mut plain = label;
        let p_width = plain.as_str().width();
        if p_width < min_width {
            plain.push_str(&" ".repeat(min_width - p_width));
        }
        truncate_to_width(&plain, max_width)
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
