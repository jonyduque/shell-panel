use std::borrow::Cow;
use vt100::Parser;

/// Extension trait for [`vt100::Cell`] providing ghost text / dim attribute detection.
pub trait CellExt {
    /// Returns true if the cell has dim / faint or dull gray appearance (e.g. PSReadLine inline prediction).
    fn dim(&self) -> bool;
}

impl CellExt for vt100::Cell {
    fn dim(&self) -> bool {
        match self.fgcolor() {
            vt100::Color::Idx(8) | vt100::Color::Idx(242) => true,
            vt100::Color::Rgb(r, g, b) => r == g && b == g && r > 0 && r < 160,
            _ => false,
        }
    }
}

/// Headless VT100 terminal emulator wrapper for tracking shell display state,
/// cursor position, and extracting typed command line text while filtering ghost text.
pub struct HeadlessTerminal {
    parser: Parser,
    pub cols: u16,
    pub rows: u16,
}

impl HeadlessTerminal {
    /// Initializes a new headless VT100 terminal emulator with specified columns and rows.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            parser: Parser::new(rows, cols, 0),
            cols,
            rows,
        }
    }

    /// Feeds raw terminal output bytes into the VT100 parser.
    pub fn process(&mut self, bytes: &[u8]) {
        let processed = preprocess_vt_bytes(bytes);
        self.parser.process(&processed);
    }

    /// Returns current cursor position as `(col, row)` (0-indexed).
    pub fn cursor_position(&self) -> (u16, u16) {
        let screen = self.parser.screen();
        let (row, col) = screen.cursor_position();
        (col, row)
    }

    /// Returns true if the terminal is currently on the alternate screen buffer.
    pub fn is_alternate_buffer(&self) -> bool {
        self.parser.screen().alternate_screen()
    }

    /// Resizes the terminal screen buffer.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.parser.set_size(rows, cols);
    }

    /// Exposes the underlying VT100 screen buffer for cell inspection.
    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    /// Extracts typed command text starting from `(prompt_row, prompt_end_x)`
    /// up to the current cursor position `(cursor_col, cursor_row)`.
    ///
    /// Filters out ghost text (dim or italic suggestions from PSReadLine).
    /// Preserves spaces where cell contents are empty.
    /// Trims trailing whitespace.
    pub fn extract_command_text(&self, prompt_row: u16, prompt_end_x: u16) -> String {
        let screen = self.parser.screen();
        let (cursor_col, cursor_row) = self.cursor_position();

        if cursor_row < prompt_row {
            return String::new();
        }
        if cursor_row == prompt_row && cursor_col <= prompt_end_x {
            return String::new();
        }

        let mut result = String::new();
        for row in prompt_row..=cursor_row {
            let start_col = if row == prompt_row { prompt_end_x } else { 0 };
            let end_col = if row == cursor_row { cursor_col } else { self.cols };

            for col in start_col..end_col {
                if let Some(cell) = screen.cell(row, col) {
                    if cell.is_wide_continuation() {
                        continue;
                    }
                    // Filter out PSReadLine inline prediction ghost text (dim or italic)
                    let is_ghost = cell.dim() || cell.italic();
                    if !is_ghost {
                        let ch = cell.contents();
                        if !ch.is_empty() {
                            result.push_str(&ch);
                        } else {
                            result.push(' ');
                        }
                    }
                }
            }
        }

        result.trim_end().to_string()
    }
}

/// Translates SGR 2 (dim / faint) to SGR 90 (bright black / dark gray) and
/// SGR 22 (normal intensity) to SGR 39 (default fgcolor), because `vt100` crate
/// does not natively record SGR 2 in its cell attributes.
fn preprocess_vt_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    if !bytes.contains(&0x1b) {
        return Cow::Borrowed(bytes);
    }
    if !bytes.windows(4).any(|w| w == b"\x1b[2m" || w == b";2m" || w == b"[2;" || w == b";2;")
        && !bytes.windows(5).any(|w| w == b"\x1b[22m")
    {
        return Cow::Borrowed(bytes);
    }

    let mut out = Vec::with_capacity(bytes.len() + 16);
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"\x1b[2m") {
            out.extend_from_slice(b"\x1b[90m");
            i += 4;
        } else if bytes[i..].starts_with(b"\x1b[22m") {
            out.extend_from_slice(b"\x1b[39m");
            i += 5;
        } else if bytes[i..].starts_with(b";2m") {
            out.extend_from_slice(b";90m");
            i += 3;
        } else if bytes[i..].starts_with(b"[2;") {
            out.extend_from_slice(b"[90;");
            i += 3;
        } else if bytes[i..].starts_with(b";2;") {
            out.extend_from_slice(b";90;");
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Cow::Owned(out)
}
