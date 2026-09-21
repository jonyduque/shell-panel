use crate::pty::shell::ShellType;
use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, ExitStatus, PtyPair, PtySize};
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
        cmd.arg("-NoLogo");
        cmd.arg("-ExecutionPolicy");
        cmd.arg("Bypass");
        cmd.arg("-NoExit");
        cmd.arg("-Command");

        // Literal PowerShell single quotes escape ($ and ` are preserved without evaluation)
        let escaped_path = script_path.to_string_lossy().replace('\'', "''");
        cmd.arg(format!("try {{ . '{}' }} catch {{}}", escaped_path));

        let child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("Falha ao iniciar processo da shell {}", shell_type.executable_name()))?;

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

    /// Terminates the child process.
    pub fn kill(&mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
    }

    /// Checks the child process exit status without blocking.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        Ok(status)
    }
}
