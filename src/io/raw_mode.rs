use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

/// RAII guard for terminal raw mode. Disables raw mode when dropped.
pub struct RawModeGuard {
    active: bool,
}

impl RawModeGuard {
    pub fn enter() -> Result<Self> {
        enable_raw_mode()?;
        Ok(Self { active: true })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
        }
    }
}
