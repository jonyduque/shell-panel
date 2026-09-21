use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use futures_util::StreamExt;

use crate::core::config::Config;
use crate::engine::lexer::lex_command_line;
use crate::engine::provider::{CompletionProvider, Suggestion, SuggestionKind};
use crate::engine::providers::carapace::CarapaceProvider;
use crate::engine::providers::files::FileProvider;
use crate::engine::providers::json_spec::{FigOption, FigSpec, FigSubcommand, JsonSpecProvider};
use crate::engine::providers::powershell::PowerShellProvider;
use crate::engine::providers::zoxide::ZoxideProvider;
use crate::engine::replacement::calculate_replacement;
use crate::io::filter::sanitize_output_stream;
use crate::io::key_event::{classify_key, encode_key_event, ActionKey};
use crate::io::raw_mode::RawModeGuard;
use crate::pty::conpty::ConPtySession;
use crate::pty::shell::detect_shell;
use crate::shell::command_state::CommandState;
use crate::shell::osc::parse_osc_sequence;
use crate::ui::renderer::{DropdownLayout, Renderer};
use crate::ui::suggestion_state::SuggestionState;
use crate::vt::emulator::HeadlessTerminal;

/// Embedded PowerShell shell integration script ensuring single-binary portability.
pub const SHELL_INTEGRATION_SCRIPT: &str = include_str!("../../assets/shellIntegration.ps1");

