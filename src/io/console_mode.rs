//! The console's input mode: whether the host's input arrives as raw terminal text.

use crossterm_winapi::{ConsoleMode, Handle};

/// `ENABLE_VIRTUAL_TERMINAL_INPUT`: the console delivers the host's input as the bytes it sent,
/// instead of decoding them into key, mouse and focus records.
pub const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;

/// `mode` with the virtual terminal input flag set or cleared and every other bit untouched.
pub fn with_vt_input(mode: u32, enabled: bool) -> u32 {
    if enabled {
        mode | ENABLE_VIRTUAL_TERMINAL_INPUT
    } else {
        mode & !ENABLE_VIRTUAL_TERMINAL_INPUT
    }
}

/// Sets or clears the flag on the console input. Fails outside a console, for example when
/// stdin is redirected; the caller decides whether that matters.
pub fn set_vt_input(enabled: bool) -> std::io::Result<()> {
    let console = ConsoleMode::from(Handle::current_in_handle()?);
    let mode = console.mode()?;
    let wanted = with_vt_input(mode, enabled);
    if wanted != mode {
        console.set_mode(wanted)?;
    }
    Ok(())
}
