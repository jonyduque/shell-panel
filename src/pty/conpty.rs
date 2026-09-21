use crate::pty::shell::ShellType;
use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, ExitStatus, PtyPair, PtySize};

pub struct ConPtySession {
    pub pair: PtyPair,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
}

/// Options for [`ConPtySession::spawn`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SpawnOptions {
    /// Start PowerShell with `-NoProfile`.
    pub no_profile: bool,
}

impl ConPtySession {
    /// Spawns a ConPTY session running the specified shell with the initialization script.
    pub fn spawn(
        shell_type: ShellType,
        cols: u16,
        rows: u16,
        options: SpawnOptions,
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell_type.executable_name());
        // portable-pty starts the child in the home directory unless a cwd is given.
        cmd.cwd(std::env::current_dir()?);
        cmd.env("ISTERM", "1");
        cmd.env("TERM", "xterm-256color");
        cmd.arg("-NoLogo");
        if options.no_profile {
            cmd.arg("-NoProfile");
        }
        cmd.arg("-NoExit");
        cmd.arg("-EncodedCommand");
        cmd.arg(crate::shell::integration::encoded_command());

        let child = pair.slave.spawn_command(cmd).with_context(|| {
            format!(
                "Falha ao iniciar processo da shell {}",
                shell_type.executable_name()
            )
        })?;

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

/// Waits for the shell on a dedicated thread and reports its exit code.
///
/// ConPTY keeps its output pipe open after the child exits, so end-of-file on the
/// PTY reader cannot be used to detect that the shell is gone.
pub fn watch_exit(
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
) -> tokio::sync::oneshot::Receiver<u32> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let code = child.wait().map(|status| status.exit_code()).unwrap_or(1);
        let _ = tx.send(code);
    });
    rx
}
