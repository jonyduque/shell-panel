use std::io::Write;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::ui::patch;
use crate::ui::suggestion_state::SuggestionState;
use crate::ui::theme::Theme;
use crate::vt::emulator::HeadlessTerminal;

/// Represents the vertical on-screen layout of an active dropdown menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropdownLayout {
    pub start_row: u16,
    pub row_count: u16,
}

/// Renderer responsible for drawing and clearing the floating suggestion panel
/// without corrupting the underlying terminal screen buffer.
pub struct Renderer;

impl Renderer {
    /// Renders the current visible suggestions page as a floating dropdown panel.
    /// Returns the active `DropdownLayout` if rendered, or `None` if invisible/empty.
    pub fn render_dropdown<W: Write>(
        state: &SuggestionState,
        term: &HeadlessTerminal,
        cursor_x: u16,
        cursor_y: u16,
        out: &mut W,
    ) -> std::io::Result<Option<DropdownLayout>> {
        if !state.visible || state.total_items() == 0 {
            return Ok(None);
        }

        let page = state.visible_page();
        if page.is_empty() {
            return Ok(None);
        }

        let page_len = page.len() as u16;

        // Determine dropdown vertical placement
        let start_row = if cursor_y + 1 + page_len <= term.rows {
            // Render below cursor
            cursor_y + 1
        } else if cursor_y >= page_len {
            // Render above cursor
            cursor_y - page_len
        } else {
            // Render below clamped to terminal boundary
            (cursor_y + 1).min(term.rows.saturating_sub(page_len))
        };

        let col = (cursor_x + 1).min(term.cols);
        let available_cols = (term.cols as usize).saturating_sub(col as usize - 1);
        let target_min_width = 30.min(available_cols);

        // Hide cursor and save cursor position
        write!(out, "\x1b[?25l\x1b[s")?;

        for (i, (sug, selected)) in page.iter().enumerate() {
            let target_row = start_row + i as u16;
            if target_row >= term.rows {
                break;
            }

            let prefix = if *selected {
                Theme::SELECTED_PREFIX
            } else {
                Theme::UNSELECTED_PREFIX
            };

            let styled_line = format_suggestion_line(
                prefix,
                &sug.display,
                sug.description.as_deref(),
                *selected,
                target_min_width,
                available_cols,
            );

            // Move cursor to (target_row + 1, col) and write line
            write!(out, "\x1b[{};{}H{}", target_row + 1, col, styled_line)?;
        }

        // Restore cursor position and unhide cursor
        write!(out, "\x1b[u\x1b[?25h")?;
        out.flush()?;

        Ok(Some(DropdownLayout {
            start_row,
            row_count: page_len,
        }))
    }

    /// Clears the rendered dropdown lines by restoring original terminal screen cells.
    pub fn clear_dropdown<W: Write>(
        layout: &DropdownLayout,
        term: &HeadlessTerminal,
        out: &mut W,
    ) -> std::io::Result<()> {
        // Hide cursor and save position
        write!(out, "\x1b[?25l\x1b[s")?;

        let end_row = (layout.start_row + layout.row_count).min(term.rows);
        for r in layout.start_row..end_row {
            let restored = patch::restore_line(r, term);
            write!(out, "{}", restored)?;
        }

        // Restore cursor and unhide
        write!(out, "\x1b[u\x1b[?25h")?;
        out.flush()?;

        Ok(())
    }
}

/// Helper function to format, style, and pad/truncate a suggestion line.
fn format_suggestion_line(
    prefix: &str,
    display: &str,
    description: Option<&str>,
    selected: bool,
    min_width: usize,
    max_width: usize,
) -> String {
    if max_width == 0 {
        return String::new();
    }

    if selected {
        let mut plain = format!("{}{}", prefix, display);
        if let Some(desc) = description {
            plain.push_str("  ");
            plain.push_str(desc);
        }

        let plain_width = plain.as_str().width();
        if plain_width < min_width {
            plain.push_str(&" ".repeat(min_width - plain_width));
        }
        let truncated = truncate_to_width(&plain, max_width);
        Theme::format_selected(&truncated)
    } else {
        let prefix_display = format!("{}{}", prefix, display);
        let pd_width = prefix_display.as_str().width();

        if let Some(desc) = description {
            let desc_with_gap = format!("  {}", desc);
            let desc_width = desc_with_gap.as_str().width();
            let total_width = pd_width + desc_width;

            if total_width <= max_width {
                let padding = if total_width < min_width {
                    " ".repeat(min_width - total_width)
                } else {
                    String::new()
                };
                format!(
                    "{}{}{}",
                    prefix_display,
                    Theme::format_description(&desc_with_gap),
                    padding
                )
            } else if pd_width < max_width {
                let allowed_desc = max_width - pd_width;
                let truncated_desc = truncate_to_width(&desc_with_gap, allowed_desc);
                format!("{}{}", prefix_display, Theme::format_description(&truncated_desc))
            } else {
                truncate_to_width(&prefix_display, max_width)
            }
        } else {
            let mut plain = prefix_display;
            let p_width = plain.as_str().width();
            if p_width < min_width {
                plain.push_str(&" ".repeat(min_width - p_width));
            }
            truncate_to_width(&plain, max_width)
        }
    }
}

/// Truncates string so that its visible unicode column width does not exceed `max_width`.
fn truncate_to_width(s: &str, max_width: usize) -> String {
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

/// Top-level helper function forwarding to `Renderer::render_dropdown`.
pub fn render_dropdown<W: Write>(
    state: &SuggestionState,
    term: &HeadlessTerminal,
    cursor_x: u16,
    cursor_y: u16,
    out: &mut W,
) -> std::io::Result<Option<DropdownLayout>> {
    Renderer::render_dropdown(state, term, cursor_x, cursor_y, out)
}

/// Top-level helper function forwarding to `Renderer::clear_dropdown`.
pub fn clear_dropdown<W: Write>(
    layout: &DropdownLayout,
    term: &HeadlessTerminal,
    out: &mut W,
) -> std::io::Result<()> {
    Renderer::clear_dropdown(layout, term, out)
}
