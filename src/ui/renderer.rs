use std::io::Write;

use crate::ui::patch;
use crate::ui::suggestion_state::SuggestionState;
use crate::ui::theme::{format_suggestion_line_with_theme_and_min_width, Theme};
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
        theme: &Theme,
        cursor_x: u16,
        cursor_y: u16,
        out: &mut W,
    ) -> std::io::Result<Option<DropdownLayout>> {
        if term.cols == 0 || term.rows == 0 || !state.visible || state.total_items() == 0 {
            return Ok(None);
        }

        let page = state.visible_page();
        if page.is_empty() {
            return Ok(None);
        }

        let page_len = page.len() as u16;

        // Determine dropdown vertical placement
        let start_row = if cursor_y.saturating_add(1).saturating_add(page_len) <= term.rows {
            // Render below cursor
            cursor_y.saturating_add(1)
        } else if cursor_y >= page_len {
            // Render above cursor
            cursor_y.saturating_sub(page_len)
        } else {
            // Render below clamped to terminal boundary
            cursor_y.saturating_add(1).min(term.rows.saturating_sub(page_len))
        };

        let col = (cursor_x + 1).min(term.cols);
        let available_cols = (term.cols as usize).saturating_sub((col as usize).saturating_sub(1));
        let min_width = 30.min(available_cols);

        // Hide cursor and save cursor position
        write!(out, "\x1b[?25l\x1b[s")?;

        let mut rendered_rows = 0u16;
        for (i, (sug, selected)) in page.iter().enumerate() {
            let target_row = start_row + i as u16;
            if target_row >= term.rows {
                break;
            }
            rendered_rows += 1;

            let styled_line = format_suggestion_line_with_theme_and_min_width(
                sug,
                theme,
                *selected,
                min_width,
                term.cols as usize,
            );

            // Move cursor to (target_row + 1, col) and write line
            write!(out, "\x1b[{};{}H{}", target_row + 1, col, styled_line)?;
        }

        // Restore cursor position and unhide cursor
        write!(out, "\x1b[u\x1b[?25h")?;
        out.flush()?;

        Ok(Some(DropdownLayout {
            start_row,
            row_count: rendered_rows,
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

/// Top-level helper function forwarding to `Renderer::render_dropdown`.
pub fn render_dropdown<W: Write>(
    state: &SuggestionState,
    term: &HeadlessTerminal,
    theme: &Theme,
    cursor_x: u16,
    cursor_y: u16,
    out: &mut W,
) -> std::io::Result<Option<DropdownLayout>> {
    Renderer::render_dropdown(state, term, theme, cursor_x, cursor_y, out)
}

/// Top-level helper function forwarding to `Renderer::clear_dropdown`.
pub fn clear_dropdown<W: Write>(
    layout: &DropdownLayout,
    term: &HeadlessTerminal,
    out: &mut W,
) -> std::io::Result<()> {
    Renderer::clear_dropdown(layout, term, out)
}
