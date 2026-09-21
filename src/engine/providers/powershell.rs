use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use crate::engine::provider::{CompletionProvider, Suggestion, SuggestionKind};
use crate::pty::shell::{detect_shell, ShellType};

/// Embedded PowerShell completion worker script.
pub const PS_WORKER_SCRIPT: &str = include_str!("../../../assets/psWorker.ps1");

/// Resolves or extracts the PowerShell worker script path.
///
/// If `assets/psWorker.ps1` exists on disk, returns its canonical path.
/// Otherwise, extracts the embedded script to `%TEMP%\shell-panel\psWorker.ps1`.
pub fn get_ps_worker_script_path() -> Result<PathBuf, std::io::Error> {
    let local = Path::new("assets/psWorker.ps1");
    if local.is_file() {
        if let Ok(abs) = local.canonicalize() {
            let s = abs.to_string_lossy();
            let clean = s.strip_prefix(r"\\?\").unwrap_or(&s);
            return Ok(PathBuf::from(clean));
        }
    }

    let temp_dir = std::env::temp_dir().join("shell-panel");
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_path = temp_dir.join("psWorker.ps1");
    let clean_str = temp_path.to_string_lossy();
    let clean = clean_str.strip_prefix(r"\\?\").unwrap_or(&clean_str);
    let target = PathBuf::from(clean);
    std::fs::write(&target, PS_WORKER_SCRIPT)?;
    Ok(target)
}

/// A completion match returned by `System.Management.Automation.CommandCompletion`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PsCompletionMatch {
    pub name: String,
    pub display: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub result_type: String,
}

/// Top-level JSON completion response from `psWorker.ps1`.
#[allow(non_snake_case)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PsCompletionResponse {
    #[serde(default)]
    pub replacementIndex: Option<isize>,
    #[serde(default)]
    pub replacementLength: Option<isize>,
    #[serde(default)]
    pub matches: Vec<PsCompletionMatch>,
}

/// Checks if a command name is a known or likely PowerShell alias.
fn is_powershell_alias(name: &str) -> bool {
    const COMMON_ALIASES: &[&str] = &[
        "cat", "cd", "chdir", "clc", "clear", "clhy", "cli", "clp", "cls", "clv", "cns", "compare",
        "copy", "cp", "cpi", "curl", "cvpa", "dbp", "del", "diff", "dir", "echo", "epal", "epcsv",
        "epsn", "erase", "etsn", "expy", "fc", "fhx", "fl", "foreach", "ft", "fw", "gal", "gbp",
        "gc", "gcb", "gci", "gcm", "gcs", "gdr", "gerr", "ghy", "gi", "gin", "gjb", "gl", "gm",
        "gmo", "gp", "gps", "gpv", "group", "gsn", "gsnp", "gsv", "gtz", "gu", "gv", "gwmi", "h",
        "history", "icm", "iex", "ihy", "ii", "ipal", "ipcsv", "ipmo", "ipsn", "irm", "ise",
        "iwmi", "iwr", "kill", "lp", "ls", "man", "md", "measure", "mi", "mount", "move", "mp",
        "mv", "nal", "ndr", "ni", "nmo", "nsn", "nv", "ogv", "oh", "popd", "ps", "pushd", "pwd",
        "r", "rbp", "rcjb", "rcsn", "rd", "rdr", "ren", "ri", "rjb", "rm", "rmdir", "rmo", "rni",
        "rnp", "rp", "rsn", "rsnp", "ru", "rv", "rvpa", "rwmi", "sajb", "sal", "saps", "sasv",
        "sbp", "sc", "scb", "select", "set", "shcm", "si", "sl", "sleep", "sls", "sort", "sp",
        "spjb", "spps", "spsv", "start", "su", "sv", "swmi", "tee", "trcm", "type", "wget",
        "where", "wjb", "write",
    ];

    let lower = name.to_lowercase();
    if COMMON_ALIASES.contains(&lower.as_str()) {
        return true;
    }

    if !name.contains('-') && !name.contains('.') && name.len() <= 4 {
        return true;
    }

    false
}

/// Parses the JSON output from `psWorker.ps1` into a list of `Suggestion` items.
pub fn parse_powershell_completion_json(json: &str) -> Vec<Suggestion> {
    let resp: PsCompletionResponse = match serde_json::from_str(json) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    resp.matches
        .into_iter()
        .map(|m| {
            let (kind, priority) = match m.result_type.as_str() {
                "Command" => {
                    if is_powershell_alias(&m.name) {
                        (SuggestionKind::Alias, 80)
                    } else {
                        (SuggestionKind::PowerShellCmdlet, 80)
                    }
                }
                "Alias" => (SuggestionKind::Alias, 80),
                "ParameterName" => (SuggestionKind::Option, 75),
                "ProviderContainer" => (SuggestionKind::Directory, 60),
                "ProviderItem" => (SuggestionKind::File, 50),
                _ => (SuggestionKind::Other, 70),
            };

            let description = m
                .description
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty());

            Suggestion::new(m.name, m.display, description, priority).with_kind(kind)
        })
        .collect()
}

struct WorkerRequest {
    text: String,
    cursor: usize,
    response_tx: oneshot::Sender<Vec<Suggestion>>,
}

struct ChildState {
    process: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    lines: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
}

async fn spawn_child(binary_path: &str, script_path: &Path) -> Option<ChildState> {
    let mut cmd = tokio::process::Command::new(binary_path);
    cmd.args(&[
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ]);
    cmd.arg(script_path);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW = 0x08000000
        cmd.creation_flags(0x08000000);
    }

    let mut process = match cmd.spawn() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to spawn PowerShell worker ({}): {}", binary_path, e);
            return None;
        }
    };

    let stdin = process.stdin.take()?;
    let stdout = process.stdout.take()?;
    let lines = tokio::io::BufReader::new(stdout).lines();

    Some(ChildState {
        process,
        stdin,
        lines,
    })
}

