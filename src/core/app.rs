use std::io::{Read, Write};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use futures_util::StreamExt;
use portable_pty::PtySize;
use tokio::time::Instant;

use crate::core::config::Config;
use crate::engine::aggregate::{plan_replacement, CompletionEngine};
use crate::engine::provider::Suggestion;
use crate::engine::providers::json_spec::{default_specs_dir, JsonSpecProvider};
use crate::io::console_mode::set_vt_input;
use crate::io::key_event::{
    classify_key, encode_key_event, ends_in_unfinished_escape, is_stray_escape, program_mode_bytes,
    withheld_tab_bytes, ActionKey,
};
use crate::io::raw_mode::RawModeGuard;
use crate::pty::conpty::{watch_exit, ConPtySession, SpawnOptions};
use crate::pty::shell::detect_shell;
use crate::shell::command_state::CommandState;
use crate::shell::osc::REPORT_REQUEST_KEY;
use crate::shell::report::ShellReport;
use crate::shell::stream::ingest_pty_chunk;
use crate::ui::renderer::{DropdownLayout, Renderer};
use crate::ui::suggestion_state::SuggestionState;
use crate::ui::theme::Theme;
use crate::vt::emulator::HeadlessTerminal;
use tracing::debug;

/// How long Tab waits for the shell's report before it is handed to PowerShell unchanged.
/// PowerShell's own completion can take seconds when it has to load a module.
const REPORT_TIMEOUT: Duration = Duration::from_secs(3);

/// The longest program mode holds keys back to complete an escape sequence (and the longest it
/// keeps draining one burst), so PTY output, the exit and resizes are never starved.
const BURST_DEADLINE: Duration = Duration::from_millis(20);

/// The stream's next item if one is queued right now, `Err` otherwise; never waits. Polled
/// with the task's own context, so the stream's waker stays the reactor's (`now_or_never` would
/// replace it with a no-op one and input would stop arriving).
async fn next_if_ready<S: futures_util::Stream + Unpin>(
    stream: &mut S,
) -> Result<Option<S::Item>, ()> {
    std::future::poll_fn(|cx| {
        std::task::Poll::Ready(match stream.poll_next_unpin(cx) {
            std::task::Poll::Ready(item) => Ok(item),
            std::task::Poll::Pending => Err(()),
        })
    })
    .await
}

/// The most bytes one program-mode write carries.
const BURST_MAX_BYTES: usize = 64 * 1024;

/// Result of a background completion, tagged with the key generation that requested it.
struct CompletionOutcome {
    generation: u64,
    report: ShellReport,
    results: Vec<Suggestion>,
}

/// The dropdown on screen together with the report its suggestions were computed for.
struct Dropdown {
    state: SuggestionState,
    layout: Option<DropdownLayout>,
    report: Option<ShellReport>,
}

impl Dropdown {
    fn is_open(&self) -> bool {
        self.state.visible
    }

    fn draw<W: Write>(&mut self, term: &HeadlessTerminal, theme: &Theme, out: &mut W) {
        // Restore the rows the previous layout covered first: paging can land on a shorter page
        // or move the panel above the cursor, and those rows would keep stale suggestions.
        if let Some(layout) = self.layout.take() {
            let _ = Renderer::clear_dropdown(&layout, term, out);
        }
        let (cx, cy) = term.cursor_position();
        self.layout = Renderer::render_dropdown(&self.state, term, theme, cx, cy, out)
            .ok()
            .flatten();
    }

    fn open<W: Write>(
        &mut self,
        report: ShellReport,
        results: Vec<Suggestion>,
        term: &HeadlessTerminal,
        theme: &Theme,
        out: &mut W,
    ) {
        self.report = Some(report);
        self.state.set_suggestions(results);
        self.draw(term, theme, out);
    }

    /// Restores the covered rows. Must run before new output reaches `term`.
    fn close<W: Write>(&mut self, term: &HeadlessTerminal, out: &mut W) {
        if let Some(layout) = self.layout.take() {
            let _ = Renderer::clear_dropdown(&layout, term, out);
        }
        self.state.dismiss();
    }
}

fn write_to_pty<W: Write>(writer: &mut W, bytes: &[u8]) {
    if !bytes.is_empty() {
        let _ = writer.write_all(bytes);
        let _ = writer.flush();
    }
}

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub override_shell: Option<String>,
    pub no_profile: bool,
}

impl App {
    pub fn new(config: Config, override_shell: Option<String>) -> Self {
        let theme = Theme::from_config(&config);
        Self {
            config,
            theme,
            override_shell,
            no_profile: false,
        }
    }