/// Resolves or extracts the shell integration script path.
///
/// If `assets/shellIntegration.ps1` exists on disk, returns its canonical path.
/// Otherwise, extracts the embedded script to `%TEMP%\shell-panel\shellIntegration.ps1`.
pub fn get_shell_integration_path() -> Result<PathBuf> {
    let local = Path::new("assets/shellIntegration.ps1");
    if local.is_file() {
        if let Ok(abs) = local.canonicalize() {
            let s = abs.to_string_lossy();
            let clean = s.strip_prefix(r"\\?\").unwrap_or(&s);
            return Ok(PathBuf::from(clean));
        }
    }

    let temp_dir = std::env::temp_dir().join("shell-panel");
    std::fs::create_dir_all(&temp_dir)?;
    let temp_path = temp_dir.join("shellIntegration.ps1");
    let clean_str = temp_path.to_string_lossy();
    let clean = clean_str.strip_prefix(r"\\?\").unwrap_or(&clean_str);
    let target = PathBuf::from(clean);
    std::fs::write(&target, SHELL_INTEGRATION_SCRIPT)?;
    Ok(target)
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

/// Helper function to scan OSC sequences from PTY output chunks,
/// process VT bytes incrementally up to OSC boundaries to track exact cursor position,
/// handle command state transitions, and return clean terminal bytes with OSC 6973 stripped.
fn scan_and_handle_osc(
    data: &[u8],
    term: &mut HeadlessTerminal,
    command_state: &mut CommandState,
    residual: &mut Vec<u8>,
) -> Vec<u8> {
    let mut clean_output = Vec::with_capacity(data.len());
    let mut i = 0;
    let mut last = 0;

    while i < data.len() {
        if data[i..].starts_with(b"\x1b]6973;") {
            if i > last {
                let slice = &data[last..i];
                term.process(slice);
                clean_output.extend_from_slice(slice);
            }
            let payload_start = i + 2; // skip '\x1b]'
            let mut j = i + 7; // after '\x1b]6973;'
            let mut term_len = 0;

            while j < data.len() {
                if data[j] == 0x07 {
                    term_len = 1;
                    break;
                }
                if data[j..].starts_with(b"\x1b\\") {
                    term_len = 2;
                    break;
                }
                j += 1;
            }

            if term_len > 0 {
                let payload = &data[payload_start..j];
                if let Ok(payload_str) = std::str::from_utf8(payload) {
                    if let Some(event) = parse_osc_sequence(payload_str) {
                        let (cx, cy) = term.cursor_position();
                        command_state.handle_osc(event, cy, cx);
                    }
                }
                i = j + term_len;
                last = i;
            } else {
                // Unterminated OSC sequence at chunk boundary: buffer for next chunk
                residual.extend_from_slice(&data[i..]);
                return clean_output;
            }
        } else {
            i += 1;
        }
    }

    if last < data.len() {
        let tail = &data[last..];
        let osc_prefix = b"\x1b]6973;";
        let mut prefix_matched = 0;
        for len in (1..=osc_prefix.len().min(tail.len())).rev() {
            if tail.ends_with(&osc_prefix[..len]) {
                prefix_matched = len;
                break;
            }
        }
        if prefix_matched > 0 {
            let safe_len = tail.len() - prefix_matched;
            if safe_len > 0 {
                let slice = &tail[..safe_len];
                term.process(slice);
                clean_output.extend_from_slice(slice);
            }
            residual.extend_from_slice(&tail[safe_len..]);
        } else {
            term.process(tail);
            clean_output.extend_from_slice(tail);
        }
    }

    clean_output
}

/// Determines whether file suggestions should be included alongside existing provider suggestions.
///
/// If subcommands, commands, PowerShell cmdlets, or options were already found,
/// file completions are only included if the active token contains `/`, `\`, or starts with `.`.
pub fn should_include_files<'a>(
    active_token: &str,
    existing: impl IntoIterator<Item = &'a Suggestion>,
) -> bool {
    let has_gating_suggestion = existing.into_iter().any(|s| {
        matches!(
            s.kind,
            SuggestionKind::Subcommand
                | SuggestionKind::Command
                | SuggestionKind::PowerShellCmdlet
                | SuggestionKind::Option
        )
    });

    if !has_gating_suggestion {
        return true;
    }

    active_token.contains('/') || active_token.contains('\\') || active_token.starts_with('.')
}

pub struct App {
    pub config: Config,
    pub theme: crate::ui::theme::Theme,
    pub override_shell: Option<String>,
}

impl App {
    pub fn new(config: Config, override_shell: Option<String>) -> Self {
        let theme = crate::ui::theme::Theme::from_config(&config);
        Self {
            config,
            theme,
            override_shell,
        }
    }

    pub async fn run(&mut self) -> Result<u32> {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let shell_type = detect_shell(self.override_shell.as_deref());
        let script_path = get_shell_integration_path()?;

        let mut pty_session = ConPtySession::spawn(shell_type, cols, rows, &script_path)?;
        let _raw_guard = RawModeGuard::enter()?;

        let mut pty_reader = pty_session.pair.master.try_clone_reader()?;
        let mut pty_writer = pty_session.pair.master.take_writer()?;

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
                    Err(_) => break,
                }
            }
        });

        let mut term = HeadlessTerminal::new(cols, rows);
        let mut command_state = CommandState::default();
        let mut suggestion_state = SuggestionState::new(self.config.max_suggestions);

        let file_provider = FileProvider::new();
        let json_spec_provider = default_json_spec_provider();
        let zoxide_provider = ZoxideProvider::default();
        let carapace_provider = CarapaceProvider::default();
        let powershell_provider = PowerShellProvider::new(shell_type);

        let mut dropdown_layout: Option<DropdownLayout> = None;
        let mut event_stream = crossterm::event::EventStream::new();
        let mut stdout = std::io::stdout();
        let mut osc_residual: Vec<u8> = Vec::new();

        loop {
            tokio::select! {
                // PTY Output chunk
                chunk = pty_rx.recv() => {
                    let chunk = match chunk {
                        Some(c) => c,
                        None => break,
                    };

                    // Combine with any previous unterminated OSC residual
                    let mut data_to_process = std::mem::take(&mut osc_residual);
                    data_to_process.extend_from_slice(&chunk);

                    // 1. Strip Win32 / Kitty sequences
                    let sanitized = sanitize_output_stream(&data_to_process);

                    // 2. Scan OSC sequences, update terminal and command state, and get clean terminal bytes (OSC 6973 stripped)
                    let clean = scan_and_handle_osc(&sanitized, &mut term, &mut command_state, &mut osc_residual);

                    // 3. If dropdown was visible and terminal output arrived, clear dropdown to avoid visual tearing
                    if let Some(layout) = dropdown_layout.take() {
                        let _ = Renderer::clear_dropdown(&layout, &term, &mut stdout);
                        suggestion_state.dismiss();
                    }

                    // 4. Print clean bytes to stdout (no internal OSC sequences leak to host) and flush
                    if !clean.is_empty() {
                        let _ = stdout.write_all(&clean);
                        let _ = stdout.flush();
                    }

                    // 5. If NOT in alternate buffer and prompt ended (!command_state.in_prompt):
                    if !term.is_alternate_buffer() && !command_state.in_prompt {
                        if let (Some(p_row), Some(p_col)) = (command_state.prompt_line, command_state.prompt_end_x) {
                            command_state.command_text = term.extract_command_text(p_row, p_col);
                        }
                    }
                }

                // Host Key / Terminal Events
                maybe_event = event_stream.next() => {
                    let event = match maybe_event {
                        Some(Ok(ev)) => ev,
                        _ => continue,
                    };

                    match event {
                        crossterm::event::Event::Resize(new_cols, new_rows) => {
                            let _ = pty_session.resize(new_cols, new_rows);
                            term.resize(new_cols, new_rows);
                            if let Some(layout) = dropdown_layout.take() {
                                let _ = Renderer::clear_dropdown(&layout, &term, &mut stdout);
                            }
                            suggestion_state.dismiss();
                        }
                        crossterm::event::Event::Key(key_event) => {
                            if key_event.kind == crossterm::event::KeyEventKind::Release {
                                continue;
                            }

                            match classify_key(&key_event) {
                                ActionKey::MenuDown => {
                                    if suggestion_state.visible {
                                        suggestion_state.move_down();
                                        let (cx, cy) = term.cursor_position();
                                        dropdown_layout = Renderer::render_dropdown(&suggestion_state, &term, &self.theme, cx, cy, &mut stdout).ok().flatten();
                                    } else {
                                        let encoded = encode_key_event(&key_event);
                                        if !encoded.is_empty() {
                                            let _ = pty_writer.write_all(&encoded);
                                            let _ = pty_writer.flush();
                                        }
                                    }
                                }
                                ActionKey::MenuUp => {
                                    if suggestion_state.visible {
                                        suggestion_state.move_up();
                                        let (cx, cy) = term.cursor_position();
                                        dropdown_layout = Renderer::render_dropdown(&suggestion_state, &term, &self.theme, cx, cy, &mut stdout).ok().flatten();
                                    } else {
                                        let encoded = encode_key_event(&key_event);
                                        if !encoded.is_empty() {
                                            let _ = pty_writer.write_all(&encoded);
                                            let _ = pty_writer.flush();
                                        }
                                    }
                                }
                                ActionKey::DismissMenu => {
                                    if suggestion_state.visible {
                                        if let Some(layout) = dropdown_layout.take() {
                                            let _ = Renderer::clear_dropdown(&layout, &term, &mut stdout);
                                        }
                                        suggestion_state.dismiss();
                                    } else {
                                        let encoded = encode_key_event(&key_event);
                                        if !encoded.is_empty() {
                                            let _ = pty_writer.write_all(&encoded);
                                            let _ = pty_writer.flush();
                                        }
                                    }
                                }
                                ActionKey::AcceptSuggestion => {
                                    if suggestion_state.visible {
                                        let selected_name = suggestion_state.active_item().map(|s| s.name.clone());
                                        if let Some(layout) = dropdown_layout.take() {
                                            let _ = Renderer::clear_dropdown(&layout, &term, &mut stdout);
                                        }
                                        suggestion_state.dismiss();

                                        if let Some(selected_name) = selected_name {
                                            let tokens = lex_command_line(&command_state.command_text);
                                            let active_token_text = tokens.last().map(|t| t.text.as_str()).unwrap_or("");
                                            let replacement = calculate_replacement(active_token_text, &selected_name);

                                            let mut write_buf = Vec::new();
                                            for _ in 0..replacement.backspace_count {
                                                write_buf.push(0x7f);
                                            }
                                            write_buf.extend_from_slice(replacement.insert_text.as_bytes());
                                            let _ = pty_writer.write_all(&write_buf);
                                            let _ = pty_writer.flush();
                                        }
                                    } else {
                                        if let (Some(p_row), Some(p_col)) = (command_state.prompt_line, command_state.prompt_end_x) {
                                            command_state.command_text = term.extract_command_text(p_row, p_col);
                                        }

                                        if command_state.command_text.trim().is_empty() {
                                            let _ = pty_writer.write_all(b"\t");
                                            let _ = pty_writer.flush();
                                        } else {
                                            let tokens = lex_command_line(&command_state.command_text);
                                            let root_cmd = tokens.first().map(|t| t.text.as_str()).unwrap_or("");
                                            let active_token_text = tokens.last().map(|t| t.text.as_str()).unwrap_or("");

                                            let json_sugs = if json_spec_provider.can_handle(root_cmd) {
                                                json_spec_provider.complete(&command_state.command_text, &command_state.cwd).await
                                            } else {
                                                Vec::new()
                                            };

                                            let zoxide_sugs = if zoxide_provider.can_handle(root_cmd) {
                                                zoxide_provider.complete(&command_state.command_text, &command_state.cwd).await
                                            } else {
                                                Vec::new()
                                            };

                                            let carapace_sugs = if json_sugs.is_empty() && carapace_provider.can_handle(root_cmd) {
                                                carapace_provider.complete(&command_state.command_text, &command_state.cwd).await
                                            } else {
                                                Vec::new()
                                            };

                                            let ps_sugs = if json_sugs.is_empty() && powershell_provider.can_handle(root_cmd) {
                                                powershell_provider.complete(&command_state.command_text, &command_state.cwd).await
                                            } else {
                                                Vec::new()
                                            };

                                            let include_files = should_include_files(
                                                active_token_text,
                                                json_sugs.iter().chain(carapace_sugs.iter()).chain(ps_sugs.iter()),
                                            );

                                            let file_sugs = if include_files && file_provider.can_handle(root_cmd) {
                                                file_provider.complete(&command_state.command_text, &command_state.cwd).await
                                            } else {
                                                Vec::new()
                                            };

                                            let mut results = Vec::with_capacity(
                                                json_sugs.len() + zoxide_sugs.len() + carapace_sugs.len() + ps_sugs.len() + file_sugs.len(),
                                            );
                                            results.extend(json_sugs);
                                            results.extend(zoxide_sugs);
                                            results.extend(carapace_sugs);
                                            results.extend(ps_sugs);
                                            results.extend(file_sugs);

                                            results.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.name.cmp(&b.name)));
                                            let mut seen = HashSet::new();
                                            results.retain(|s| seen.insert(s.name.clone()));

                                            if results.is_empty() {
                                                let _ = pty_writer.write_all(b"\t");
                                                let _ = pty_writer.flush();
                                            } else if results.len() == 1 {
                                                let replacement = calculate_replacement(active_token_text, &results[0].name);
                                                let mut write_buf = Vec::new();
                                                for _ in 0..replacement.backspace_count {
                                                    write_buf.push(0x7f);
                                                }
                                                write_buf.extend_from_slice(replacement.insert_text.as_bytes());
                                                let _ = pty_writer.write_all(&write_buf);
                                                let _ = pty_writer.flush();
                                            } else {
                                                suggestion_state.set_suggestions(results);
                                                let (cx, cy) = term.cursor_position();
                                                dropdown_layout = Renderer::render_dropdown(&suggestion_state, &term, &self.theme, cx, cy, &mut stdout).ok().flatten();
                                            }
                                        }
                                    }
                                }
                                ActionKey::Passthrough => {
                                    if suggestion_state.visible {
                                        if let Some(layout) = dropdown_layout.take() {
                                            let _ = Renderer::clear_dropdown(&layout, &term, &mut stdout);
                                        }
                                        suggestion_state.dismiss();
                                    }

                                    if key_event.code == crossterm::event::KeyCode::Enter {
                                        command_state.command_text.clear();
                                        command_state.prompt_end_x = None;
                                    }

                                    let encoded = encode_key_event(&key_event);
                                    if !encoded.is_empty() {
                                        let _ = pty_writer.write_all(&encoded);
                                        let _ = pty_writer.flush();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Clean up on exit
        if let Some(layout) = dropdown_layout.take() {
            let _ = Renderer::clear_dropdown(&layout, &term, &mut stdout);
        }

        let exit_status = pty_session.child.wait()?;
        drop(_raw_guard);

        Ok(exit_status.exit_code())
    }
}
