use crate::vt::emulator::HeadlessTerminal;

/// Represents an on-screen line patch to restore or draw terminal text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinePatch {
    pub row: u16,
    pub col: u16,
    pub content: String,
}

/// Reconstructs the terminal's underlying line at `row` and returns an ANSI
/// sequence that moves to the start of the line, clears it, and writes back the original text.
pub fn restore_line(row: u16, term: &HeadlessTerminal) -> String {
    let screen = term.screen();
    let mut reconstructed = String::new();

    for col in 0..term.cols {
        if let Some(cell) = screen.cell(row, col) {
            if cell.is_wide_continuation() {
                continue;
            }
            let ch = cell.contents();
            if !ch.is_empty() {
                reconstructed.push_str(&ch);
            } else {
                reconstructed.push(' ');
            }
        } else {
            reconstructed.push(' ');
        }
    }

    let reconstructed_text = reconstructed.trim_end();
    format!("\x1b[{};1H\x1b[2K{}", row + 1, reconstructed_text)
}
