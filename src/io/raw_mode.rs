use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use super::console_mode::{record_original_vt_input, restore_original_vt_input};

/// RAII guard for terminal raw mode. Disables raw mode when dropped.
pub struct RawModeGuard {
    active: bool,
}

impl RawModeGuard {
    pub fn enter() -> Result<Self> {
        // Before anything changes the mode: this is what the drop and the panic hook restore.
        record_original_vt_input();
        enable_raw_mode()?;
        Ok(Self { active: true })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.active {
            let mut out = std::io::stdout().lock();
            let _ = std::io::Write::write_all(&mut out, b"\x1b[?25h");
            let _ = std::io::Write::flush(&mut out);
            // Leave the console input mode as it was found.
            restore_original_vt_input();
            let _ = disable_raw_mode();
        }
    }
}