async fn run_worker_loop(
    mut request_rx: mpsc::Receiver<WorkerRequest>,
    binary_path: String,
    script_path: PathBuf,
) {
    let mut child: Option<ChildState> = None;
    let mut is_first_query = true;

    while let Some(req) = request_rx.recv().await {
        if child.is_none() {
            child = spawn_child(&binary_path, &script_path).await;
            is_first_query = true;
        }

        let Some(mut running) = child.take() else {
            let _ = req.response_tx.send(Vec::new());
            continue;
        };

        let req_json = match serde_json::to_string(&serde_json::json!({
            "text": req.text,
            "cursor": req.cursor,
        })) {
            Ok(j) => j,
            Err(_) => {
                let _ = req.response_tx.send(Vec::new());
                child = Some(running);
                continue;
            }
        };

        let write_ok = async {
            running.stdin.write_all(req_json.as_bytes()).await?;
            running.stdin.write_all(b"\n").await?;
            running.stdin.flush().await?;
            Ok::<(), std::io::Error>(())
        }
        .await
        .is_ok();

        if !write_ok {
            let _ = running.process.kill().await;
            let _ = req.response_tx.send(Vec::new());
            continue;
        }

        // Initial process start can take up to 3500ms; warm queries enforce 350ms.
        let timeout_duration = if is_first_query {
            Duration::from_millis(3500)
        } else {
            Duration::from_millis(350)
        };

        let mut read_success = false;
        let mut response_suggestions = Vec::new();

        let deadline = tokio::time::Instant::now() + timeout_duration;
        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                break;
            }
            let remaining = deadline - now;
            match tokio::time::timeout(remaining, running.lines.next_line()).await {
                Ok(Ok(Some(line))) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if !trimmed.starts_with('{') {
                        continue;
                    }
                    response_suggestions = parse_powershell_completion_json(trimmed);
                    read_success = true;
                    is_first_query = false;
                    break;
                }
                _ => {
                    break;
                }
            }
        }

        if read_success {
            let _ = req.response_tx.send(response_suggestions);
            child = Some(running);
        } else {
            let _ = running.process.kill().await;
            let _ = req.response_tx.send(Vec::new());
        }
    }

    if let Some(mut running) = child {
        let _ = running.stdin.write_all(b"EXIT\n").await;
        let _ = running.stdin.flush().await;
        let _ = tokio::time::timeout(Duration::from_millis(200), running.process.wait()).await;
        let _ = running.process.kill().await;
    }
}

/// Completion provider that invokes PowerShell's native `CommandCompletion` engine
/// via a persistent background worker process.
#[derive(Clone)]
pub struct PowerShellProvider {
    binary_path: String,
    script_path: PathBuf,
    worker_tx: Arc<tokio::sync::Mutex<Option<mpsc::Sender<WorkerRequest>>>>,
}

impl Default for PowerShellProvider {
    fn default() -> Self {
        Self::new(detect_shell(None))
    }
}

impl PowerShellProvider {
    /// Creates a new `PowerShellProvider` with the specified `ShellType`.
    pub fn new(shell_type: ShellType) -> Self {
        Self::with_binary(shell_type.executable_name())
    }

    /// Creates a new `PowerShellProvider` with the specified `ShellType`.
    pub fn with_shell(shell_type: ShellType) -> Self {
        Self::new(shell_type)
    }

    /// Creates a new `PowerShellProvider` with a custom binary path.
    pub fn with_binary(binary_path: impl Into<String>) -> Self {
        let script_path =
            get_ps_worker_script_path().unwrap_or_else(|_| PathBuf::from("assets/psWorker.ps1"));
        let provider = Self {
            binary_path: binary_path.into(),
            script_path,
            worker_tx: Arc::new(tokio::sync::Mutex::new(None)),
        };

        if tokio::runtime::Handle::try_current().is_ok() {
            let p = provider.clone();
            tokio::spawn(async move {
                let _ = p.get_or_start_worker().await;
            });
        }

        provider
    }

    async fn get_or_start_worker(&self) -> Option<mpsc::Sender<WorkerRequest>> {
        let mut lock = self.worker_tx.lock().await;
        if let Some(ref tx) = *lock {
            if !tx.is_closed() {
                return Some(tx.clone());
            }
        }

        let (worker_tx, worker_rx) = mpsc::channel(32);
        let binary = self.binary_path.clone();
        let script = self.script_path.clone();

        tokio::spawn(async move {
            run_worker_loop(worker_rx, binary, script).await;
        });

        *lock = Some(worker_tx.clone());
        Some(worker_tx)
    }
}

#[async_trait::async_trait]
impl CompletionProvider for PowerShellProvider {
    fn name(&self) -> &'static str {
        "powershell"
    }

    fn can_handle(&self, _cmd: &str) -> bool {
        true
    }

    async fn complete(&self, cmd_line: &str, _cwd: &str) -> Vec<Suggestion> {
        let worker_tx = match self.get_or_start_worker().await {
            Some(tx) => tx,
            None => return Vec::new(),
        };

        let (response_tx, response_rx) = oneshot::channel();
        let req = WorkerRequest {
            text: cmd_line.to_string(),
            cursor: cmd_line.len(),
            response_tx,
        };

        if worker_tx.send(req).await.is_err() {
            return Vec::new();
        }

        match tokio::time::timeout(Duration::from_millis(3500), response_rx).await {
            Ok(Ok(suggestions)) => suggestions,
            _ => Vec::new(),
        }
    }
}
