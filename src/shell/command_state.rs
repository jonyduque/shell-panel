use crate::shell::osc::OscEvent;
use crate::shell::report::ShellReport;

/// What shell-panel knows about the shell from its integration messages.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandState {
    /// Secret of this session; messages without it are ignored (see `new_session_token`).
    pub token: String,
    /// Current filesystem location of the shell.
    pub cwd: String,
    /// True while PSReadLine is reading a line: the only time the report request is answered.
    pub reading_line: bool,
    /// Latest completion report, taken by the reactor loop.
    pub report: Option<ShellReport>,
}

impl CommandState {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            ..Self::default()
        }
    }

    pub fn handle_osc(&mut self, event: OscEvent) {
        match event {
            OscEvent::ReadLineStarted { cwd } => {
                self.reading_line = true;
                self.report = None;
                if let Some(cwd) = cwd {
                    self.cwd = cwd;
                }
            }
            OscEvent::ReadLineEnded => self.reading_line = false,
            OscEvent::Report(report) => self.report = Some(report),
        }
    }
}
