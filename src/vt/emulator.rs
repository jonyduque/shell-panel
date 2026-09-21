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
    /// Preserves user-typed spaces (including trailing spaces before cursor).
    /// Stops collecting and filters out PSReadLine ghost text (dim or italic suggestions).
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
        'rows: for row in prompt_row..=cursor_row {
            let start_col = if row == prompt_row { prompt_end_x } else { 0 };
            let end_col = if row == cursor_row { cursor_col } else { self.cols };

            for col in start_col..end_col {
                if let Some(cell) = screen.cell(row, col) {
                    if cell.is_wide_continuation() {
                        continue;
                    }
                    // Filter out PSReadLine inline prediction ghost text (dim or italic)
                    let is_ghost = cell.dim() || cell.italic();
                    if is_ghost {
                        // Once ghost text begins, stop accumulating command text
                        break 'rows;
                    }
                    let ch = cell.contents();
                    if !ch.is_empty() {
                        result.push_str(&ch);
                    } else {
                        result.push(' ');
                    }
                }
            }
        }

        // If the extracted text contains a shell prompt marker (e.g. '❯' from Oh-My-Posh / Starship
        // or '>' from standard PowerShell 'PS C:\...>'), isolate the command line after the last marker.
        if let Some(pos) = result.rfind('❯') {
            return result[pos + '❯'.len_utf8()..].trim_start().to_string();
        }
        if let Some(pos) = result.rfind('>') {
            return result[pos + 1..].trim_start().to_string();
        }

        result
    }
}

/// CSI SGR-aware preprocessor for terminal byte sequences.
///
/// Only modifies SGR sequences (`\x1b[...m`), rewriting standalone parameter `2`
/// (faint/dim) to `90` (bright black) so `vt100` records it in its cell state.
/// Safely ignores cursor position (`\x1b[2;10H`), RGB (`\x1b[38;2;...m`),
/// 256-color palette (`\x1b[38;5;2m`), and plain text.
pub fn preprocess_vt_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    if !bytes.contains(&0x1b) {
        return Cow::Borrowed(bytes);
    }

    let mut out = Vec::with_capacity(bytes.len() + 16);
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i..].starts_with(b"\x1b[") {
            // Find the end of the CSI sequence (final byte in 0x40..=0x7E)
            let start = i;
            let mut j = i + 2;
            while j < bytes.len() && (0x20..=0x3f).contains(&bytes[j]) {
                j += 1;
            }

            if j < bytes.len() {
                let final_byte = bytes[j];
                let csi_body = &bytes[i + 2..j];

                if final_byte == b'm' {
                    // SGR sequence: parse numeric parameters separated by ';'
                    if let Ok(param_str) = std::str::from_utf8(csi_body) {
                        let mut params: Vec<&str> = param_str.split(';').collect();
                        let mut modified = false;
                        let mut skip_count = 0;

                        for idx in 0..params.len() {
                            if skip_count > 0 {
                                skip_count -= 1;
                                continue;
                            }
                            let p = params[idx];
                            if p == "38" || p == "48" {
                                if let Some(&next_p) = params.get(idx + 1) {
                                    if next_p == "2" {
                                        // 38;2;r;g;b -> skip next 4 parameters (2, r, g, b)
                                        skip_count = 4;
                                        continue;
                                    } else if next_p == "5" {
                                        // 38;5;idx -> skip next 2 parameters (5, idx)
                                        skip_count = 2;
                                        continue;
                                    }
                                }
                            }
                            if p == "2" {
                                params[idx] = "90";
                                modified = true;
                            }
                        }

                        if modified {
                            out.extend_from_slice(b"\x1b[");
                            out.extend_from_slice(params.join(";").as_bytes());
                            out.push(b'm');
                            i = j + 1;
                            continue;
                        }
                    }
                }

                // Unmodified CSI sequence
                out.extend_from_slice(&bytes[start..=j]);
                i = j + 1;
                continue;
            }
        }

        out.push(bytes[i]);
        i += 1;
    }

    Cow::Owned(out)
}