    pub async fn run(&mut self) -> Result<u32> {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let shell_type = detect_shell(self.override_shell.as_deref());

        let ConPtySession { pair, child, token } = ConPtySession::spawn(
            shell_type,
            cols,
            rows,
            SpawnOptions {
                no_profile: self.no_profile,
            },
        )?;
        let mut exit_rx = watch_exit(child);

        let mut json_specs = JsonSpecProvider::with_embedded_specs();
        if let Some(dir) = default_specs_dir().filter(|d| d.is_dir()) {
            for warning in json_specs.load_dir(&dir) {
                eprintln!("shell-panel: {warning}");
            }
        }

        let _raw_guard = RawModeGuard::enter()?;
        // Program mode until the shell says PSReadLine is reading a line: the host's input then
        // reaches the PTY unchanged, which is also how the answer to ConPTY's start-up DA1 query
        // gets to ConPTY.
        let mut program_mode = true;
        if let Err(err) = set_vt_input(true) {
            debug!(%err, "could not enable virtual terminal input");
        }

        let mut pty_reader = pair.master.try_clone_reader()?;
        let mut pty_writer = pair.master.take_writer()?;

        let (pty_tx, mut pty_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1024);
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match pty_reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if pty_tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(ref err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        });

        let mut term = HeadlessTerminal::new(cols, rows);
        let mut command_state = CommandState::new(token);
        let mut dropdown = Dropdown {
            state: SuggestionState::new(self.config.max_suggestions),
            layout: None,
            report: None,
        };
        let engine = CompletionEngine::new(json_specs);
        let (completion_tx, mut completion_rx) = tokio::sync::mpsc::channel::<CompletionOutcome>(4);

        let mut event_stream = crossterm::event::EventStream::new();
        let mut stdout = std::io::stdout();
        let mut osc_residual: Vec<u8> = Vec::new();
        // Bumped by every key press: reports and results of an older generation are stale.
        let mut generation: u64 = 0;
        // Deadline of the report requested by the last Tab, while it is outstanding.
        let mut report_deadline: Option<Instant> = None;
        // Set while a Tab is waiting for its completion: the `\t` was withheld in favour of the
        // report request and still owes the shell a Tab if the completion never lands.
        let mut tab_pending = false;
        let mut exit_code: Option<u32> = None;
        let mut input_open = true;

