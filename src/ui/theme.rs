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
