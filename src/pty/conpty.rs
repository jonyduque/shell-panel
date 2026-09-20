use crate::pty::shell::ShellType;
use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::path::Path;

pub struct ConPtySession {
    pub pair: PtyPair,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl ConPtySession {
    /// Spawns a ConPTY session running the specified shell with the initialization script.
    pub fn spawn(
        shell_type: ShellType,
        cols: u16,
        rows: u16,
        script_path: &Path,
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell_type.executable_name());
        cmd.env("ISTERM", "1");
        cmd.env("TERM", "xterm-256color");
        cmd.arg("-noexit");
        cmd.arg("-command");
        cmd.arg(format!("try {{ . \"{}\" }} catch {{}}", script_path.display()));

        let child = pair.slave.spawn_command(cmd)?;

        Ok(Self { pair, child })
    }

    /// Resizes the PTY terminal window dimensions.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.pair.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }
}
