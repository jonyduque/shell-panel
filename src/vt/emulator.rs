use vt100::Parser;

/// Headless VT100 terminal emulator wrapper: mirrors the shell's screen so shell-panel knows the
/// cursor position, the screen contents under the dropdown and whether the alternate buffer is on.
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
        self.parser.process(bytes);
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
}
