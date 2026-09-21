use crate::vt::emulator::HeadlessTerminal;

/// Returns an ANSI sequence that moves to the start of `row`, clears it and redraws the row
/// exactly as the headless terminal holds it, colors and attributes included.
pub fn restore_line(row: u16, term: &HeadlessTerminal) -> String {
    let formatted = term
        .screen()
        .rows_formatted(0, term.cols)
        .nth(row as usize)
        .unwrap_or_default();
    format!(
        "\x1b[{};1H\x1b[0m\x1b[2K{}\x1b[0m",
        row + 1,
        String::from_utf8_lossy(&formatted)
    )
}
