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
    /// The report answering the Tab that waits now, taken by the reactor loop.
    pub report: Option<ShellReport>,
    /// Report requests written to the shell whose report has not arrived yet.
    outstanding_reports: u32,
    /// True while a Tab waits for the answer to the latest request.
    awaiting_report: bool,
}

impl CommandState {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            ..Self::default()
        }
    }

    /// A Tab wrote a report request to the shell and now waits for its answer.
    pub fn request_report(&mut self) {
        self.outstanding_reports += 1;
        self.awaiting_report = true;
    }

    /// The waiting Tab gave up (timeout, or another key): its answer, when it lands, is stale.
    pub fn abandon_report(&mut self) {
        self.awaiting_report = false;
    }

    pub fn handle_osc(&mut self, event: OscEvent) {
        match event {
            OscEvent::ReadLineStarted { cwd } => {
                self.reading_line = true;
                self.report = None;
                // Every report of the previous line came before this marker: a request still
                // counted was never answered and never will be.
                self.outstanding_reports = 0;
                if let Some(cwd) = cwd {
                    self.cwd = cwd;
                }
            }
            OscEvent::ReadLineEnded => self.reading_line = false,
            OscEvent::Report(report) => {
                // PSReadLine answers requests in order, so only the report that settles the last
                // one describes the line as it is now.
                self.outstanding_reports = self.outstanding_reports.saturating_sub(1);
                if self.outstanding_reports == 0 && self.awaiting_report {
                    self.awaiting_report = false;
                    self.report = Some(report);
                }
            }
        }
    }
}
