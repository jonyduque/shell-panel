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

use std::sync::atomic::{AtomicU8, Ordering};

/// The VT input bit as it was when shell-panel took the console: 0 = nothing recorded,
/// 1 = clear, 2 = set, 3 = the mode could not be read.
static ORIGINAL_VT_INPUT: AtomicU8 = AtomicU8::new(0);

/// Encodes what `vt_input_enabled` found, for `ORIGINAL_VT_INPUT`.
pub fn encode_recorded(original: Option<bool>) -> u8 {
    match original {
        Some(false) => 1,
        Some(true) => 2,
        None => 3,
    }
}

/// The bit to put back for a recorded value, or `None` when there is nothing to put back.
pub fn decode_recorded(recorded: u8) -> Option<bool> {
    match recorded {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    }
}

fn vt_input_enabled() -> std::io::Result<bool> {
    let console = ConsoleMode::from(Handle::current_in_handle()?);
    Ok(console.mode()? & ENABLE_VIRTUAL_TERMINAL_INPUT != 0)
}

/// Remembers the console's VT input bit as found. Call it once, before changing the mode.
pub fn record_original_vt_input() {
    ORIGINAL_VT_INPUT.store(encode_recorded(vt_input_enabled().ok()), Ordering::SeqCst);
}

/// Puts the recorded VT input bit back; does nothing when none was recorded.
pub fn restore_original_vt_input() {
    if let Some(original) = decode_recorded(ORIGINAL_VT_INPUT.load(Ordering::SeqCst)) {
        let _ = set_vt_input(original);
    }
}
