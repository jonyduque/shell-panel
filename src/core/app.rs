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
use crate::engine::providers::json_spec::{FigOption, FigSpec, FigSubcommand, JsonSpecProvider};
use crate::io::key_event::{classify_key, encode_key_event, ActionKey};
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

/// How long Tab waits for the shell's report before it is handed to PowerShell unchanged.
/// PowerShell's own completion can take seconds when it has to load a module.
const REPORT_TIMEOUT: Duration = Duration::from_secs(3);

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

/// Builds the default `JsonSpecProvider` pre-populated with git and docker specs.
pub fn default_json_spec_provider() -> JsonSpecProvider {
    let mut provider = JsonSpecProvider::new();
    provider.add_spec(default_git_spec());
    provider.add_spec(default_docker_spec());
    provider
}

/// Returns the default Fig CLI specification for `git`.
pub fn default_git_spec() -> FigSpec {
    FigSpec {
        name: "git".to_string(),
        description: Some("Fast, scalable, distributed revision control system".to_string()),
        subcommands: vec![
            FigSubcommand {
                name: "status".to_string(),
                description: Some("Show the working tree status".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["-s".to_string(), "--short".to_string()],
                        description: Some("Give the output in the short-format".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-b".to_string(), "--branch".to_string()],
                        description: Some(
                            "Show the branch and tracking info even in short-format".to_string(),
                        ),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "commit".to_string(),
                description: Some("Record changes to the repository".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["-m".to_string(), "--message".to_string()],
                        description: Some("Use the given msg as the commit message".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-a".to_string(), "--all".to_string()],
                        description: Some(
                            "Automatically stage modified and deleted files".to_string(),
                        ),
                        args: None,
                    },
                    FigOption {
                        name: vec!["--amend".to_string()],
                        description: Some("Amend previous commit".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "push".to_string(),
                description: Some("Update remote refs along with associated objects".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["-u".to_string(), "--set-upstream".to_string()],
                        description: Some("Set upstream for git pull/status".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-f".to_string(), "--force".to_string()],
                        description: Some("Force updates".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "pull".to_string(),
                description: Some(
                    "Fetch from and integrate with another repository or branch".to_string(),
                ),
                options: vec![FigOption {
                    name: vec!["--rebase".to_string()],
                    description: Some("Rebase current branch on top of upstream".to_string()),
                    args: None,
                }],
                ..Default::default()
            },
            FigSubcommand {
                name: "add".to_string(),
                description: Some("Add file contents to the index".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["-A".to_string(), "--all".to_string()],
                        description: Some("Add all tracked and untracked files".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-p".to_string(), "--patch".to_string()],
                        description: Some("Interactively choose hunks of patch".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "checkout".to_string(),
                description: Some("Switch branches or restore working tree files".to_string()),
                options: vec![FigOption {
                    name: vec!["-b".to_string()],
                    description: Some("Create and checkout a new branch".to_string()),
                    args: None,
                }],
                ..Default::default()
            },
            FigSubcommand {
                name: "branch".to_string(),
                description: Some("List, create, or delete branches".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["-a".to_string(), "--all".to_string()],
                        description: Some(
                            "List both remote-tracking and local branches".to_string(),
                        ),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-d".to_string(), "--delete".to_string()],
                        description: Some("Delete a branch".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-D".to_string()],
                        description: Some("Shortcut for --delete --force".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "diff".to_string(),
                description: Some(
                    "Show changes between commits, commit and working tree, etc".to_string(),
                ),
                options: vec![FigOption {
                    name: vec!["--staged".to_string(), "--cached".to_string()],
                    description: Some("View changes staged in the index".to_string()),
                    args: None,
                }],
                ..Default::default()
            },
            FigSubcommand {
                name: "log".to_string(),
                description: Some("Show commit logs".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["--oneline".to_string()],
                        description: Some(
                            "Shorthand for --pretty=oneline --abbrev-commit".to_string(),
                        ),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-n".to_string()],
                        description: Some("Limit number of commits to output".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "clone".to_string(),
                description: Some("Clone a repository into a new directory".to_string()),
                ..Default::default()
            },
            FigSubcommand {
                name: "merge".to_string(),
                description: Some("Join two or more development histories together".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["--no-ff".to_string()],
                        description: Some(
                            "Create a merge commit even if fast-forward is possible".to_string(),
                        ),
                        args: None,
                    },
                    FigOption {
                        name: vec!["--abort".to_string()],
                        description: Some("Abort current conflict resolution process".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "rebase".to_string(),
                description: Some("Reapply commits on top of another base tip".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["-i".to_string(), "--interactive".to_string()],
                        description: Some("Interactive rebase".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["--continue".to_string()],
                        description: Some("Continue rebase after resolving conflict".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["--abort".to_string()],
                        description: Some("Abort rebase and reset HEAD".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "reset".to_string(),
                description: Some("Reset current HEAD to specified state".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["--hard".to_string()],
                        description: Some("Reset index and working tree".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["--soft".to_string()],
                        description: Some("Keep changes in index and working tree".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
        ],
        options: vec![
            FigOption {
                name: vec!["-v".to_string(), "--version".to_string()],
                description: Some("Print git version".to_string()),
                args: None,
            },
            FigOption {
                name: vec!["--help".to_string()],
                description: Some("Display help information".to_string()),
                args: None,
            },
        ],
    }
}

/// Returns the default Fig CLI specification for `docker`.
pub fn default_docker_spec() -> FigSpec {
    FigSpec {
        name: "docker".to_string(),
        description: Some("A self-sufficient runtime for containers".to_string()),
        subcommands: vec![
            FigSubcommand {
                name: "run".to_string(),
                description: Some("Run a command in a new container".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["-d".to_string(), "--detach".to_string()],
                        description: Some(
                            "Run container in background and print container ID".to_string(),
                        ),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-p".to_string(), "--publish".to_string()],
                        description: Some("Publish a container's port(s) to the host".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-v".to_string(), "--volume".to_string()],
                        description: Some("Bind mount a volume".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-it".to_string()],
                        description: Some("Allocate a pseudo-TTY and keep stdin open".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["--name".to_string()],
                        description: Some("Assign a name to the container".to_string()),
                        args: None,
                    },
                    FigOption {
                        name: vec!["--rm".to_string()],
                        description: Some(
                            "Automatically remove container when it exits".to_string(),
                        ),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "ps".to_string(),
                description: Some("List containers".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["-a".to_string(), "--all".to_string()],
                        description: Some(
                            "Show all containers (default shows just running)".to_string(),
                        ),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-q".to_string(), "--quiet".to_string()],
                        description: Some("Only display container IDs".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "build".to_string(),
                description: Some("Build an image from a Dockerfile".to_string()),
                options: vec![
                    FigOption {
                        name: vec!["-t".to_string(), "--tag".to_string()],
                        description: Some(
                            "Name and optionally tag in 'name:tag' format".to_string(),
                        ),
                        args: None,
                    },
                    FigOption {
                        name: vec!["-f".to_string(), "--file".to_string()],
                        description: Some("Path to the Dockerfile".to_string()),
                        args: None,
                    },
                ],
                ..Default::default()
            },
            FigSubcommand {
                name: "images".to_string(),
                description: Some("List images".to_string()),
                options: vec![FigOption {
                    name: vec!["-a".to_string(), "--all".to_string()],
                    description: Some("Show all images".to_string()),
                    args: None,
                }],
                ..Default::default()
            },
            FigSubcommand {
                name: "pull".to_string(),
                description: Some("Download an image from a registry".to_string()),
                ..Default::default()
            },
            FigSubcommand {
                name: "push".to_string(),
                description: Some("Upload an image to a registry".to_string()),
                ..Default::default()
            },
            FigSubcommand {
                name: "stop".to_string(),
                description: Some("Stop one or more running containers".to_string()),
                ..Default::default()
            },
            FigSubcommand {
                name: "start".to_string(),
                description: Some("Start one or more stopped containers".to_string()),
                ..Default::default()
            },
            FigSubcommand {
                name: "restart".to_string(),
                description: Some("Restart one or more containers".to_string()),
                ..Default::default()
            },
            FigSubcommand {
                name: "rm".to_string(),
                description: Some("Remove one or more containers".to_string()),
                options: vec![FigOption {
                    name: vec!["-f".to_string(), "--force".to_string()],
                    description: Some("Force the removal of a running container".to_string()),
                    args: None,
                }],
                ..Default::default()
            },
            FigSubcommand {
                name: "rmi".to_string(),
                description: Some("Remove one or more images".to_string()),
                options: vec![FigOption {
                    name: vec!["-f".to_string(), "--force".to_string()],
                    description: Some("Force removal of the image".to_string()),
                    args: None,
                }],
                ..Default::default()
            },
            FigSubcommand {
                name: "logs".to_string(),
                description: Some("Fetch the logs of a container".to_string()),
                options: vec![FigOption {
                    name: vec!["-f".to_string(), "--follow".to_string()],
                    description: Some("Follow log output".to_string()),
                    args: None,
                }],
                ..Default::default()
            },
            FigSubcommand {
                name: "exec".to_string(),
                description: Some("Run a command in a running container".to_string()),
                options: vec![FigOption {
                    name: vec!["-it".to_string()],
                    description: Some("Allocate a pseudo-TTY and keep stdin open".to_string()),
                    args: None,
                }],
                ..Default::default()
            },
            FigSubcommand {
                name: "compose".to_string(),
                description: Some("Docker Compose subcommands".to_string()),
                subcommands: vec![
                    FigSubcommand {
                        name: "up".to_string(),
                        description: Some("Create and start containers".to_string()),
                        ..Default::default()
                    },
                    FigSubcommand {
                        name: "down".to_string(),
                        description: Some("Stop and remove containers, networks".to_string()),
                        ..Default::default()
                    },
                    FigSubcommand {
                        name: "build".to_string(),
                        description: Some("Build or rebuild services".to_string()),
                        ..Default::default()
                    },
                    FigSubcommand {
                        name: "logs".to_string(),
                        description: Some("View output from containers".to_string()),
                        ..Default::default()
                    },
                    FigSubcommand {
                        name: "ps".to_string(),
                        description: Some("List containers".to_string()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        ],
        options: vec![
            FigOption {
                name: vec!["-v".to_string(), "--version".to_string()],
                description: Some("Print version information and quit".to_string()),
                args: None,
            },
            FigOption {
                name: vec!["--help".to_string()],
                description: Some("Print usage".to_string()),
                args: None,
            },
        ],
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

        let ConPtySession { pair, child } = ConPtySession::spawn(
            shell_type,
            cols,
            rows,
            SpawnOptions {
                no_profile: self.no_profile,
            },
        )?;
        let mut exit_rx = watch_exit(child);
        let _raw_guard = RawModeGuard::enter()?;

        let mut pty_reader = pair.master.try_clone_reader()?;
        let mut pty_writer = pair.master.take_writer()?;

        let (pty_tx, mut pty_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1024);
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = pty_reader.read(&mut buf) {
                if n == 0 || pty_tx.blocking_send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });

        let mut term = HeadlessTerminal::new(cols, rows);
        let mut command_state = CommandState::default();
        let mut dropdown = Dropdown {
            state: SuggestionState::new(self.config.max_suggestions),
            layout: None,
            report: None,
        };
        let engine = CompletionEngine::new(default_json_spec_provider());
        let (completion_tx, mut completion_rx) = tokio::sync::mpsc::channel::<CompletionOutcome>(4);

        let mut event_stream = crossterm::event::EventStream::new();
        let mut stdout = std::io::stdout();
        let mut osc_residual: Vec<u8> = Vec::new();
        // Bumped by every key press: reports and results of an older generation are stale.
        let mut generation: u64 = 0;
        // Deadline of the report requested by the last Tab, while it is outstanding.
        let mut report_deadline: Option<Instant> = None;
        let mut exit_code: Option<u32> = None;

        loop {
            let deadline = report_deadline.unwrap_or_else(Instant::now);

            tokio::select! {
                code = &mut exit_rx => {
                    // The shell's last output may still be in flight: drain until the PTY is quiet.
                    let drain_until = Instant::now() + Duration::from_secs(1);
                    while let Ok(Some(chunk)) = tokio::time::timeout_at(
                        drain_until.min(Instant::now() + Duration::from_millis(100)),
                        pty_rx.recv(),
                    ).await {
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

                    if let Some(report) = command_state.report.take() {
                        // Only the answer to the latest Tab is wanted.
                        if report_deadline.take().is_some() {
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
                    match outcome.results.as_slice() {
                        [] => write_to_pty(&mut pty_writer, b"\t"),
                        [only] => write_to_pty(&mut pty_writer, &plan_replacement(&outcome.report, only).to_bytes()),
                        _ => dropdown.open(outcome.report, outcome.results, &term, &self.theme, &mut stdout),
                    }
                }

                _ = tokio::time::sleep_until(deadline), if report_deadline.is_some() => {
                    // No report: the chord was not bound or PowerShell is busy. Plain Tab.
                    report_deadline = None;
                    write_to_pty(&mut pty_writer, b"\t");
                }

                maybe_event = event_stream.next() => {
                    let event = match maybe_event {
                        Some(Ok(ev)) => ev,
                        _ => continue,
                    };

                    match event {
                        Event::Resize(new_cols, new_rows) => {
                            dropdown.close(&term, &mut stdout);
                            let _ = pair.master.resize(PtySize { rows: new_rows, cols: new_cols, pixel_width: 0, pixel_height: 0 });
                            term.resize(new_cols, new_rows);
                        }
                        Event::Key(key_event) if key_event.kind != KeyEventKind::Release => {
                            generation += 1;
                            report_deadline = None;
                            self.handle_key(
                                &key_event,
                                &mut dropdown,
                                &term,
                                &command_state,
                                &mut pty_writer,
                                &mut stdout,
                                &mut report_deadline,
                            );
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
        command_state: &CommandState,
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
            write_to_pty(pty_writer, REPORT_REQUEST_KEY);
            *report_deadline = Some(Instant::now() + REPORT_TIMEOUT);
        } else {
            write_to_pty(pty_writer, &encode_key_event(key_event));
        }
    }
}
