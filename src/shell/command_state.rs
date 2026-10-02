use crate::shell::osc::OscEvent;
use crate::shell::report::ShellReport;

/// What shell-panel knows about the shell from its integration messages.
#[derive(Default, Clone, PartialEq, Eq)]
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

/// Written by hand so the session secret never reaches a log or a panic message.
impl std::fmt::Debug for CommandState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandState")
            .field("token", &"<redacted>")
            .field("cwd", &self.cwd)
            .field("reading_line", &self.reading_line)
            .field("report", &self.report)
            .field("outstanding_reports", &self.outstanding_reports)
            .field("awaiting_report", &self.awaiting_report)
            .finish()
    }
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
        self.outstanding_reports = self.outstanding_reports.saturating_add(1);
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
                // Trade-off: forget requests that were never answered (swallowed chords), so
                // they cannot make a later report look stale. The cost is a residual window of
                // a few milliseconds: a chord written between Enter and the arrival of RE
                // (reading_line still true) is answered by the next ReadLine, after this
                // marker, so its report is counted against the next line.
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