        loop {
            let deadline = report_deadline.unwrap_or_else(Instant::now);

            tokio::select! {
                code = &mut exit_rx => {
                    // Restore the covered rows before the drain loop writes more PTY output.
                    dropdown.close(&term, &mut stdout);
                    // The shell's last output may still be in flight: drain until the PTY is quiet.
                    let drain_until = Instant::now() + Duration::from_secs(1);
                    // An orphaned child can keep the PTY busy: the deadline ends the loop even
                    // when every poll of `recv` returns a chunk before it expires.
                    while Instant::now() < drain_until {
                        let Ok(Some(chunk)) = tokio::time::timeout_at(
                            drain_until.min(Instant::now() + Duration::from_millis(100)),
                            pty_rx.recv(),
                        ).await else { break };
                        let clean = ingest_pty_chunk(&chunk, &mut term, &mut command_state, &mut osc_residual);
                        let _ = stdout.write_all(&clean);
                    }
                    let _ = stdout.flush();
                    exit_code = Some(code.unwrap_or(1));
                    break;
                }

                chunk = pty_rx.recv() => {
                    let Some(chunk) = chunk else { break };
                    // Restore the covered rows from the mirror as it was when they were covered;
                    // output that scrolls would otherwise be applied twice.
                    dropdown.close(&term, &mut stdout);
                    let clean = ingest_pty_chunk(&chunk, &mut term, &mut command_state, &mut osc_residual);
                    if !clean.is_empty() {
                        let _ = stdout.write_all(&clean);
                        let _ = stdout.flush();
                    }

                    // Prompt mode while PSReadLine reads a line, program mode otherwise.
                    let want = !command_state.reading_line;
                    if want != program_mode {
                        program_mode = want;
                        if let Err(err) = set_vt_input(program_mode) {
                            debug!(%err, program_mode, "could not switch the console input mode");
                        }
                    }

                    if let Some(report) = command_state.report.take() {
                        // `CommandState` keeps a report only when it answers the Tab that waits now.
                        // Two sources say "a Tab waits": `CommandState::awaiting_report` and
                        // `report_deadline`. They must change together; this guard makes
                        // `abandon_report` merely defensive.
                        if report_deadline.take().is_some() {
                            debug!(matches = report.matches.len(), "shell report arrived");
                            let engine = engine.clone();
                            let cwd = command_state.cwd.clone();
                            let tx = completion_tx.clone();
                            tokio::spawn(async move {
                                let results = engine.complete(&report, &cwd).await;
                                let _ = tx.send(CompletionOutcome { generation, report, results }).await;
                            });
                        }
                    }
                }

                Some(outcome) = completion_rx.recv() => {
                    if outcome.generation != generation {
                        continue;
                    }
                    debug!(merged = outcome.results.len(), "completion results merged");
                    // The Tab is answered here, whichever branch applies.
                    tab_pending = false;
                    match outcome.results.as_slice() {
                        [] => write_to_pty(&mut pty_writer, b"\t"),
                        [only] => write_to_pty(&mut pty_writer, &plan_replacement(&outcome.report, only).to_bytes()),
                        _ => dropdown.open(outcome.report, outcome.results, &term, &self.theme, &mut stdout),
                    }
                }

                _ = tokio::time::sleep_until(deadline), if report_deadline.is_some() => {
                    // No report: the chord was not bound or PowerShell is busy. Plain Tab.
                    debug!("shell report timed out");
                    report_deadline = None;
                    tab_pending = false;
                    command_state.abandon_report();
                    write_to_pty(&mut pty_writer, b"\t");
                }

                maybe_event = event_stream.next(), if input_open => {
                    let event = match maybe_event {
                        Some(Ok(ev)) => ev,
                        Some(Err(_)) => continue,
                        // The console input is gone; polling again would return None at once, forever.
                        None => {
                            input_open = false;
                            continue;
                        }
                    };

                    match event {
                        Event::Resize(new_cols, new_rows) => {
                            dropdown.close(&term, &mut stdout);
                            let _ = pair.master.resize(PtySize { rows: new_rows, cols: new_cols, pixel_width: 0, pixel_height: 0 });
                            term.resize(new_cols, new_rows);
                        }
                        // A bare ESC cannot be a key press in prompt mode (see
                        // `is_stray_escape`): drop it, PSReadLine would act on an ESC.
                        Event::Key(key_event) if !program_mode && is_stray_escape(&key_event) => {
                            debug!(?key_event, "dropped a stray ESC in prompt mode");
                        }
                        Event::Key(key_event) if key_event.kind != KeyEventKind::Release => {
                            generation += 1;
                            report_deadline = None;
                            command_state.abandon_report();
                            // This key invalidates the outstanding report, so hand the shell the
                            // Tab it never got before the key that follows it.
                            write_to_pty(&mut pty_writer, withheld_tab_bytes(tab_pending, key_event.code));
                            if program_mode {
                                // The host's own input, byte for byte: no classification, no
                                // report request, no dropdown.
                                //
                                // The keys the console has already queued go out in one write:
                                // ConPTY reads an ESC that ends a write as the Escape key, so
                                // `ESC [ A` written one byte at a time arrives as Escape, `[`, `A`.
                                // Queued keys are drained without waiting (`next_if_ready` polls
                                // the stream once; a zero timeout would cost a timer tick, about
                                // 16 ms on Windows); only bytes that end inside an escape
                                // sequence wait for the rest, and never past `until`.
                                let until = Instant::now() + BURST_DEADLINE;
                                let mut bytes = program_mode_bytes(&key_event);
                                let mut resize = None;
                                while bytes.len() < BURST_MAX_BYTES && Instant::now() < until {
                                    let next = if ends_in_unfinished_escape(&bytes) {
                                        tokio::time::timeout_at(until, event_stream.next()).await.map_err(|_| ())
                                    } else {
                                        next_if_ready(&mut event_stream).await
                                    };
                                    let Ok(Some(Ok(next))) = next else { break };
                                    match next {
                                        Event::Key(k) if k.kind != KeyEventKind::Release => {
                                            bytes.extend_from_slice(&program_mode_bytes(&k));
                                        }
                                        Event::Resize(c, r) => {
                                            resize = Some((c, r));
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                write_to_pty(&mut pty_writer, &bytes);
                                tab_pending = false;
                                if let Some((new_cols, new_rows)) = resize {
                                    dropdown.close(&term, &mut stdout);
                                    let _ = pair.master.resize(PtySize { rows: new_rows, cols: new_cols, pixel_width: 0, pixel_height: 0 });
                                    term.resize(new_cols, new_rows);
                                }
                            } else {
                                self.handle_key(
                                    &key_event,
                                    &mut dropdown,
                                    &term,
                                    &mut command_state,
                                    &mut pty_writer,
                                    &mut stdout,
                                    &mut report_deadline,
                                );
                                // `handle_key` sets the deadline exactly when it requested a report.
                                tab_pending = report_deadline.is_some();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        dropdown.close(&term, &mut stdout);
        drop(_raw_guard);
        let exit_code = match exit_code {
            Some(code) => code,
            // The reader thread ended first; give the exit watcher a moment before giving up.
            None => tokio::time::timeout(Duration::from_secs(2), exit_rx)
                .await
                .ok()
                .and_then(|result| result.ok())
                .unwrap_or(1),
        };
        Ok(exit_code)
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_key<W: Write, O: Write>(
        &self,
        key_event: &KeyEvent,
        dropdown: &mut Dropdown,
        term: &HeadlessTerminal,
        command_state: &mut CommandState,
        pty_writer: &mut W,
        stdout: &mut O,
        report_deadline: &mut Option<Instant>,
    ) {
        let action = classify_key(key_event);

        if dropdown.is_open() {
            match action {
                ActionKey::MenuDown => {
                    dropdown.state.move_down();
                    dropdown.draw(term, &self.theme, stdout);
                }
                ActionKey::MenuUp => {
                    dropdown.state.move_up();
                    dropdown.draw(term, &self.theme, stdout);
                }
                ActionKey::DismissMenu => dropdown.close(term, stdout),
                ActionKey::AcceptSuggestion => {
                    let selected = dropdown.state.active_item().cloned();
                    dropdown.close(term, stdout);
                    if let (Some(report), Some(selected)) = (dropdown.report.take(), selected) {
                        write_to_pty(pty_writer, &plan_replacement(&report, &selected).to_bytes());
                    }
                }
                ActionKey::Passthrough => {
                    dropdown.close(term, stdout);
                    write_to_pty(pty_writer, &encode_key_event(key_event));
                }
            }
            return;
        }

        let completes = action == ActionKey::AcceptSuggestion
            && key_event.code == KeyCode::Tab
            && command_state.reading_line
            && !term.is_alternate_buffer();
        if completes {
            // Only PSReadLine answers the request, and it is reading right now.
            debug!("requesting shell report");
            write_to_pty(pty_writer, REPORT_REQUEST_KEY);
            command_state.request_report();
            *report_deadline = Some(Instant::now() + REPORT_TIMEOUT);
        } else {
            write_to_pty(pty_writer, &encode_key_event(key_event));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A screen with `base<row>` on every row and the cursor back on row 0.
    fn base_screen() -> (HeadlessTerminal, Vec<u8>) {
        let mut bytes = Vec::new();
        for row in 0..24u16 {
            bytes.extend_from_slice(format!("\x1b[{};1Hbase{row}", row + 1).as_bytes());
        }
        bytes.extend_from_slice(b"\x1b[1;1H");
        let mut term = HeadlessTerminal::new(80, 24);
        term.process(&bytes);
        (term, bytes)
    }

    fn row_text(term: &HeadlessTerminal, row: u16) -> String {
        (0..term.cols)
            .filter_map(|col| term.screen().cell(row, col))
            .map(|cell| cell.contents())
            .collect()
    }

    #[test]
    fn test_draw_restores_the_previous_layout_before_paging() {
        let (term, base) = base_screen();
        let theme = Theme::default();
        let mut dropdown = Dropdown {
            state: SuggestionState::new(5),
            layout: None,
            report: None,
        };
        // 13 items over pages of 5: the last page has only 3.
        dropdown.state.set_suggestions(
            (0..13)
                .map(|i| Suggestion::new(format!("item{i}"), format!("item{i}"), None, 50))
                .collect(),
        );

        let mut first = Vec::new();
        dropdown.draw(&term, &theme, &mut first);
        assert_eq!(
            dropdown.layout,
            Some(DropdownLayout {
                start_row: 1,
                row_count: 5
            })
        );

        for _ in 0..10 {
            dropdown.state.move_down();
        }
        let mut second = Vec::new();
        dropdown.draw(&term, &theme, &mut second);
        assert_eq!(
            dropdown.layout,
            Some(DropdownLayout {
                start_row: 1,
                row_count: 3
            })
        );

        // Replay both frames on the screen they were written to.
        let mut screen = HeadlessTerminal::new(80, 24);
        screen.process(&base);
        screen.process(&first);
        screen.process(&second);

        assert!(row_text(&screen, 1).contains("item10"));
        for row in [4u16, 5] {
            let text = row_text(&screen, row);
            assert!(
                text.starts_with(&format!("base{row}")),
                "row {row} kept a stale suggestion: {text:?}"
            );
        }
    }
}
