# Review Fixes Implementation Plan (v2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix every defect, architectural weakness, test gap and documentation gap found in the two project reviews of 2026-09-21, in the agreed order: quick critical fixes first, then the completion redesign, then the remaining items, cleanup and documentation.

**Architecture:** shell-panel wraps PowerShell in a ConPTY, mirrors the screen in a headless `vt100` terminal and draws a completion dropdown on Tab. The redesign (Tasks 7–9) moves everything that needs PowerShell knowledge *into the user's own session*: the integration script is passed with `-EncodedCommand` (no temp file, no execution-policy bypass), wraps `PSConsoleHostReadLine` to mark when PSReadLine is reading a line, and binds **Ctrl+Alt+Shift+F12** to a handler that reports the exact line, cursor and PowerShell's own completions (`CommandCompletion.CompleteInput`) in one `OSC 6973;CMP` message. shell-panel merges that report with Fig specs, carapace and zoxide in a `CompletionEngine`, off the UI loop. The background PowerShell worker, the filesystem provider, the prompt wrapper and all screen scraping are deleted.

**Tech Stack:** Rust 2021, tokio, crossterm 0.28, portable-pty 0.8.1, vt100 0.15.2, clap 4, serde/toml, PowerShell 7 / Windows PowerShell 5.1 with PSReadLine 2.x.

**Spec:** the findings tables below (first review R1, second review R2). Every task names the finding IDs it closes.

## Evidence gathered before planning (spikes, all deleted afterwards)

| Question | Result |
|----------|--------|
| Does a PSReadLine key handler answer through ConPTY? | Yes. Sending `\x1b[24;8~` returned line + cursor in 30–300 ms on pwsh 7 and Windows PowerShell 5.1, also with an oh-my-posh profile. |
| Can the same handler return PowerShell's completions from the *user's* session? | Yes. `$myvar_` completed a variable defined in the session; `[System.IO.Fi` → 17 types with range `ri=1 rl=12`; `Get-` → 483 matches, capped to 100, 11 KB OSC payload delivered intact; `CompleteInput` took 6–250 ms (560 ms worst case on 5.1). |
| Replacement range when the cursor is mid-token? | `Get-Child｜Item` → `ri=0 rl=13 cursor=9`: the range extends past the cursor, so characters after the cursor must be deleted too. |
| Does a `PSConsoleHostReadLine` wrapper survive `function prompt {...}` being replaced? | Yes: `RE` on Enter and a new `RS` on the next read, both shells. |
| Does `-EncodedCommand` work for the integration script? | Yes, both shells (3.5 KB encoded in the spike). |
| Does the current prompt wrapper change what the user's prompt sees? | Yes: the wrapped prompt sees `$?` = `True` after a failed command (baseline `False`), which breaks error indicators of oh-my-posh/starship-style prompts. |
| Where does the child shell start? | In `%USERPROFILE%`: portable-pty defaults the cwd to the home directory when none is set. |
| Can the real binary be tested end to end? | Yes. `shell-panel.exe` inside a test-owned ConPTY: `git sta`+Tab → `git status`, `git `+Tab → dropdown on the mirrored screen; `exit` → process still alive after 8 s (confirms C2). |
| Do win32-input-mode sequences reach PSReadLine? | Yes (portable-pty creates the ConPTY with `PSEUDOCONSOLE_WIN32_INPUT_MODE`): `\x1b[13;28;13;1;16;1_` acted as Shift+Enter; byte `0x08` acted as Ctrl+Backspace, on both shells. |
| Is the code formatted? | No: `cargo fmt --check` reports 81 diffs. |

## Findings being fixed

### First review (R1)

| ID | Finding | Task |
|----|---------|------|
| C1 | `-c` used by both `--check` and `--config`; debug builds panic on start | 2 |
| C2 | shell-panel never exits after `exit` (ConPTY keeps the output pipe open) | 3 |
| C3 | Typed-line capture by screen scraping is wrong under ConPTY (`prompt_end_x=0`, oh-my-posh rows, `echo a > fi` → `"fi"`) | 7, 9 |
| C4 | Carapace invoked as `carapace _carapace export` (completes carapace itself) | 5 |
| C5 | PowerShell worker ignores the shell's cwd | 9 (worker deleted; completion runs in the session) |
| H6 | PowerShell replacement range ignored (`[System.IO.Fi` loses its `[`) | 8 |
| H7 | Dropdown lines not clipped to the right edge | 6 |
| H8 | File names with spaces inserted unquoted; quoted tokens erased with the wrong count | 8, 9 (PowerShell's own, already quoted, path completion replaces `FileProvider`; raw-token erasing for the other sources) |
| H9 | Providers awaited inside the `select!` loop; worker warm-up does not start the process | 9 |
| H10 | `assets/*.ps1` loaded from the current directory and run with `-ExecutionPolicy Bypass` | 7 (script embedded and passed with `-EncodedCommand`; no script file at all) |
| H11 | AltGr characters encoded as control bytes | 10 |
| M1 | Closed `EventStream` makes the loop spin at 100% CPU | 12 |
| M2 | Gray-color ghost-text heuristic truncates input; `prompt_line` goes stale | 9 (code deleted) |
| M3 | Unterminated OSC residual grows without bound | 12 |
| M4 | carapace/zoxide children not killed on timeout | 5 |
| M5 | Dropdown restore drops colors of the covered rows | 13 |
| M6 | Logs go to stderr while the terminal is in raw mode | 14 |
| M7 | No guard against nested sessions | 14 |
| M8 | Unsupported `--shell` values silently run pwsh | 15 |
| M9 | `debounce_ms` unused; config typos and errors invisible | 15 |
| L1 | Dead code, unused dependency, hard-coded version, Portuguese error string, clippy warnings | 2, 16 |
| L2 | Git/docker specs hard-coded in `app.rs`; no user specs | 17 |
| D1 | README out of date; old plans have stale checkboxes | 18 |

### Second review (R2)

| ID | Finding | Task |
|----|---------|------|
| N1 | The shell starts in `%USERPROFILE%`, not in the directory shell-panel was started from | 4 |
| N2 | The prompt wrapper hides `$?` from the user's prompt function | 9 (wrapper removed) |
| N3 | `-ExecutionPolicy Bypass` is applied to the user's whole interactive session | 7 |
| N4 | The completion worker is a separate `-NoProfile` process (no session variables, functions, registered argument completers); a slow query kills it and the next one pays a cold start; no request ids | 9 (deleted) |
| N5 | In short terminals the dropdown is placed over the cursor row (e.g. 8 rows, cursor on row 4, 5 suggestions → rows 3–7) | 6 |
| N6 | The dropdown is cleared *after* new output was applied to the mirror; if that output scrolls, the restored rows are scrolled a second time on the real terminal | 6 |
| N7 | Key encoding gaps: Ctrl+Backspace sends plain backspace, Shift/Ctrl+Enter send plain Enter, modified F-keys lose their modifiers | 10 |
| N8 | A built-in JSON spec hides carapace completely (`git ` shows 13 subcommands although carapace knows ~150); providers run sequentially | 8 |
| N9 | `is_powershell_alias` labels every command of ≤ 4 letters without `-` as an alias (`git`, `node`, `npm`) | 8 |
| N10 | The lexer ignores `(`, `{` and the call operators `&` / `.` (`if ($x) { git sta`, `& git sta`) | 11 |
| N11 | If prompt markers are lost (user redefines `prompt` after start-up) completion state goes stale | 9 (state comes from the ReadLine wrapper) |
| N12 | Host VT processing is never enabled explicitly (legacy conhost started from cmd/Explorer shows raw escapes) | 14 |
| N13 | `ISTERM` is inshellisense's variable name; both tools would confuse each other | 14 |
| T1 | No test runs the real binary; the reactor loop, exit path and Tab flow are untested | 4, 9 |
| T2 | Tautological tests re-implement app logic inside the test (`test_priority_sorting_and_deduplication`, `test_replacement_del_0x7f_encoding`) or test struct literals (`test_line_patch_struct`) | 8, 9, 16 |
| T3 | PTY tests load the developer's profile (slow, environment-dependent), hard-code `ShellType::Pwsh`, and each re-implements the reader thread | 4 |
| T4 | `cargo fmt` was never applied | 1 |

## Architecture alternatives considered

| Part | Alternative | Decision |
|------|-------------|----------|
| Reading the typed line | Screen scraping with prompt markers (current) vs. asking PSReadLine | **PSReadLine report.** Scraping cannot be made correct under ConPTY (markers overtake the text). |
| PowerShell completions | Separate worker process (current) vs. `CompleteInput` inside the user's session | **In session.** Sees the real session state and location, no process management, no cwd sync, one round trip. Cost: completion runs on the shell's thread, exactly like native Tab. |
| "Is PSReadLine reading?" | `prompt` wrapper (current) vs. `PSConsoleHostReadLine` wrapper (what VS Code's shell integration does) | **ReadLine wrapper.** Independent of the prompt function, correct for continuation lines, no `$?` side effect. |
| Script delivery | Temp file + `-ExecutionPolicy Bypass` (current) vs. `-EncodedCommand` | **EncodedCommand.** No file, no policy change, nothing to hijack. |
| Shell → shell-panel channel | OSC through the PTY (current) vs. a named pipe | **Keep OSC.** Verified with 11 KB payloads. Risk: inbox conhost of old Windows 10 builds truncates long OSC strings; if that shows up, switch the report to a named pipe (`\\.\pipe\shell-panel-<pid>`, tokio `net` feature) — the message format stays the same. |
| Inserting the choice | Backspace/Delete + typing (current) vs. `PSConsoleReadLine::Replace` from a handler | **Keep key simulation** (needs no back channel); now driven by PowerShell's exact range. Revisit together with the named pipe. |
| Key encoding | xterm sequences (current) vs. win32-input-mode for every key | **xterm + win32-input-mode only for the gaps** (crossterm does not expose virtual-key codes, so full win32 encoding would be guesswork). |
| Filesystem completion | Own `FileProvider` vs. PowerShell's path completion | **PowerShell's.** Already quoted, provider-aware, relative to the real location; two sources would show duplicates in two styles. |
| Whole product as a PSReadLine module (no PTY wrapper) | Would remove ConPTY, the VT mirror and key encoding, but turns the project into a PowerShell module and loses the path to other shells | **Rejected**; noted for the record. |

## Global Constraints

- Platform: Windows only (ConPTY). Tests that spawn a shell need `pwsh.exe` or `powershell.exe` on PATH; they start it with `-NoProfile`.
- Do not add new crates. `thiserror` is removed (Task 16).
- Code, comments, identifiers, error messages and docs in English.
- Before every commit: `cargo fmt`, then `cargo build --all-targets` without errors and `cargo test` green. If PTY tests time out under parallel load use `cargo test -- --test-threads=4`.
- One commit per task, Conventional Commits style. Every commit message ends with the attribution lines in effect for the executing session.
- Reserved chord: `Ctrl+Alt+Shift+F12`, sent to the PTY as the bytes `\x1b[24;8~`.
- Shell messages: `ESC ] 6973;<payload> BEL` with payloads `RS;<cwd>`, `RE`, `CMP;<json>`. Values are escaped by `__SP-Escape`: every control character, `\` and `;` becomes `\xHH` per UTF-8 byte.
- `CMP` JSON: `{"line":string,"cursor":int,"replacementIndex":int,"replacementLength":int,"matches":[[completionText,listItemText,resultType,toolTip],...]}`; indices are UTF-16 code units; at most 100 matches; tooltips at most 120 characters.
- PowerShell files in this plan contain backslashes: create them with an editor/Write tool, never through a shell heredoc (Git Bash collapses `\\`).

## Execution protocol (token budget)

Tasks stay the unit of commit and of test cycle; **batches** are the unit of delegation. One fresh implementer subagent per batch, one reviewer per batch.

| Batch | Tasks | Implementer model | Review |
|-------|-------|-------------------|--------|
| B1 | 1, 2 | sonnet | controller only (diff stat + test tail) |
| B2 | 3, 4 | sonnet | one reviewer |
| B3 | 5, 6 | sonnet | one reviewer |
| B4 | 7 | opus | spec review + quality review |
| B5 | 8 | opus | spec review + quality review |
| B6 | 9 | opus | spec review + quality review, then the manual checks of Task 9 Step 8 by the user |
| B7 | 10, 11 | sonnet | one reviewer |
| B8 | 12, 13, 14, 15 | sonnet | one reviewer |
| B9 | 16, 17 | sonnet | one reviewer |
| B10 | 18 | sonnet | controller only |

Rules for the controller (the session that dispatches subagents):

1. **Fresh agents, never forks.** A fork inherits the whole controller conversation; a fresh agent starts with only its prompt.
2. **Hand over a file, not the plan.** Before dispatching a batch, cut its task sections plus the "Global Constraints" section out of this file into the scratchpad (`sed -n '<from>,<to>p'`) and give the subagent that path. The subagent must not open this plan, the older plans or the review discussion.
3. **The prompt carries only:** the path of the batch file, the base commit SHA, the working directory, the commit attribution lines, and the report format below. Everything else is in the batch file.
4. **Report format (hard limit 15 lines):** one line per commit (`<sha> <subject>`), the final `test result:` lines of `cargo test`, the last line of `cargo clippy` when the task asks for it, and a list of deviations from the plan with the reason. No file contents, no diffs, no logs.
5. **Subagents keep their own context small:** read only the files named in the task's **Files** block (plus what the compiler points at); pipe long command output through `tail`/`grep`; never paste build logs into the report.
6. **Verification by the controller is cheap and mandatory:** `git log --oneline <base>..HEAD`, `git diff --stat <base>..HEAD`, and `cargo test 2>&1 | grep "test result"`. The controller does not read the diff itself unless a reviewer flags something.
7. **Reviewers** receive the batch file path and the commit range, read the diff themselves (`git diff <base>..HEAD`), and answer with at most 10 findings, one line each (`path:line — problem — fix`), or `APPROVED`.
8. **Fix loops** go back to the same implementer with `SendMessage` (its context already holds the task); a new agent is started only when the implementer is gone.
9. **Stop conditions:** a subagent that cannot make a step pass after two attempts stops and reports the failing command and its last 20 lines of output, instead of improvising a redesign.

Workspace settings that support this (`.claude/settings.local.json`, not committed): claude.ai connectors and synced skills/plugins off, the `gobsidian`/`hostinger-api` MCP servers off, unrelated user skills hidden. Kept: `superpowers` (execution skills), `caveman` and `context-mode` (both reduce tokens). Restart the session after changing that file.

## File map

| File | Status | Responsibility after this plan |
|------|--------|--------------------------------|
| `src/cli.rs` | create (T2) | clap `Cli` |
| `src/main.rs` | modify | guards, logging, config, launch |
| `src/core/app.rs` | rewrite (T9) | reactor loop only |
| `src/core/config.rs` | modify (T15) | config model, loading with warnings |
| `src/pty/conpty.rs` | modify (T3, T4, T7) | spawn in cwd with `-EncodedCommand`, `watch_exit`, `SpawnOptions` |
| `src/shell/integration.rs` | create (T7) | embedded script, base64/UTF-16 encoding |
| `src/shell/osc.rs` | modify (T7, T9) | message parsing, `REPORT_REQUEST_KEY` |
| `src/shell/report.rs` | create (T7) | `ShellReport`, `ShellMatch` |
| `src/shell/command_state.rs` | rewrite (T9) | cwd, `reading_line`, last report |
| `src/shell/stream.rs` | create (T9) | PTY chunk ingestion |
| `src/engine/aggregate.rs` | create (T8) | `CompletionEngine`, merge rules, `plan_replacement` |
| `src/engine/replacement.rs` | modify (T8) | replacement math incl. forward deletes, `to_bytes` |
| `src/engine/lexer.rs` | modify (T8, T11) | `active_token_raw`, `(`/`{`/`&` handling |
| `src/engine/providers/powershell.rs`, `files.rs`, `assets/psWorker.ps1` | delete (T9) | — |
| `src/vt/emulator.rs` | modify (T9) | plain headless terminal |
| `src/ui/renderer.rs`, `suggestion_state.rs`, `patch.rs` | modify (T6, T13) | geometry, colored restore |
| `src/io/key_event.rs` | modify (T10) | AltGr, missing chords |
| `assets/shellIntegration.ps1` | rewrite (T7, T9) | ReadLine wrapper + report handler |
| `assets/specs/*.json` | create (T17) | embedded Fig specs |
| `tests/common/mod.rs` | create (T4) | PTY test harness |
| `tests/e2e_binary_test.rs` | create (T4), extend (T9) | end-to-end tests of the real binary |

---

## Phase A — baseline and quick critical fixes

### Task 1: Format the code base (T4)

**Files:** every `.rs` file (mechanical).

- [ ] **Step 1:** Run `cargo fmt`.
- [ ] **Step 2:** Run `cargo fmt --check` → no output. Run `cargo test` → all pass.
- [ ] **Step 3: Commit**

```bash
git add -A src tests
git commit -m "style: apply rustfmt"
```

---

### Task 2: Fix the clashing `-c` flag and take version/author from Cargo (C1, part of L1)

**Files:**
- Create: `src/cli.rs`, `tests/cli_test.rs`
- Modify: `src/lib.rs`, `src/main.rs`

**Interfaces:**
- Produces: `shell_panel::cli::Cli { shell: Option<String>, verbose: bool, check: bool, config: Option<PathBuf>, print_default_config: bool }`. `--check` keeps `-c`; `--config` is long-only. (Task 4 adds `no_profile`.)

- [ ] **Step 1: Write the failing test** — create `tests/cli_test.rs`:

```rust
use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use shell_panel::cli::Cli;

#[test]
fn test_cli_definition_is_valid() {
    // clap's debug assertions are what panicked at startup when two args shared `-c`.
    Cli::command().debug_assert();
}

#[test]
fn test_check_keeps_short_flag_and_config_is_long_only() {
    let cli = Cli::try_parse_from(["shell-panel", "-c", "--config", "x.toml"]).unwrap();
    assert!(cli.check);
    assert_eq!(cli.config, Some(PathBuf::from("x.toml")));
}

#[test]
fn test_version_comes_from_cargo_manifest() {
    assert_eq!(Cli::command().get_version(), Some(env!("CARGO_PKG_VERSION")));
}
```

- [ ] **Step 2:** Run `cargo test --test cli_test` → compile error `unresolved import shell_panel::cli`.

- [ ] **Step 3: Create `src/cli.rs`**

```rust
use std::path::PathBuf;

use clap::Parser;

/// Command-line arguments for shell-panel.
#[derive(Parser, Debug)]
#[command(name = "shell-panel", version, author, about, long_about = None)]
pub struct Cli {
    /// Shell to run (pwsh or powershell). Defaults to pwsh.exe when it is on PATH.
    #[arg(short, long)]
    pub shell: Option<String>,

    /// Enable verbose debug logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Check if currently running inside a shell-panel session
    #[arg(short, long)]
    pub check: bool,

    /// Path to TOML configuration file (defaults to ~/.config/shell-panel.toml)
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Print default sample configuration in TOML format
    #[arg(long)]
    pub print_default_config: bool,
}
```

Add `pub mod cli;` as the first line of `src/lib.rs`. In `src/main.rs` delete the `Cli` struct and its attributes, and make the imports:

```rust
use clap::Parser;
use shell_panel::cli::Cli;
use shell_panel::core;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
```

- [ ] **Step 4:** `cargo test --test cli_test` → 3 passed. `cargo run -q -- --help` prints help without panicking. `cargo test` → all pass.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/lib.rs src/main.rs tests/cli_test.rs
git commit -m "fix(cli): resolve -c clash between --check and --config and read version from Cargo"
```

---

### Task 3: Exit when the shell exits (C2)

**Files:**
- Modify: `src/pty/conpty.rs`, `src/core/app.rs` (`App::run`)
- Test: `tests/pty_test.rs`

**Interfaces:**
- Produces: `pub fn watch_exit(child: Box<dyn portable_pty::Child + Send + Sync>) -> tokio::sync::oneshot::Receiver<u32>` in `shell_panel::pty::conpty`.

- [ ] **Step 1: Write the failing test** — append to `tests/pty_test.rs` (imports at the top of the file):

```rust
use std::io::Write;
use std::time::Duration;

use shell_panel::core::app::get_shell_integration_path;
use shell_panel::pty::conpty::watch_exit;

#[tokio::test]
async fn test_watch_exit_reports_code_although_pty_output_stays_open() {
    let script = get_shell_integration_path().unwrap();
    let ConPtySession { pair, child } =
        ConPtySession::spawn(detect_shell(None), 80, 24, &script).unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let exit_rx = watch_exit(child);

    // Input typed before PSReadLine is up stays in the console input buffer.
    tokio::time::sleep(Duration::from_secs(3)).await;
    writer.write_all(b"exit 3\r").unwrap();
    writer.flush().unwrap();

    let code = tokio::time::timeout(Duration::from_secs(30), exit_rx)
        .await
        .expect("shell did not report exit")
        .expect("watcher thread dropped the sender");
    assert_eq!(code, 3);
    drop(pair);
}
```

- [ ] **Step 2:** `cargo test --test pty_test` → compile error `cannot find function watch_exit`.

- [ ] **Step 3: Implement** — append to `src/pty/conpty.rs`:

```rust
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
```

- [ ] **Step 4: Use it in `App::run`** (`src/core/app.rs`)

1. Imports: add `use std::time::Duration;` and `use portable_pty::PtySize;`; change the conpty import to `use crate::pty::conpty::{watch_exit, ConPtySession};`.
2. Replace the spawn block with:
   ```rust
   let ConPtySession { pair, child } = ConPtySession::spawn(shell_type, cols, rows, &script_path)?;
   let mut exit_rx = watch_exit(child);
   let _raw_guard = RawModeGuard::enter()?;

   let mut pty_reader = pair.master.try_clone_reader()?;
   let mut pty_writer = pair.master.take_writer()?;
   ```
3. Before `loop {` add `let mut exit_code: Option<u32> = None;`.
4. First branch inside `tokio::select! {`:
   ```rust
   code = &mut exit_rx => {
       // The shell's last output may still be in flight: drain until the PTY is quiet.
       let drain_until = tokio::time::Instant::now() + Duration::from_secs(1);
       while let Ok(Some(chunk)) = tokio::time::timeout_at(
           drain_until.min(tokio::time::Instant::now() + Duration::from_millis(100)),
           pty_rx.recv(),
       ).await {
           let mut data = std::mem::take(&mut osc_residual);
           data.extend_from_slice(&chunk);
           let sanitized = sanitize_output_stream(&data);
           let clean = scan_and_handle_osc(&sanitized, &mut term, &mut command_state, &mut osc_residual);
           let _ = stdout.write_all(&clean);
       }
       let _ = stdout.flush();
       exit_code = Some(code.unwrap_or(1));
       break;
   }
   ```
5. Resize branch: replace `pty_session.resize(new_cols, new_rows)` with
   `pair.master.resize(PtySize { rows: new_rows, cols: new_cols, pixel_width: 0, pixel_height: 0 })`.
6. Replace the end of `run` (from `let exit_status = pty_session.child.wait()?;`) with:
   ```rust
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
   ```

- [ ] **Step 5:** `cargo test` → all pass.

- [ ] **Step 6: Commit**

```bash
git add src/pty/conpty.rs src/core/app.rs tests/pty_test.rs
git commit -m "fix(core): exit when the shell exits instead of waiting for PTY EOF"
```

---

### Task 4: Start the shell in the current directory, add `--no-profile`, build the PTY test harness and the first end-to-end test (N1, T1, T3)

**Files:**
- Create: `tests/common/mod.rs`, `tests/e2e_binary_test.rs`
- Modify: `src/pty/conpty.rs`, `src/cli.rs`, `src/main.rs`, `src/core/app.rs`, `tests/pty_test.rs`, `tests/e2e_pty_test.rs`

**Interfaces:**
- Produces:
  - `#[derive(Debug, Clone, Copy, Default)] pub struct SpawnOptions { pub no_profile: bool }` and `ConPtySession::spawn(shell_type, cols, rows, script_path: &Path, options: SpawnOptions)`.
  - `Cli.no_profile: bool` (`--no-profile`), `App.no_profile: bool` (public field, default `false`).
  - Test harness `common::Terminal` with `spawn(CommandBuilder)`, `attach(master, child)`, `shell_panel(cwd: &Path)`, `send(&[u8])`, `screen() -> String`, `raw: Vec<u8>`, `wait_until(Duration, Fn(&Terminal) -> bool) -> bool`, `wait_for_text(&str, Duration) -> bool`, `wait_for_raw(&[u8], Duration) -> bool`, `wait_exit(Duration) -> Option<u32>`.

- [ ] **Step 1: Create the harness** — `tests/common/mod.rs`:

```rust
#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

pub const COLS: u16 = 120;
pub const ROWS: u16 = 30;

/// A process running in a test-owned ConPTY, with its screen mirrored by `vt100`.
pub struct Terminal {
    parser: vt100::Parser,
    /// Every byte received so far (OSC reports included).
    pub raw: Vec<u8>,
    rx: Receiver<Vec<u8>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
}

impl Terminal {
    pub fn spawn(cmd: CommandBuilder) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize { rows: ROWS, cols: COLS, pixel_width: 0, pixel_height: 0 })
            .expect("openpty");
        let child = pair.slave.spawn_command(cmd).expect("spawn");
        Self::attach(pair.master, child)
    }

    pub fn attach(master: Box<dyn MasterPty + Send>, child: Box<dyn Child + Send + Sync>) -> Self {
        let mut reader = master.try_clone_reader().expect("reader");
        let writer = master.take_writer().expect("writer");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self {
            parser: vt100::Parser::new(ROWS, COLS, 0),
            raw: Vec::new(),
            rx,
            writer,
            child,
            _master: master,
        }
    }

    /// Starts the real shell-panel binary with `--no-profile` in `cwd`.
    pub fn shell_panel(cwd: &Path) -> Self {
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_shell-panel"));
        cmd.arg("--no-profile");
        cmd.cwd(cwd);
        // `cargo test` may itself be running inside a shell-panel session.
        cmd.env_remove("ISTERM");
        cmd.env_remove("SHELL_PANEL_SESSION");
        Self::spawn(cmd)
    }

    fn pump(&mut self, wait: Duration) {
        if let Ok(bytes) = self.rx.recv_timeout(wait) {
            self.parser.process(&bytes);
            self.raw.extend_from_slice(&bytes);
        }
    }

    pub fn wait_until(&mut self, timeout: Duration, pred: impl Fn(&Terminal) -> bool) -> bool {
        let start = Instant::now();
        loop {
            if pred(self) {
                return true;
            }
            if start.elapsed() > timeout {
                return false;
            }
            self.pump(Duration::from_millis(50));
        }
    }

    pub fn wait_for_text(&mut self, text: &str, timeout: Duration) -> bool {
        self.wait_until(timeout, |t| t.screen().contains(text))
    }

    pub fn wait_for_raw(&mut self, needle: &[u8], timeout: Duration) -> bool {
        self.wait_until(timeout, |t| t.raw.windows(needle.len()).any(|w| w == needle))
    }

    pub fn screen(&self) -> String {
        self.parser.screen().contents()
    }

    pub fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).expect("write to pty");
        self.writer.flush().expect("flush pty");
    }

    pub fn wait_exit(&mut self, timeout: Duration) -> Option<u32> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Some(status.exit_code());
            }
            self.pump(Duration::from_millis(50));
        }
        None
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
```

- [ ] **Step 2: Write the failing end-to-end test** — `tests/e2e_binary_test.rs`:

```rust
mod common;

use std::time::Duration;

use common::Terminal;

const START: Duration = Duration::from_secs(40);
const STEP: Duration = Duration::from_secs(15);

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sp_e2e_{}_{}", tag, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_shell_starts_in_the_directory_shell_panel_was_started_from() {
    let dir = temp_dir("cwd");
    let mut term = Terminal::shell_panel(&dir);
    // The default prompt is `PS <path>> `.
    let name = dir.file_name().unwrap().to_str().unwrap().to_string();
    assert!(term.wait_for_text(&name, START), "screen: {}", term.screen());
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_tab_inserts_single_match_opens_dropdown_and_exit_code_propagates() {
    let dir = temp_dir("tab");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());

    term.send(b"git sta");
    assert!(term.wait_for_text("git sta", STEP));
    term.send(b"\t");
    assert!(term.wait_for_text("git status", STEP), "screen: {}", term.screen());

    term.send(b"\x1b"); // Esc: PSReadLine reverts the line
    term.send(b"git ");
    // Wait until the mirror shows exactly the new line: the old `git status` also contains `> git`.
    assert!(term.wait_until(STEP, |t| t.screen().trim_end().ends_with("> git")), "screen: {}", term.screen());
    term.send(b"\t");
    assert!(term.wait_for_text("Record changes to the repository", STEP), "screen: {}", term.screen());

    term.send(b"\x1b"); // closes the dropdown
    assert!(
        term.wait_until(STEP, |t| !t.screen().contains("Record changes to the repository")),
        "dropdown not cleared: {}",
        term.screen()
    );

    term.send(b"\x1b");
    term.send(b"exit 5\r");
    assert_eq!(term.wait_exit(STEP), Some(5));
}
```

- [ ] **Step 3:** `cargo test --test e2e_binary_test` → both fail (`--no-profile` is an unknown argument, so the binary exits with code 2 at once).

- [ ] **Step 4: Implement**

`src/pty/conpty.rs`:

```rust
/// Options for [`ConPtySession::spawn`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SpawnOptions {
    /// Start PowerShell with `-NoProfile`.
    pub no_profile: bool,
}
```

Change `spawn` to take `options: SpawnOptions` as last parameter, and in its body, after `CommandBuilder::new(...)`:

```rust
        // portable-pty starts the child in the home directory unless a cwd is given.
        cmd.cwd(std::env::current_dir()?);
```

and after `cmd.arg("-NoLogo");`:

```rust
        if options.no_profile {
            cmd.arg("-NoProfile");
        }
```

`src/cli.rs` — add the field:

```rust
    /// Start PowerShell without loading profiles
    #[arg(long)]
    pub no_profile: bool,
```

`src/core/app.rs` — add `pub no_profile: bool,` to `App`, `no_profile: false,` in `App::new`, import `SpawnOptions`, and spawn with `ConPtySession::spawn(shell_type, cols, rows, &script_path, SpawnOptions { no_profile: self.no_profile })?`.

`src/main.rs` — after `let mut app = ...;` add `app.no_profile = cli.no_profile;`.

Update the other callers to pass `SpawnOptions::default()` plus the import `use shell_panel::pty::conpty::SpawnOptions;`: `tests/pty_test.rs` (two calls), `tests/e2e_pty_test.rs` (one call). In those tests use `SpawnOptions { no_profile: true }` so they stop loading the developer's profile.

- [ ] **Step 5:** `cargo test` → all pass, including both new end-to-end tests.

- [ ] **Step 6: Commit**

```bash
git add src/pty/conpty.rs src/cli.rs src/main.rs src/core/app.rs tests/common tests/e2e_binary_test.rs tests/pty_test.rs tests/e2e_pty_test.rs
git commit -m "fix(pty): start the shell in the current directory; add --no-profile and end-to-end tests of the binary"
```

---

### Task 5: Call carapace with the correct syntax and kill timed-out helpers (C4, M4)

**Files:**
- Modify: `src/engine/providers/carapace.rs`, `src/engine/providers/zoxide.rs`
- Test: `tests/engine_test.rs`

**Interfaces:**
- Produces: `pub fn carapace_args(tokens: &[CommandToken]) -> Vec<String>` in `shell_panel::engine::providers::carapace`.

- [ ] **Step 1: Write the failing test** — append to `tests/engine_test.rs` (extend the carapace import with `carapace_args`, add `use shell_panel::engine::lexer::lex_command_line;`):

```rust
#[test]
fn test_carapace_args_use_completer_export_form() {
    // `carapace <completer> export <completer> <args...>`; `_carapace` would complete carapace itself.
    assert_eq!(carapace_args(&lex_command_line("git sta")), vec!["git", "export", "git", "sta"]);
    assert_eq!(carapace_args(&lex_command_line("npm ")), vec!["npm", "export", "npm", ""]);
    assert!(carapace_args(&lex_command_line("")).is_empty());
}
```

- [ ] **Step 2:** `cargo test --test engine_test test_carapace_args` → compile error.

- [ ] **Step 3: Implement** — in `carapace.rs` import `use crate::engine::lexer::{lex_command_line, CommandToken};` and add:

```rust
/// Builds the arguments for `carapace <completer> export <completer> <args...>`.
pub fn carapace_args(tokens: &[CommandToken]) -> Vec<String> {
    let Some(root) = tokens.first() else {
        return Vec::new();
    };
    let mut args = vec![root.text.clone(), "export".to_string()];
    args.extend(tokens.iter().map(|t| t.text.clone()));
    args
}
```

Replace the body of `complete`:

```rust
    async fn complete(&self, cmd_line: &str, cwd: &str) -> Vec<Suggestion> {
        let args = carapace_args(&lex_command_line(cmd_line));
        if args.is_empty() {
            return Vec::new();
        }

        let mut cmd = Command::new(&self.binary_path);
        cmd.args(&args).kill_on_drop(true);
        if !cwd.is_empty() {
            cmd.current_dir(cwd);
        }

        let output = match tokio::time::timeout(std::time::Duration::from_millis(200), cmd.output()).await {
            Ok(Ok(out)) if out.status.success() => out,
            _ => return Vec::new(),
        };

        parse_carapace_json(&String::from_utf8_lossy(&output.stdout))
    }
```

In `zoxide.rs` add `.kill_on_drop(true)` before `.output()`.

- [ ] **Step 4:** `cargo test` → all pass.

- [ ] **Step 5: Commit**

```bash
git add src/engine/providers/carapace.rs src/engine/providers/zoxide.rs tests/engine_test.rs
git commit -m "fix(engine): invoke carapace as '<cmd> export <cmd> args' and kill timed-out helpers"
```

---

### Task 6: Dropdown geometry — clip to the right edge, never cover the cursor row, clear before new output (H7, N5, N6)

**Files:**
- Modify: `src/ui/suggestion_state.rs`, `src/ui/renderer.rs`, `src/core/app.rs` (PTY branch)
- Test: `tests/renderer_test.rs`

**Interfaces:**
- Produces: `SuggestionState::visible_page_with(&self, page_rows: usize) -> Vec<(&Suggestion, bool)>`; `visible_page()` keeps working (`page_rows = max_rows`).

- [ ] **Step 1: Write failing tests** — append to `tests/renderer_test.rs`:

```rust
fn many(n: usize) -> Vec<Suggestion> {
    (0..n)
        .map(|i| Suggestion::new(format!("item{i}"), format!("item{i}"), None, 50))
        .collect()
}

#[test]
fn test_render_dropdown_never_writes_past_right_edge() {
    let term = HeadlessTerminal::new(80, 24);
    let mut state = SuggestionState::new(5);
    state.set_suggestions(vec![
        Suggestion::new("a-very-long-suggestion-name", "a-very-long-suggestion-name", Some("with a long description".into()), 80),
        Suggestion::new("second", "second", None, 70),
    ]);

    let mut out = Vec::new();
    Renderer::render_dropdown(&state, &term, &Theme::default(), 70, 5, &mut out).unwrap();

    // Replay on a blank screen: the panel starts at column 70, anything left of it wrapped.
    let mut screen = HeadlessTerminal::new(80, 24);
    screen.process(&out);
    for row in 0..24 {
        for col in 0..70 {
            let cell = screen.screen().cell(row, col).unwrap();
            assert!(cell.contents().is_empty(), "overflow wrote {:?} at row {} col {}", cell.contents(), row, col);
        }
    }
}

#[test]
fn test_dropdown_never_covers_the_cursor_row() {
    // 8 rows, cursor on row 4, 5 suggestions wanted: 3 rows below, 4 above -> 4 rows above.
    let term = HeadlessTerminal::new(80, 8);
    let mut state = SuggestionState::new(5);
    state.set_suggestions(many(12));

    let mut out = Vec::new();
    let layout = Renderer::render_dropdown(&state, &term, &Theme::default(), 0, 4, &mut out)
        .unwrap()
        .expect("rendered");

    assert_eq!(layout, DropdownLayout { start_row: 0, row_count: 4 });
}

#[test]
fn test_dropdown_in_one_row_terminal_is_not_rendered() {
    let term = HeadlessTerminal::new(80, 1);
    let mut state = SuggestionState::new(5);
    state.set_suggestions(many(3));
    let mut out = Vec::new();
    assert_eq!(Renderer::render_dropdown(&state, &term, &Theme::default(), 0, 0, &mut out).unwrap(), None);
}

#[test]
fn test_visible_page_with_smaller_page() {
    let mut state = SuggestionState::new(5);
    state.set_suggestions(many(7));
    for _ in 0..4 {
        state.move_down(); // active index 4 -> second page of size 4
    }
    let page = state.visible_page_with(4);
    assert_eq!(page.len(), 3);
    assert!(page[0].1);
    assert_eq!(page[0].0.name, "item4");
}
```

- [ ] **Step 2:** `cargo test --test renderer_test` → compile error (`visible_page_with`), then failures.

- [ ] **Step 3: Implement paging** — in `src/ui/suggestion_state.rs` replace `visible_page` with:

```rust
    /// Returns the suggestions on the current page, each paired with whether it is the active item.
    pub fn visible_page(&self) -> Vec<(&Suggestion, bool)> {
        self.visible_page_with(self.max_rows)
    }

    /// Like [`visible_page`](Self::visible_page) with an explicit page size, for when the
    /// terminal has room for fewer rows than `max_rows`.
    pub fn visible_page_with(&self, page_rows: usize) -> Vec<(&Suggestion, bool)> {
        if self.suggestions.is_empty() {
            return Vec::new();
        }

        let page_rows = page_rows.max(1);
        let start = (self.active_idx / page_rows) * page_rows;
        let end = (start + page_rows).min(self.suggestions.len());

        self.suggestions[start..end]
            .iter()
            .enumerate()
            .map(|(offset, item)| (item, (start + offset) == self.active_idx))
            .collect()
    }
```

- [ ] **Step 4: Implement placement and clipping** — in `Renderer::render_dropdown` replace everything from `let page = state.visible_page();` to the end of the `start_row` computation with:

```rust
        // Rows free below and above the cursor row; the cursor row itself is never covered.
        let below = term.rows.saturating_sub(cursor_y.saturating_add(1)) as usize;
        let above = cursor_y as usize;
        let wanted = state.max_rows.min(state.total_items());
        let (place_below, page_rows) = if wanted <= below {
            (true, wanted)
        } else if wanted <= above {
            (false, wanted)
        } else if below >= above {
            (true, below)
        } else {
            (false, above)
        };
        if page_rows == 0 {
            return Ok(None);
        }

        let page = state.visible_page_with(page_rows);
        let page_len = page.len() as u16;
        let start_row = if place_below {
            cursor_y + 1
        } else {
            cursor_y - page_len
        };
```

and in the call to `format_suggestion_line_with_theme_and_min_width` replace the last argument `term.cols as usize,` with `available_cols,`.

- [ ] **Step 5: Clear the dropdown before new output reaches the mirror** — in `src/core/app.rs`, in the PTY-output branch, move the block

```rust
                    if let Some(layout) = dropdown_layout.take() {
                        let _ = Renderer::clear_dropdown(&layout, &term, &mut stdout);
                        suggestion_state.dismiss();
                    }
```

so that it runs *before* `scan_and_handle_osc` (directly after `data_to_process.extend_from_slice(&chunk);`), with the comment `// Restore the covered rows from the mirror as it was when they were covered; output that scrolls would otherwise be applied twice.`

- [ ] **Step 6:** `cargo test` → all pass (existing above/below tests keep their layouts; the e2e dropdown test still passes).

- [ ] **Step 7: Commit**

```bash
git add src/ui/suggestion_state.rs src/ui/renderer.rs src/core/app.rs tests/renderer_test.rs
git commit -m "fix(ui): clip the dropdown horizontally, keep it off the cursor row and clear it before new output"
```

---

## Phase B — completion from inside the PowerShell session

### Task 7: Shell integration v2 — `-EncodedCommand`, ReadLine markers and the completion report (C3 part 1, H10, N3)

During this task the old prompt wrapper stays in the script so the app keeps working; Task 9 removes it.

**Files:**
- Create: `src/shell/integration.rs`, `src/shell/report.rs`, `tests/shell_report_test.rs`
- Modify: `assets/shellIntegration.ps1`, `src/shell/mod.rs`, `src/shell/osc.rs`, `src/shell/command_state.rs`, `src/pty/conpty.rs`, `src/core/app.rs`, `tests/osc_test.rs`, `tests/pty_test.rs`, `tests/e2e_pty_test.rs`
- Delete: `tests/integration_script_test.rs`

**Interfaces:**
- Produces:
  - `shell_panel::shell::integration::{SCRIPT: &str, base64_encode(&[u8]) -> String, encoded_command() -> String}`.
  - `ConPtySession::spawn(shell_type: ShellType, cols: u16, rows: u16, options: SpawnOptions) -> Result<Self>` (script path parameter removed).
  - `shell_panel::shell::report::{ShellReport, ShellMatch}`:
    `ShellReport { line: String, cursor: usize, replacement_index: i64, replacement_length: i64, matches: Vec<ShellMatch> }`, `ShellMatch(pub String /*completion text*/, pub String /*list item*/, pub String /*result type*/, pub String /*tooltip*/)`,
    `ShellReport::cursor_byte(&self) -> Option<usize>`, `text_before_cursor(&self) -> Option<&str>`, `replacement_range(&self) -> Option<(usize, usize)>` (byte range containing the cursor).
  - `utf16_to_byte_index(s: &str, utf16_idx: usize) -> Option<usize>` in `shell_panel::engine::replacement`.
  - `OscEvent::{ReadLineStarted { cwd: Option<String> }, ReadLineEnded, Report(ShellReport)}`, `pub const REPORT_REQUEST_KEY: &[u8] = b"\x1b[24;8~";`, `unescape_cwd` renamed to `unescape_value`.
  - Removed: `get_shell_integration_path`, `SHELL_INTEGRATION_SCRIPT` (use `integration::SCRIPT`).

- [ ] **Step 1: Write failing unit tests**

Append to `tests/engine_test.rs` (extend the replacement import with `utf16_to_byte_index`):

```rust
#[test]
fn test_utf16_to_byte_index() {
    assert_eq!(utf16_to_byte_index("", 0), Some(0));
    assert_eq!(utf16_to_byte_index("ação x", 4), Some(6));
    assert_eq!(utf16_to_byte_index("ação x", 6), Some(8));
    assert_eq!(utf16_to_byte_index("🚀a", 1), None); // inside a surrogate pair
    assert_eq!(utf16_to_byte_index("🚀a", 2), Some(4));
    assert_eq!(utf16_to_byte_index("ab", 3), None);
}
```

In `tests/osc_test.rs` rename every `unescape_cwd` to `unescape_value` and append:

```rust
use shell_panel::shell::report::{ShellMatch, ShellReport};

#[test]
fn test_parse_readline_markers() {
    assert_eq!(
        parse_osc_sequence(r"6973;RS;C:\x5cdev\x5cx64"),
        Some(OscEvent::ReadLineStarted { cwd: Some(r"C:\dev\x64".to_string()) })
    );
    assert_eq!(parse_osc_sequence("6973;RS;"), Some(OscEvent::ReadLineStarted { cwd: None }));
    assert_eq!(parse_osc_sequence("6973;RE"), Some(OscEvent::ReadLineEnded));
}

#[test]
fn test_parse_completion_report() {
    // The JSON is escaped as a whole: `;` -> \x3b, and each backslash of JSON's `\\` -> \x5c.
    let payload = r#"6973;CMP;{"line":"cd .\x5c\x5cs\x3b","cursor":7,"replacementIndex":3,"replacementLength":4,"matches":[[".\x5c\x5csrc","src","ProviderContainer","C:\x5c\x5cp\x5c\x5csrc"]]}"#;
    assert_eq!(
        parse_osc_sequence(payload),
        Some(OscEvent::Report(ShellReport {
            line: r"cd .\s;".to_string(),
            cursor: 7,
            replacement_index: 3,
            replacement_length: 4,
            matches: vec![ShellMatch(r".\src".into(), "src".into(), "ProviderContainer".into(), r"C:\p\src".into())],
        }))
    );
    assert_eq!(parse_osc_sequence("6973;CMP;not json"), None);
}

#[test]
fn test_report_ranges() {
    let report = ShellReport {
        line: "echo ação Get-ChildItem".into(),
        cursor: 18, // after "Get-Chil" (UTF-16 units)
        replacement_index: 10,
        replacement_length: 13,
        matches: vec![],
    };
    assert_eq!(report.text_before_cursor(), Some("echo ação Get-Chil"));
    let (start, end) = report.replacement_range().unwrap();
    assert_eq!(&report.line[start..end], "Get-ChildItem");

    let bogus = ShellReport { replacement_index: -1, ..report.clone() };
    assert_eq!(bogus.replacement_range(), None);
    // A range that does not contain the cursor cannot be applied with Backspace/Delete.
    let away = ShellReport { replacement_index: 0, replacement_length: 4, ..report };
    assert_eq!(away.replacement_range(), None);
}
```

Create `tests/shell_report_test.rs`:

```rust
mod common;

use std::time::Duration;

use common::{Terminal, COLS, ROWS};
use shell_panel::pty::conpty::{ConPtySession, SpawnOptions};
use shell_panel::pty::shell::detect_shell;
use shell_panel::shell::integration::{base64_encode, encoded_command, SCRIPT};
use shell_panel::shell::osc::{parse_osc_sequence, OscEvent, REPORT_REQUEST_KEY};

#[test]
fn test_base64_encode_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn test_encoded_command_fits_the_windows_command_line() {
    assert!(SCRIPT.contains("PSConsoleHostReadLine"));
    assert!(encoded_command().len() < 30_000, "CreateProcess limit is 32767 characters");
}

fn last_report(raw: &[u8]) -> Option<OscEvent> {
    let marker = b"\x1b]6973;CMP;";
    let start = raw.windows(marker.len()).rposition(|w| w == marker)?;
    let end = start + raw[start..].iter().position(|&b| b == 0x07)?;
    parse_osc_sequence(std::str::from_utf8(&raw[start + 2..end]).ok()?)
}

#[test]
fn test_session_reports_readline_state_line_cursor_and_completions() {
    let ConPtySession { pair, child } =
        ConPtySession::spawn(detect_shell(None), COLS, ROWS, SpawnOptions { no_profile: true }).unwrap();
    let mut term = Terminal::attach(pair.master, child);

    assert!(term.wait_for_raw(b"\x1b]6973;RS;", Duration::from_secs(40)), "no ReadLine marker");

    // A variable that exists only in this session proves completion runs inside it.
    term.send(b"$sp_report_zz = 1\r");
    assert!(term.wait_for_raw(b"\x1b]6973;RE\x07", Duration::from_secs(15)));

    term.send("echo 'ação' > $sp_report_".as_bytes());
    assert!(term.wait_for_text("$sp_report_", Duration::from_secs(15)));
    term.send(REPORT_REQUEST_KEY);
    assert!(term.wait_until(Duration::from_secs(20), |t| last_report(&t.raw).is_some()), "no report");

    let Some(OscEvent::Report(report)) = last_report(&term.raw) else { unreachable!() };
    assert_eq!(report.line, "echo 'ação' > $sp_report_");
    assert_eq!(report.text_before_cursor(), Some("echo 'ação' > $sp_report_"));
    let (start, end) = report.replacement_range().expect("range");
    assert_eq!(&report.line[start..end], "$sp_report_");
    assert!(report.matches.iter().any(|m| m.0 == "$sp_report_zz"), "matches: {:?}", report.matches);
}
```

- [ ] **Step 2:** `cargo test --test osc_test --test shell_report_test --test engine_test` → compile errors.

- [ ] **Step 3: `utf16_to_byte_index`** — append to `src/engine/replacement.rs`:

```rust
/// Converts a UTF-16 code-unit index (the unit PowerShell and .NET use) into a byte index into `s`.
/// Returns `None` when the index is past the end or falls inside a surrogate pair.
pub fn utf16_to_byte_index(s: &str, utf16_idx: usize) -> Option<usize> {
    let mut units = 0;
    for (byte_idx, ch) in s.char_indices() {
        if units == utf16_idx {
            return Some(byte_idx);
        }
        units += ch.len_utf16();
        if units > utf16_idx {
            return None;
        }
    }
    (units == utf16_idx).then_some(s.len())
}
```

- [ ] **Step 4: Create `src/shell/report.rs`**

```rust
use serde::Deserialize;

use crate::engine::replacement::utf16_to_byte_index;

/// One PowerShell completion result: completion text, list item text, result type, tooltip.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShellMatch(pub String, pub String, pub String, pub String);

/// What the PSReadLine handler reports: the edited line, the cursor and PowerShell's completions.
/// All indices are UTF-16 code units, the unit .NET strings use.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellReport {
    pub line: String,
    pub cursor: usize,
    #[serde(default)]
    pub replacement_index: i64,
    #[serde(default)]
    pub replacement_length: i64,
    #[serde(default)]
    pub matches: Vec<ShellMatch>,
}

impl ShellReport {
    /// Byte index of the cursor in `line`.
    pub fn cursor_byte(&self) -> Option<usize> {
        utf16_to_byte_index(&self.line, self.cursor)
    }

    /// The line up to the cursor.
    pub fn text_before_cursor(&self) -> Option<&str> {
        self.cursor_byte().map(|end| &self.line[..end])
    }

    /// Byte range PowerShell wants replaced. `None` when it is malformed or does not contain the
    /// cursor (it is applied with Backspace and Delete, which only work around the cursor).
    pub fn replacement_range(&self) -> Option<(usize, usize)> {
        let index = usize::try_from(self.replacement_index).ok()?;
        let length = usize::try_from(self.replacement_length).ok()?;
        let start = utf16_to_byte_index(&self.line, index)?;
        let end = utf16_to_byte_index(&self.line, index + length)?;
        let cursor = self.cursor_byte()?;
        (start <= cursor && cursor <= end).then_some((start, end))
    }
}
```

Add `pub mod integration;` and `pub mod report;` to `src/shell/mod.rs`.

- [ ] **Step 5: Parser** — in `src/shell/osc.rs`: rename `unescape_cwd` to `unescape_value` (function, doc comment, call sites), add `use crate::shell::report::ShellReport;`, add the variants

```rust
    /// PSReadLine started reading a line; `cwd` is set when the location is a filesystem path.
    ReadLineStarted { cwd: Option<String> },
    /// PSReadLine returned the line (it was accepted or cancelled).
    ReadLineEnded,
    /// Answer to [`REPORT_REQUEST_KEY`].
    Report(ShellReport),
```

the constant

```rust
/// Key sequence for Ctrl+Alt+Shift+F12. The integration script binds this chord to a PSReadLine
/// handler that answers with `OSC 6973;CMP;<json>`.
pub const REPORT_REQUEST_KEY: &[u8] = b"\x1b[24;8~";
```

and, in `parse_osc_sequence` before the final `None`:

```rust
    if body == "RE" {
        return Some(OscEvent::ReadLineEnded);
    }
    if let Some(cwd) = body.strip_prefix("RS;") {
        let cwd = unescape_value(cwd);
        return Some(OscEvent::ReadLineStarted { cwd: (!cwd.is_empty()).then_some(cwd) });
    }
    if let Some(json) = body.strip_prefix("CMP;") {
        return serde_json::from_str(&unescape_value(json)).ok().map(OscEvent::Report);
    }
```

In `src/shell/command_state.rs` add a catch-all arm so the old state machine ignores the new events for now:

```rust
            OscEvent::ReadLineStarted { .. } | OscEvent::ReadLineEnded | OscEvent::Report(_) => {}
```

- [ ] **Step 6: Create `src/shell/integration.rs`**

```rust
/// PowerShell integration script, embedded in the binary and passed with `-EncodedCommand`:
/// no file is written and the user's execution policy is left alone.
pub const SCRIPT: &str = include_str!("../../assets/shellIntegration.ps1");

/// Standard base64 with padding.
pub fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

/// The script as `-EncodedCommand` expects it: base64 of its UTF-16LE bytes.
pub fn encoded_command() -> String {
    let utf16: Vec<u8> = SCRIPT.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64_encode(&utf16)
}
```

- [ ] **Step 7: Spawn with `-EncodedCommand`** — in `src/pty/conpty.rs` remove the `script_path` parameter and the `use std::path::Path;` import, and replace the argument block (from `cmd.arg("-ExecutionPolicy");` to the `cmd.arg(format!("try ...` line) so that the arguments are:

```rust
        cmd.arg("-NoLogo");
        if options.no_profile {
            cmd.arg("-NoProfile");
        }
        cmd.arg("-NoExit");
        cmd.arg("-EncodedCommand");
        cmd.arg(crate::shell::integration::encoded_command());
```

In `src/core/app.rs` delete `SHELL_INTEGRATION_SCRIPT`, `get_shell_integration_path`, the `script_path` variable and the `Path`/`PathBuf` imports that become unused; call `ConPtySession::spawn(shell_type, cols, rows, SpawnOptions { no_profile: self.no_profile })?`.

- [ ] **Step 8: Extend the script** — insert into `assets/shellIntegration.ps1`, after the `__IS-Escape-Value` function (use an editor, the block contains backslashes):

```powershell
function Global:__SP-Escape([string]$value) {
    [regex]::Replace($value, '[\x00-\x1f\x7f\\;]', { param($match)
        -join ([System.Text.Encoding]::UTF8.GetBytes($match.Value) | ForEach-Object { '\x{0:x2}' -f $_ })
    })
}

function Global:__SP-Send([string]$payload) {
    [Console]::Write("$([char]0x1b)]6973;$payload$([char]0x07)")
}

# Mark the time PSReadLine spends reading a line. Unlike a prompt wrapper this survives the user
# redefining `prompt` and does not change what the prompt function sees.
if (Get-Command PSConsoleHostReadLine -ErrorAction Ignore) {
    $Global:__SP_OriginalReadLine = $function:PSConsoleHostReadLine
    function Global:PSConsoleHostReadLine {
        $cwd = if ($pwd.Provider.Name -eq 'FileSystem') { $pwd.ProviderPath } else { '' }
        __SP-Send "RS;$(__SP-Escape $cwd)"
        try { $Global:__SP_OriginalReadLine.Invoke() } finally { __SP-Send 'RE' }
    }
}

# Ctrl+Alt+Shift+F12 (sent by shell-panel when Tab is pressed): report line, cursor and completions.
try {
    Set-PSReadLineKeyHandler -Chord 'Ctrl+Alt+Shift+F12' -BriefDescription 'ShellPanelReport' -ScriptBlock {
        $line = $null
        $cursor = $null
        [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
        $report = @{ line = $line; cursor = $cursor; replacementIndex = $cursor; replacementLength = 0; matches = @() }
        try {
            $completion = [System.Management.Automation.CommandCompletion]::CompleteInput($line, $cursor, $null)
            $report.replacementIndex = $completion.ReplacementIndex
            $report.replacementLength = $completion.ReplacementLength
            $report.matches = @($completion.CompletionMatches | Select-Object -First 100 | ForEach-Object {
                $tip = "$($_.ToolTip)"
                if ($tip.Length -gt 120) {
                    $cut = if ([char]::IsHighSurrogate($tip[119])) { 119 } else { 120 }
                    $tip = $tip.Substring(0, $cut)
                }
                , @($_.CompletionText, $_.ListItemText, $_.ResultType.ToString(), $tip)
            })
        } catch {}
        __SP-Send "CMP;$(__SP-Escape (ConvertTo-Json -InputObject $report -Compress -Depth 4))"
    }
} catch {}
```

- [ ] **Step 9: Update callers and tests**

- `tests/integration_script_test.rs`: delete the file.
- `tests/pty_test.rs`: both `ConPtySession::spawn` calls lose the script argument; drop the `get_shell_integration_path` and `std::path::Path` imports.
- `tests/e2e_pty_test.rs`: replace `test_embedded_script_and_path_resolution` with

  ```rust
  #[test]
  fn test_embedded_script_defines_the_protocol() {
      use shell_panel::shell::integration::SCRIPT;
      for marker in ["6973;", "RS;", "'RE'", "CMP;", "Ctrl+Alt+Shift+F12"] {
          assert!(SCRIPT.contains(marker), "script lacks {marker}");
      }
  }
  ```

  fix the imports, and update the spawn call in `test_e2e_pty_powershell_session`.

- [ ] **Step 10:** `cargo test` → all pass (the e2e tests prove the app still works with the script delivered by `-EncodedCommand`).

- [ ] **Step 11: Commit**

```bash
git add -A src assets tests
git commit -m "feat(shell): pass the integration script with -EncodedCommand and report ReadLine state, line and completions"
```

---

### Task 8: Completion engine — shell matches, concurrent providers, merge rules, exact replacement (H6, H8, N8, N9, T2)

Pure logic, no app changes yet.

**Files:**
- Create: `src/engine/aggregate.rs`, `tests/aggregate_test.rs`
- Modify: `src/engine/mod.rs`, `src/engine/provider.rs`, `src/engine/replacement.rs`, `src/engine/lexer.rs`, `tests/engine_test.rs`, `tests/lexer_test.rs`, `tests/tab_trigger_test.rs`

**Interfaces:**
- Consumes: `ShellReport`, `ShellMatch` (Task 7), `carapace_args` (Task 5).
- Produces:
  - `Suggestion.uses_shell_range: bool` + `Suggestion::with_shell_range(self) -> Self`.
  - `ReplacementAction { backspace_count: usize, delete_count: usize, insert_text: String }`, `ReplacementAction::to_bytes(&self) -> Vec<u8>`, `replace_range(before: &str, after: &str, suggestion: &str) -> ReplacementAction`.
  - `active_token_raw(input: &str) -> &str` in `engine::lexer`; `quote_for_powershell(path: &str) -> String` in `engine::providers::zoxide`.
  - In `engine::aggregate`: `should_include_files` (moved from `core::app`), `shell_suggestions(&ShellReport) -> Vec<Suggestion>`, `merge_suggestions(active_token: &str, external: Vec<Suggestion>, shell: Vec<Suggestion>) -> Vec<Suggestion>`, `plan_replacement(&ShellReport, &Suggestion) -> ReplacementAction`, `#[derive(Clone)] CompletionEngine` with `new(json_spec: JsonSpecProvider) -> Self` and `async complete(&self, report: &ShellReport, cwd: &str) -> Vec<Suggestion>`.

- [ ] **Step 1: Write failing tests**

`tests/lexer_test.rs` (extend the import with `active_token_raw`):

```rust
#[test]
fn test_active_token_raw_keeps_quotes_and_escapes() {
    assert_eq!(active_token_raw("git sta"), "sta");
    assert_eq!(active_token_raw("cd 'My Do"), "'My Do");
    assert_eq!(active_token_raw("git commit -m \"hello wor"), "\"hello wor");
    assert_eq!(active_token_raw("ls "), "");
    assert_eq!(active_token_raw("a | b"), "b");
    assert_eq!(active_token_raw("cmd --name=va"), "va");
    assert_eq!(active_token_raw("echo a` b"), "a` b");
}
```

`tests/engine_test.rs`: add `delete_count: 0,` to every existing `ReplacementAction { .. }` literal (10 places), extend the import with `replace_range`, and append:

```rust
#[test]
fn test_replacement_bytes_and_forward_delete() {
    // Cursor inside `Get-Child|Item`, suggestion `Get-ChildItem`.
    let action = replace_range("Get-Child", "Item", "Get-ChildItem");
    assert_eq!(
        action,
        ReplacementAction { backspace_count: 9, delete_count: 4, insert_text: "Get-ChildItem ".into() }
    );
    let mut expected = vec![0x7f; 9];
    for _ in 0..4 {
        expected.extend_from_slice(b"\x1b[3~");
    }
    expected.extend_from_slice(b"Get-ChildItem ");
    assert_eq!(action.to_bytes(), expected);

    // Nothing after the cursor: same as the prefix logic (only the suffix is typed).
    assert_eq!(
        replace_range("System.IO.Fi", "", "System.IO.File"),
        ReplacementAction { backspace_count: 0, delete_count: 0, insert_text: "le ".into() }
    );
}

#[test]
fn test_no_trailing_space_after_quoted_directory() {
    assert_eq!(calculate_replacement("'My", r"'.\My Documents\'").insert_text, r"'.\My Documents\'");
    assert_eq!(calculate_replacement("a", "'a b.txt'").insert_text, "'a b.txt' ");
}
```

Create `tests/aggregate_test.rs`:

```rust
use shell_panel::engine::aggregate::{merge_suggestions, plan_replacement, shell_suggestions, CompletionEngine};
use shell_panel::engine::provider::{Suggestion, SuggestionKind};
use shell_panel::engine::providers::json_spec::{FigSpec, FigSubcommand, JsonSpecProvider};
use shell_panel::engine::replacement::ReplacementAction;
use shell_panel::shell::report::{ShellMatch, ShellReport};

fn report(line: &str, cursor: usize, index: i64, length: i64, matches: Vec<ShellMatch>) -> ShellReport {
    ShellReport { line: line.into(), cursor, replacement_index: index, replacement_length: length, matches }
}

fn m(text: &str, list: &str, kind: &str, tip: &str) -> ShellMatch {
    ShellMatch(text.into(), list.into(), kind.into(), tip.into())
}

#[test]
fn test_shell_suggestions_kinds_and_descriptions() {
    let sugs = shell_suggestions(&report("x", 1, 0, 1, vec![
        m("Get-ChildItem", "Get-ChildItem", "Command", "\r\nGet-ChildItem [[-Path] <string[]>]\r\n"),
        m("git", "git", "Command", "git"),
        m("ls", "ls", "Command", "Get-ChildItem"),
        m("-Path", "Path", "ParameterName", "[string[]] Path"),
        m(r".\src\", "src", "ProviderContainer", r"C:\p\src"),
        m(r".\a.txt", "a.txt", "ProviderItem", r"C:\p\a.txt"),
        m("$x", "x", "Variable", "x"),
    ]));
    let kinds: Vec<SuggestionKind> = sugs.iter().map(|s| s.kind).collect();
    assert_eq!(kinds, vec![
        SuggestionKind::PowerShellCmdlet,
        SuggestionKind::Command, // a native command is not an alias just because it is short
        SuggestionKind::Alias,
        SuggestionKind::Option,
        SuggestionKind::Directory,
        SuggestionKind::File,
        SuggestionKind::Other,
    ]);
    assert!(sugs.iter().all(|s| s.uses_shell_range));
    assert_eq!(sugs[0].description.as_deref(), Some("Get-ChildItem [[-Path] <string[]>]"));
    assert_eq!(sugs[1].description, None); // tooltip equal to the text adds nothing
    assert_eq!(sugs[4].display, "src");
}

#[test]
fn test_merge_prefers_specs_hides_files_and_dedupes_path_styles() {
    let external = vec![
        Suggestion::new("status", "status", Some("spec".into()), 80).with_kind(SuggestionKind::Subcommand),
        Suggestion::new("stash", "stash", Some("carapace".into()), 70).with_kind(SuggestionKind::Subcommand),
        Suggestion::new("status", "status", Some("carapace".into()), 70).with_kind(SuggestionKind::Subcommand),
    ];
    let shell = vec![
        Suggestion::new(r".\stats.txt", "stats.txt", None, 50).with_kind(SuggestionKind::File).with_shell_range(),
    ];
    let merged = merge_suggestions("sta", external, shell);
    let names: Vec<&str> = merged.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["status", "stash"]); // one `status` (the spec's), no file
    assert_eq!(merged[0].description.as_deref(), Some("spec"));

    // A path-like token brings files back, and `src/` equals `.\src\` for de-duplication.
    let external = vec![
        Suggestion::new("src/", "src/", None, 70).with_kind(SuggestionKind::Subcommand),
    ];
    let shell = vec![
        Suggestion::new(r".\src\", "src", None, 60).with_kind(SuggestionKind::Directory).with_shell_range(),
        Suggestion::new(r".\srv.txt", "srv.txt", None, 50).with_kind(SuggestionKind::File).with_shell_range(),
    ];
    let names: Vec<String> = merge_suggestions("./sr", external, shell).into_iter().map(|s| s.name).collect();
    assert_eq!(names, vec!["src/".to_string(), r".\srv.txt".to_string()]);
}

#[test]
fn test_plan_replacement_uses_shell_range_or_raw_token() {
    // PowerShell's range keeps the `[` in front of the type name.
    let r = report("[System.IO.Fi", 13, 1, 12, vec![]);
    let shell = Suggestion::new("System.IO.File", "File", None, 70).with_shell_range();
    assert_eq!(
        plan_replacement(&r, &shell),
        ReplacementAction { backspace_count: 0, delete_count: 0, insert_text: "le ".into() }
    );

    // Cursor in the middle of the token: the rest of the token is deleted forwards.
    let r = report("Get-ChildItem", 9, 0, 13, vec![]);
    let shell = Suggestion::new("Get-ChildItem", "Get-ChildItem", None, 80).with_shell_range();
    assert_eq!(
        plan_replacement(&r, &shell),
        ReplacementAction { backspace_count: 9, delete_count: 4, insert_text: "Get-ChildItem ".into() }
    );

    // Spec/carapace/zoxide suggestions replace the raw token before the cursor, quotes included.
    let r = report("cd 'My Do", 9, 3, 6, vec![]);
    let zoxide = Suggestion::new(r"'C:\My Documents'", r"C:\My Documents", None, 70);
    assert_eq!(
        plan_replacement(&r, &zoxide),
        ReplacementAction { backspace_count: 6, delete_count: 0, insert_text: r"'C:\My Documents' ".into() }
    );
}

#[tokio::test]
async fn test_engine_merges_spec_with_shell_matches_off_thread() {
    let mut specs = JsonSpecProvider::new();
    specs.add_spec(FigSpec {
        name: "mytool".into(),
        subcommands: vec![FigSubcommand { name: "deploy".into(), ..Default::default() }],
        ..Default::default()
    });
    let engine = CompletionEngine::new(specs);
    let r = report("mytool de", 9, 7, 2, vec![m(r".\demo.txt", "demo.txt", "ProviderItem", "")]);

    // Must be usable from a spawned task (Send + 'static).
    let sugs = tokio::spawn(async move { engine.complete(&r, "").await }).await.unwrap();
    let names: Vec<&str> = sugs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["deploy"]);
}
```

Also in `tests/engine_test.rs` (extend the zoxide import with `quote_for_powershell`):

```rust
#[test]
fn test_zoxide_paths_are_quoted_for_powershell() {
    assert_eq!(quote_for_powershell(r"C:\src"), r"C:\src");
    assert_eq!(quote_for_powershell(r"C:\My Documents"), r"'C:\My Documents'");
    assert_eq!(quote_for_powershell(r"C:\it's"), r"'C:\it''s'");

    let sugs = parse_zoxide_output("C:\\My Documents\n", "my");
    assert_eq!(sugs[0].name, r"'C:\My Documents'");
    assert_eq!(sugs[0].display, r"C:\My Documents");
}
```

`tests/tab_trigger_test.rs`: change the import of `should_include_files` to `shell_panel::engine::aggregate::should_include_files`, and delete the two tests that re-implement application code inside the test, `test_priority_sorting_and_deduplication` (now covered by `merge_suggestions` tests) and `test_replacement_del_0x7f_encoding` (now covered by `to_bytes`), plus the imports that become unused.

- [ ] **Step 2:** `cargo test --test aggregate_test --test engine_test --test lexer_test` → compile errors.

- [ ] **Step 3: `Suggestion`** — in `src/engine/provider.rs` add the field (and `uses_shell_range: false,` in `new`):

```rust
    /// True when the suggestion comes from PowerShell's own completion: it then replaces the
    /// range PowerShell reported instead of the token found by our lexer.
    #[serde(default)]
    pub uses_shell_range: bool,
```

```rust
    /// Marks the suggestion as replacing PowerShell's reported range.
    pub fn with_shell_range(mut self) -> Self {
        self.uses_shell_range = true;
        self
    }
```

- [ ] **Step 4: Replacement** — in `src/engine/replacement.rs` replace the struct and `calculate_replacement`:

```rust
/// Keys that turn the text around the cursor into the chosen suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementAction {
    /// Backspaces to erase text before the cursor.
    pub backspace_count: usize,
    /// Forward deletes to erase text after the cursor.
    pub delete_count: usize,
    /// Text to type, including any trailing space.
    pub insert_text: String,
}

impl ReplacementAction {
    /// The byte sequence to write to the PTY.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0x7f; self.backspace_count];
        for _ in 0..self.delete_count {
            bytes.extend_from_slice(b"\x1b[3~");
        }
        bytes.extend_from_slice(self.insert_text.as_bytes());
        bytes
    }
}

/// Directories (also when quoted) keep the cursor right behind them so the path can continue.
fn trailing_space(suggestion: &str) -> &'static str {
    let unquoted = suggestion.trim_end_matches(['\'', '"']);
    if unquoted.ends_with('/') || unquoted.ends_with('\\') {
        ""
    } else {
        " "
    }
}

/// Replaces `typed` (the text right before the cursor) with `suggestion`.
/// When `suggestion` extends `typed` only the missing suffix is typed.
pub fn calculate_replacement(typed: &str, suggestion: &str) -> ReplacementAction {
    let space = trailing_space(suggestion);
    if suggestion.starts_with(typed) {
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: format!("{}{}", &suggestion[typed.len()..], space),
        }
    } else {
        ReplacementAction {
            backspace_count: typed.chars().count(),
            delete_count: 0,
            insert_text: format!("{}{}", suggestion, space),
        }
    }
}

/// Replaces `before` + `after` (the text on both sides of the cursor) with `suggestion`.
pub fn replace_range(before: &str, after: &str, suggestion: &str) -> ReplacementAction {
    if after.is_empty() {
        return calculate_replacement(before, suggestion);
    }
    ReplacementAction {
        backspace_count: before.chars().count(),
        delete_count: after.chars().count(),
        insert_text: format!("{}{}", suggestion, trailing_space(suggestion)),
    }
}
```

In `src/core/app.rs` the two places that build `write_buf` by hand become `let _ = pty_writer.write_all(&replacement.to_bytes());`.

- [ ] **Step 5: `active_token_raw`** — append to `src/engine/lexer.rs`:

```rust
/// Returns the raw source text (quotes and backtick escapes included) of the token that ends
/// `input`, i.e. exactly what has to be erased to replace it at the prompt.
pub fn active_token_raw(input: &str) -> &str {
    let mut start = 0;
    let mut state = DelimQuoteState::Normal;
    let mut chars = input.char_indices();

    while let Some((i, c)) = chars.next() {
        let next = i + c.len_utf8();
        match state {
            DelimQuoteState::Normal => match c {
                ' ' | '\t' | '|' | ';' | '&' => start = next,
                '\'' => state = DelimQuoteState::SingleQuote,
                '"' => state = DelimQuoteState::DoubleQuote,
                '`' => {
                    chars.next();
                }
                '=' if input[start..i].starts_with('-') => start = next,
                _ => {}
            },
            DelimQuoteState::SingleQuote => {
                if c == '\'' {
                    state = DelimQuoteState::Normal;
                }
            }
            DelimQuoteState::DoubleQuote => match c {
                '`' => {
                    chars.next();
                }
                '"' => state = DelimQuoteState::Normal,
                _ => {}
            },
        }
    }

    &input[start..]
}
```

- [ ] **Step 5b: Quote zoxide paths** — in `src/engine/providers/zoxide.rs` add

```rust
/// Wraps `path` in single quotes when PowerShell would otherwise split or interpret it.
pub fn quote_for_powershell(path: &str) -> String {
    let needs_quotes = path
        .chars()
        .any(|c| c.is_whitespace() || "'\"`$(){};,&@#|<>".contains(c));
    if needs_quotes {
        format!("'{}'", path.replace('\'', "''"))
    } else {
        path.to_string()
    }
}
```

and in `parse_zoxide_output` build the suggestion with `Suggestion::new(quote_for_powershell(trimmed), trimmed, Some("Zoxide Directory".into()), 70)`.

- [ ] **Step 6: Create `src/engine/aggregate.rs`**

```rust
use std::collections::HashSet;

use crate::engine::lexer::{active_token_raw, lex_command_line};
use crate::engine::provider::{CompletionProvider, Suggestion, SuggestionKind};
use crate::engine::providers::carapace::CarapaceProvider;
use crate::engine::providers::json_spec::JsonSpecProvider;
use crate::engine::providers::zoxide::ZoxideProvider;
use crate::engine::replacement::{calculate_replacement, replace_range, ReplacementAction};
use crate::shell::report::ShellReport;

/// Determines whether file suggestions should be shown next to `existing` suggestions.
///
/// When subcommands, commands, cmdlets or options were found, files are only relevant if the
/// active token looks like a path (contains `/` or `\`, or starts with `.`).
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

/// Aliases PowerShell defines out of the box (completion reports them as plain commands).
const BUILTIN_ALIASES: &[&str] = &[
    "cat", "cd", "chdir", "clc", "clear", "clhy", "cli", "clp", "cls", "clv", "cnsn", "compare",
    "copy", "cp", "cpi", "cpp", "cvpa", "dbp", "del", "diff", "dir", "dnsn", "ebp", "echo", "epal",
    "epcsv", "erase", "etsn", "exsn", "fc", "fhx", "fl", "foreach", "ft", "fw", "gal", "gbp", "gc",
    "gcb", "gci", "gcm", "gcs", "gdr", "gerr", "ghy", "gi", "gin", "gjb", "gl", "gm", "gmo", "gp",
    "gps", "gpv", "group", "gsn", "gsv", "gtz", "gu", "gv", "h", "history", "icm", "iex", "ihy",
    "ii", "ipal", "ipcsv", "ipmo", "irm", "iwr", "kill", "ls", "man", "md", "measure", "mi",
    "mount", "move", "mp", "mv", "nal", "ndr", "ni", "nmo", "nsn", "nv", "ogv", "oh", "popd", "ps",
    "pushd", "pwd", "r", "rbp", "rcjb", "rcsn", "rd", "rdr", "ren", "ri", "rjb", "rm", "rmdir",
    "rmo", "rni", "rnp", "rp", "rsn", "rv", "rvpa", "sajb", "sal", "saps", "sasv", "sbp", "scb",
    "select", "set", "shcm", "si", "sl", "sleep", "sls", "sort", "sp", "spjb", "spps", "spsv",
    "start", "stz", "sv", "tee", "type", "where", "wjb", "write",
];

/// Turns PowerShell's completion matches into suggestions.
pub fn shell_suggestions(report: &ShellReport) -> Vec<Suggestion> {
    report
        .matches
        .iter()
        .map(|m| {
            let (text, list_item, result_type, tooltip) = (&m.0, &m.1, &m.2, &m.3);
            let (kind, priority) = match result_type.as_str() {
                "Command" if text.contains('-') => (SuggestionKind::PowerShellCmdlet, 80),
                "Command" if BUILTIN_ALIASES.contains(&text.to_lowercase().as_str()) => (SuggestionKind::Alias, 80),
                "Command" => (SuggestionKind::Command, 80),
                "ParameterName" => (SuggestionKind::Option, 75),
                "ProviderContainer" => (SuggestionKind::Directory, 60),
                "ProviderItem" => (SuggestionKind::File, 50),
                _ => (SuggestionKind::Other, 70),
            };
            let description = tooltip
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .filter(|l| l != text && l != list_item)
                .map(str::to_string);
            let display = if list_item.is_empty() { text.clone() } else { list_item.clone() };
            Suggestion::new(text.clone(), display, description, priority)
                .with_kind(kind)
                .with_shell_range()
        })
        .collect()
}

/// `src/`, `.\src\` and `'.\src\'` are the same entry for de-duplication.
fn dedupe_key(name: &str) -> String {
    let unquoted = name.trim_matches(['\'', '"']);
    let normalized = unquoted.replace('\\', "/").to_lowercase();
    normalized.strip_prefix("./").unwrap_or(&normalized).to_string()
}

/// Merges suggestions of the external sources (specs, carapace, zoxide) with PowerShell's:
/// highest priority first, one entry per name, and PowerShell's files only where files make sense.
pub fn merge_suggestions(
    active_token: &str,
    external: Vec<Suggestion>,
    shell: Vec<Suggestion>,
) -> Vec<Suggestion> {
    let include_files = should_include_files(active_token, external.iter());
    let mut results = external;
    results.extend(shell.into_iter().filter(|s| {
        include_files || !matches!(s.kind, SuggestionKind::File | SuggestionKind::Directory)
    }));

    // Stable sort: on equal priority and name the external source (added first) wins.
    results.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.name.cmp(&b.name)));
    let mut seen = HashSet::new();
    results.retain(|s| seen.insert(dedupe_key(&s.name)));
    results
}

/// Keys that replace the text around the cursor with `suggestion`.
pub fn plan_replacement(report: &ShellReport, suggestion: &Suggestion) -> ReplacementAction {
    let cursor = report.cursor_byte().unwrap_or(report.line.len());
    match report.replacement_range() {
        Some((start, end)) if suggestion.uses_shell_range => {
            replace_range(&report.line[start..cursor], &report.line[cursor..end], &suggestion.name)
        }
        _ => calculate_replacement(active_token_raw(&report.line[..cursor]), &suggestion.name),
    }
}

/// Queries the external providers and merges their results with the shell's own completions.
#[derive(Clone)]
pub struct CompletionEngine {
    json_spec: JsonSpecProvider,
    zoxide: ZoxideProvider,
    carapace: CarapaceProvider,
}

impl CompletionEngine {
    pub fn new(json_spec: JsonSpecProvider) -> Self {
        Self {
            json_spec,
            zoxide: ZoxideProvider::default(),
            carapace: CarapaceProvider::default(),
        }
    }

    /// Suggestions for the text before the cursor of `report`.
    pub async fn complete(&self, report: &ShellReport, cwd: &str) -> Vec<Suggestion> {
        let text = report.text_before_cursor().unwrap_or(&report.line);
        let tokens = lex_command_line(text);
        let root_cmd = tokens.first().map(|t| t.text.as_str()).unwrap_or("");
        let active_token = tokens.last().map(|t| t.text.as_str()).unwrap_or("");

        let (spec, zoxide, carapace) = tokio::join!(
            run_provider(&self.json_spec, root_cmd, text, cwd),
            run_provider(&self.zoxide, root_cmd, text, cwd),
            run_provider(&self.carapace, root_cmd, text, cwd),
        );

        let external = spec.into_iter().chain(zoxide).chain(carapace).collect();
        merge_suggestions(active_token, external, shell_suggestions(report))
    }
}

async fn run_provider(
    provider: &dyn CompletionProvider,
    root_cmd: &str,
    text: &str,
    cwd: &str,
) -> Vec<Suggestion> {
    if provider.can_handle(root_cmd) {
        provider.complete(text, cwd).await
    } else {
        Vec::new()
    }
}
```

Add `pub mod aggregate;` to `src/engine/mod.rs`. In `src/core/app.rs` delete the local `should_include_files` and import it from `crate::engine::aggregate` instead.

- [ ] **Step 7:** `cargo test` → all pass.

- [ ] **Step 8: Commit**

```bash
git add -A src tests
git commit -m "feat(engine): merge PowerShell's session completions with specs, carapace and zoxide; exact replacement ranges"
```

---

### Task 9: Rewrite the reactor loop around the report; delete the worker, the file provider, the prompt wrapper and screen scraping (C3, C5, H9, M2, N2, N4, N11, T1)

**Files:**
- Create: `src/shell/stream.rs`, `tests/stream_test.rs`
- Rewrite: `src/core/app.rs` (`App::run` and helpers), `src/shell/command_state.rs`, `assets/shellIntegration.ps1`
- Modify: `src/shell/mod.rs`, `src/shell/osc.rs`, `src/engine/providers/mod.rs`, `src/vt/emulator.rs`, `src/vt/mod.rs`
- Delete: `src/engine/providers/powershell.rs`, `src/engine/providers/files.rs`, `assets/psWorker.ps1`, `tests/powershell_provider_test.rs`
- Test: `tests/e2e_binary_test.rs`, `tests/osc_test.rs`, `tests/vt_test.rs`, `tests/engine_test.rs`, `tests/tab_trigger_test.rs`

**Interfaces:**
- Consumes: Tasks 3, 4, 6, 7, 8.
- Produces:
  - `OscEvent` reduced to `ReadLineStarted { cwd }`, `ReadLineEnded`, `Report(ShellReport)`.
  - `CommandState { cwd: String, reading_line: bool, report: Option<ShellReport> }`, `handle_osc(&mut self, event: OscEvent)`.
  - `shell::stream::ingest_pty_chunk(chunk: &[u8], term: &mut HeadlessTerminal, state: &mut CommandState, residual: &mut Vec<u8>) -> Vec<u8>`.

- [ ] **Step 1: Extend the end-to-end tests (failing first)** — append to `tests/e2e_binary_test.rs`:

```rust
#[test]
fn test_completion_uses_the_real_line_and_the_real_session() {
    let dir = temp_dir("session");
    std::fs::write(dir.join("zz_unique_file.txt"), "x").unwrap();
    std::fs::create_dir_all(dir.join("Zq Folder")).unwrap();
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START));

    // `>` inside the command used to cut the scraped line.
    term.send(b"echo a > zz_uni");
    assert!(term.wait_for_text("zz_uni", STEP));
    term.send(b"\t");
    assert!(term.wait_for_text("zz_unique_file.txt", STEP), "screen: {}", term.screen());
    term.send(b"\x1b");

    // A variable that exists only in this session.
    term.send(b"$sp_e2e_var_zz = 1\r");
    term.send(b"$sp_e2e_v");
    assert!(term.wait_for_text("$sp_e2e_v", STEP));
    term.send(b"\t");
    assert!(term.wait_for_text("$sp_e2e_var_zz", STEP), "screen: {}", term.screen());
    term.send(b"\x1b");

    // Names with spaces arrive quoted from PowerShell.
    // (An unusual name, so that zoxide history cannot add a second match.)
    term.send(b"cd Zq");
    assert!(term.wait_for_text("cd Zq", STEP));
    term.send(b"\t");
    assert!(term.wait_for_text(r"'.\Zq Folder\'", STEP), "screen: {}", term.screen());
    term.send(b"\x1b");

    // Completion in the middle of the line replaces the whole token.
    term.send(b"Get-ChildIt_XX");
    assert!(term.wait_for_text("Get-ChildIt_XX", STEP));
    term.send(b"\x1b[D\x1b[D\x1b[D"); // cursor before `_XX`
    term.send(b"\t");
    assert!(
        term.wait_until(STEP, |t| t.screen().contains("Get-ChildItem") && !t.screen().contains("_XX")),
        "screen: {}",
        term.screen()
    );

    term.send(b"\x1b");
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_tab_outside_psreadline_is_a_plain_tab() {
    let dir = temp_dir("readhost");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START));

    term.send(b"$v = Read-Host 'name'\r");
    assert!(term.wait_for_text("name:", STEP));
    term.send(b"a\tb\r");
    // Read-Host may keep or drop the Tab, but the reserved chord (`[24;8~`) must never reach it.
    // (The answer is built by concatenation so the echoed command line cannot match.)
    term.send(b"if ($v -match '^a\\s*b$') { 'TAB-' + 'CLEAN' } else { 'TAB-' + 'DIRTY' }\r");
    assert!(
        term.wait_until(STEP, |t| t.screen().contains("TAB-CLEAN") || t.screen().contains("TAB-DIRTY")),
        "screen: {}",
        term.screen()
    );
    assert!(term.screen().contains("TAB-CLEAN"), "screen: {}", term.screen());

    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}
```

Run `cargo test --test e2e_binary_test` → `test_completion_uses_the_real_line_and_the_real_session` fails (scraped line is wrong for `>`, no session variables, no mid-line support).

- [ ] **Step 2: Reduce the protocol and the state**

`src/shell/osc.rs`: remove the variants `PromptStarted`, `PromptEnded`, `Cwd` and their parsing (`PS`, `PE`, `CWD;`); update the doc comment to list `RS;`, `RE`, `CMP;`.

`src/shell/command_state.rs` becomes:

```rust
use crate::shell::osc::OscEvent;
use crate::shell::report::ShellReport;

/// What shell-panel knows about the shell from its integration messages.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandState {
    /// Current filesystem location of the shell.
    pub cwd: String,
    /// True while PSReadLine is reading a line: the only time the report request is answered.
    pub reading_line: bool,
    /// Latest completion report, taken by the reactor loop.
    pub report: Option<ShellReport>,
}

impl CommandState {
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
```

`tests/osc_test.rs`: delete the tests that use `PromptStarted`/`PromptEnded`/`Cwd` (`test_parse_osc_sequences`, `test_parse_osc_with_escapes`, `test_command_state_handle_osc`) and add:

```rust
#[test]
fn test_unknown_messages_are_ignored() {
    assert_eq!(parse_osc_sequence("1337;Other"), None);
    assert_eq!(parse_osc_sequence("6973;UNKNOWN"), None);
    assert_eq!(parse_osc_sequence("6973;PS"), None); // protocol v1
    assert_eq!(parse_osc_sequence(""), None);
}

#[test]
fn test_command_state_follows_readline_markers() {
    let mut state = CommandState::default();
    assert!(!state.reading_line);

    state.handle_osc(OscEvent::ReadLineStarted { cwd: Some("C:\\Project".into()) });
    assert!(state.reading_line);
    assert_eq!(state.cwd, "C:\\Project");

    // A non-filesystem location (e.g. HKLM:) keeps the last filesystem cwd.
    state.handle_osc(OscEvent::ReadLineStarted { cwd: None });
    assert_eq!(state.cwd, "C:\\Project");

    let report = ShellReport { line: "git".into(), cursor: 3, replacement_index: 0, replacement_length: 3, matches: vec![] };
    state.handle_osc(OscEvent::Report(report.clone()));
    assert_eq!(state.report, Some(report));

    state.handle_osc(OscEvent::ReadLineEnded);
    assert!(!state.reading_line);
    state.handle_osc(OscEvent::ReadLineStarted { cwd: None });
    assert_eq!(state.report, None);
}
```

- [ ] **Step 3: Stream ingestion** — create `tests/stream_test.rs`:

```rust
use shell_panel::shell::command_state::CommandState;
use shell_panel::shell::stream::ingest_pty_chunk;
use shell_panel::vt::emulator::HeadlessTerminal;

#[test]
fn test_message_split_across_chunks_is_reassembled_and_hidden() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::default();
    let mut residual = Vec::new();

    let first = ingest_pty_chunk(b"hi\x1b]6973;RS;C:/p", &mut term, &mut state, &mut residual);
    let second = ingest_pty_chunk(b"roj\x07there", &mut term, &mut state, &mut residual);

    assert_eq!(first, b"hi");
    assert_eq!(second, b"there");
    assert_eq!(state.cwd, "C:/proj");
    assert!(state.reading_line);
    assert!(residual.is_empty());
    assert_eq!(term.cursor_position(), (7, 0)); // "hithere"
}

#[test]
fn test_split_prefix_and_st_terminator() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::default();
    let mut residual = Vec::new();

    let first = ingest_pty_chunk(b"a\x1b]69", &mut term, &mut state, &mut residual);
    let second = ingest_pty_chunk(b"73;RE\x1b\\b", &mut term, &mut state, &mut residual);

    assert_eq!(first, b"a");
    assert_eq!(second, b"b");
    assert!(!state.reading_line);
}
```

Create `src/shell/stream.rs` — move `scan_and_handle_osc` from `app.rs` into it, without the cursor lookup:

```rust
use crate::io::filter::sanitize_output_stream;
use crate::shell::command_state::CommandState;
use crate::shell::osc::parse_osc_sequence;
use crate::vt::emulator::HeadlessTerminal;

const OSC_PREFIX: &[u8] = b"\x1b]6973;";

/// Processes one chunk of PTY output: strips input-protocol negotiation, applies and removes
/// shell-panel's OSC 6973 messages, feeds the rest to the headless terminal and returns it for
/// echoing to the host terminal. A message split across chunks waits in `residual`.
pub fn ingest_pty_chunk(
    chunk: &[u8],
    term: &mut HeadlessTerminal,
    command_state: &mut CommandState,
    residual: &mut Vec<u8>,
) -> Vec<u8> {
    let mut data = std::mem::take(residual);
    data.extend_from_slice(chunk);
    let sanitized = sanitize_output_stream(&data);
    scan_messages(&sanitized, term, command_state, residual)
}

fn scan_messages(
    data: &[u8],
    term: &mut HeadlessTerminal,
    command_state: &mut CommandState,
    residual: &mut Vec<u8>,
) -> Vec<u8> {
    let mut clean_output = Vec::with_capacity(data.len());
    let mut i = 0;
    let mut last = 0;

    while i < data.len() {
        if !data[i..].starts_with(OSC_PREFIX) {
            i += 1;
            continue;
        }

        if i > last {
            term.process(&data[last..i]);
            clean_output.extend_from_slice(&data[last..i]);
        }

        let body = &data[i + OSC_PREFIX.len()..];
        let terminator = body.iter().enumerate().find_map(|(offset, &b)| {
            if b == 0x07 {
                Some((offset, 1))
            } else if body[offset..].starts_with(b"\x1b\\") {
                Some((offset, 2))
            } else {
                None
            }
        });

        let Some((offset, terminator_len)) = terminator else {
            // Unterminated message at the chunk boundary: wait for the next chunk.
            residual.extend_from_slice(&data[i..]);
            return clean_output;
        };

        let payload_end = i + OSC_PREFIX.len() + offset;
        if let Ok(payload) = std::str::from_utf8(&data[i + 2..payload_end]) {
            if let Some(event) = parse_osc_sequence(payload) {
                command_state.handle_osc(event);
            }
        }
        i = payload_end + terminator_len;
        last = i;
    }

    // Keep back a tail that could be the start of a message prefix.
    let tail = &data[last..];
    let held = (1..=OSC_PREFIX.len().min(tail.len()))
        .rev()
        .find(|&len| tail.ends_with(&OSC_PREFIX[..len]))
        .unwrap_or(0);
    let safe = &tail[..tail.len() - held];
    term.process(safe);
    clean_output.extend_from_slice(safe);
    residual.extend_from_slice(&tail[tail.len() - held..]);

    clean_output
}
```

Add `pub mod stream;` to `src/shell/mod.rs`.

- [ ] **Step 4: Final integration script** — replace `assets/shellIntegration.ps1` entirely (editor, not heredoc) with:

```powershell
# shell-panel integration for PowerShell with PSReadLine 2.x. Passed with -EncodedCommand.
# Messages to shell-panel: ESC ] 6973;<payload> BEL with payloads RS;<cwd>, RE and CMP;<json>.

function Global:__SP-Escape([string]$value) {
    [regex]::Replace($value, '[\x00-\x1f\x7f\\;]', { param($match)
        -join ([System.Text.Encoding]::UTF8.GetBytes($match.Value) | ForEach-Object { '\x{0:x2}' -f $_ })
    })
}

function Global:__SP-Send([string]$payload) {
    [Console]::Write("$([char]0x1b)]6973;$payload$([char]0x07)")
}

# Mark the time PSReadLine spends reading a line. Unlike a prompt wrapper this survives the user
# redefining `prompt` and does not change what the prompt function sees.
if (Get-Command PSConsoleHostReadLine -ErrorAction Ignore) {
    $Global:__SP_OriginalReadLine = $function:PSConsoleHostReadLine
    function Global:PSConsoleHostReadLine {
        $cwd = if ($pwd.Provider.Name -eq 'FileSystem') { $pwd.ProviderPath } else { '' }
        __SP-Send "RS;$(__SP-Escape $cwd)"
        try { $Global:__SP_OriginalReadLine.Invoke() } finally { __SP-Send 'RE' }
    }
}

# Ctrl+Alt+Shift+F12 (sent by shell-panel when Tab is pressed): report line, cursor and completions.
try {
    Set-PSReadLineKeyHandler -Chord 'Ctrl+Alt+Shift+F12' -BriefDescription 'ShellPanelReport' -ScriptBlock {
        $line = $null
        $cursor = $null
        [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
        $report = @{ line = $line; cursor = $cursor; replacementIndex = $cursor; replacementLength = 0; matches = @() }
        try {
            $completion = [System.Management.Automation.CommandCompletion]::CompleteInput($line, $cursor, $null)
            $report.replacementIndex = $completion.ReplacementIndex
            $report.replacementLength = $completion.ReplacementLength
            $report.matches = @($completion.CompletionMatches | Select-Object -First 100 | ForEach-Object {
                $tip = "$($_.ToolTip)"
                if ($tip.Length -gt 120) {
                    $cut = if ([char]::IsHighSurrogate($tip[119])) { 119 } else { 120 }
                    $tip = $tip.Substring(0, $cut)
                }
                , @($_.CompletionText, $_.ListItemText, $_.ResultType.ToString(), $tip)
            })
        } catch {}
        __SP-Send "CMP;$(__SP-Escape (ConvertTo-Json -InputObject $report -Compress -Depth 4))"
    }
} catch {}
```

(Removed: `Set-PSReadLineOption -PredictionSource None`, the prompt wrapper, `ISTERM_TESTING`, `__IS-Escape-Value`.)

- [ ] **Step 5: Delete what the report replaces**

- Delete `src/engine/providers/powershell.rs`, `src/engine/providers/files.rs`, `assets/psWorker.ps1`, `tests/powershell_provider_test.rs`. `src/engine/providers/mod.rs` becomes:
  ```rust
  pub mod carapace;
  pub mod json_spec;
  pub mod zoxide;
  ```
- `src/vt/emulator.rs`: delete `use std::borrow::Cow;`, `CellExt` and its impl, `extract_command_text`, `preprocess_vt_bytes`; `process` becomes `self.parser.process(bytes);`. `src/vt/mod.rs`: `pub use emulator::HeadlessTerminal;`.
- `tests/vt_test.rs`: import only `HeadlessTerminal` (and `has_cpr_query` until Task 16); delete `test_command_text_extraction`, `test_command_text_preserves_trailing_spaces`, `test_ghost_text_filtering_dim_and_italic`, `test_ghost_text_filtering_psreadline_prediction_color`, `test_ansi_preprocessor_does_not_corrupt_cursor_cup`, `test_ansi_preprocessor_does_not_corrupt_rgb_or_256_colors`, `test_wide_characters_cjk_and_emojis`, `test_multiline_command_extraction`, `test_prompt_marker_isolation_oh_my_posh_and_powershell`.
- `tests/engine_test.rs`: delete the "FileProvider Tests" section (`test_file_provider_listing`) and the imports that become unused.
- `tests/tab_trigger_test.rs`: delete `test_powershell_provider_in_tab_trigger` and unused imports.

- [ ] **Step 6: Rewrite `src/core/app.rs`** — keep `default_json_spec_provider`, `default_git_spec`, `default_docker_spec` unchanged; replace everything else (imports, helpers, `App`) with:

```rust
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
        self.layout = Renderer::render_dropdown(&self.state, term, theme, cx, cy, out).ok().flatten();
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

        let ConPtySession { pair, child } =
            ConPtySession::spawn(shell_type, cols, rows, SpawnOptions { no_profile: self.no_profile })?;
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
```

- [ ] **Step 7:** `cargo build --all-targets` → fix leftover references the compiler reports. `cargo test` → all pass, including the five end-to-end tests.

- [ ] **Step 8: Manual check with your normal profile (oh-my-posh)** — `cargo build --release`, run `.\target\release\shell-panel.exe` in Windows Terminal:
1. The prompt's error indicator turns red after `Get-Item nope` (N2).
2. `git ` + Tab lists far more than 13 subcommands when carapace is installed (N8).
3. `git sta` + Tab → `git status `; `[System.IO.Fi` + Tab → dropdown, accepting keeps the `[`.
4. Type fast right after pressing Tab on `Get-ChildItem -`: typed characters appear at once and no stale dropdown pops up.
5. A multi-line command (`if ($true) {` Enter, then `Get-Chi` Tab) completes on the continuation line.

- [ ] **Step 9: Commit**

```bash
git add -A src assets tests
git commit -m "feat(core): drive completion from the PSReadLine report; remove worker, file provider, prompt wrapper and screen scraping"
```

---

## Phase C — input fidelity

### Task 10: Key encoding — AltGr, Ctrl/Alt+Backspace, Shift/Ctrl+Enter, modified F-keys (H11, N7)

**Files:**
- Modify: `src/io/key_event.rs`
- Test: `tests/io_test.rs`

- [ ] **Step 1: Write failing tests** — append to `tests/io_test.rs`:

```rust
#[test]
fn test_altgr_characters_are_sent_as_text() {
    // Windows reports AltGr as Ctrl+Alt.
    let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    for c in ['@', '?', '[', '\\', ']', '€', '{'] {
        let ev = make_key_event(KeyCode::Char(c), altgr, KeyEventKind::Press);
        assert_eq!(encode_key_event(&ev), c.to_string().into_bytes(), "AltGr char {:?}", c);
    }
    let ev = make_key_event(KeyCode::Char('a'), altgr, KeyEventKind::Press);
    assert_eq!(encode_key_event(&ev), vec![0x1b, 0x01]); // a real Ctrl+Alt+A chord
}

#[test]
fn test_backspace_and_enter_chords() {
    let key = |code, mods| encode_key_event(&make_key_event(code, mods, KeyEventKind::Press));
    assert_eq!(key(KeyCode::Backspace, KeyModifiers::NONE), vec![0x7f]);
    assert_eq!(key(KeyCode::Backspace, KeyModifiers::CONTROL), vec![0x08]); // BackwardKillWord
    assert_eq!(key(KeyCode::Backspace, KeyModifiers::ALT), vec![0x1b, 0x7f]);
    assert_eq!(key(KeyCode::Enter, KeyModifiers::NONE), b"\r".to_vec());
    // xterm has no sequence for these; ConPTY accepts win32-input-mode records.
    assert_eq!(key(KeyCode::Enter, KeyModifiers::SHIFT), b"\x1b[13;28;13;1;16;1_".to_vec());
    assert_eq!(key(KeyCode::Enter, KeyModifiers::CONTROL), b"\x1b[13;28;13;1;8;1_".to_vec());
}

#[test]
fn test_function_keys_keep_modifiers() {
    let key = |code, mods| encode_key_event(&make_key_event(code, mods, KeyEventKind::Press));
    assert_eq!(key(KeyCode::F(1), KeyModifiers::NONE), b"\x1bOP".to_vec());
    assert_eq!(key(KeyCode::F(1), KeyModifiers::SHIFT), b"\x1b[1;2P".to_vec());
    assert_eq!(key(KeyCode::F(5), KeyModifiers::CONTROL), b"\x1b[15;5~".to_vec());
    assert_eq!(key(KeyCode::F(12), KeyModifiers::NONE), b"\x1b[24~".to_vec());
}
```

- [ ] **Step 2:** `cargo test --test io_test` → failures (`'@'` → `[0]`, Ctrl+Backspace → `[0x7f]`, …).

- [ ] **Step 3: Implement** — in `src/io/key_event.rs`:

Replace the `KeyCode::Char(c) => { ... }` arm with:

```rust
        KeyCode::Char(c) => {
            let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
            let alt = event.modifiers.contains(KeyModifiers::ALT);
            let mut buf = [0u8; 4];
            let text = c.encode_utf8(&mut buf).as_bytes().to_vec();

            // Windows reports AltGr as Ctrl+Alt. A non-letter with both modifiers is the character
            // the keyboard layout produced (e.g. '@' via AltGr+Q on German layouts), not a chord.
            if ctrl && alt && !c.is_ascii_alphabetic() {
                return text;
            }

            let mut bytes = if ctrl {
                control_byte(c).map(|b| vec![b]).unwrap_or(text)
            } else {
                text
            };
            if alt {
                bytes.insert(0, 0x1b);
            }
            bytes
        }
```

Replace the `Enter` and `Backspace` arms with:

```rust
        KeyCode::Enter => {
            // win32-input-mode record: Vk=13, Sc=28, Uc=13, KeyDown, control-key state, repeat 1.
            let shift = event.modifiers.contains(KeyModifiers::SHIFT);
            let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
            match (shift, ctrl) {
                (false, false) => vec![b'\r'],
                _ => {
                    let state = (if shift { 0x10 } else { 0 }) | (if ctrl { 0x08 } else { 0 });
                    format!("\x1b[13;28;13;1;{};1_", state).into_bytes()
                }
            }
        }
        KeyCode::Backspace => {
            if event.modifiers.contains(KeyModifiers::CONTROL) {
                vec![0x08]
            } else if event.modifiers.contains(KeyModifiers::ALT) {
                vec![0x1b, 0x7f]
            } else {
                vec![0x7f]
            }
        }
```

Replace the `KeyCode::F(n)` arm with:

```rust
        KeyCode::F(n) => {
            // F1-F4 are SS3 P..S (CSI 1;m P..S with modifiers); the rest are CSI <code>~.
            let code = match n {
                5 => 15,
                6 => 17,
                7 => 18,
                8 => 19,
                9 => 20,
                10 => 21,
                11 => 23,
                12 => 24,
                _ => 0,
            };
            match (n, mod_code > 1) {
                (1..=4, false) => vec![0x1b, b'O', b'P' + (n - 1)],
                (1..=4, true) => format!("\x1b[1;{}{}", mod_code, (b'P' + (n - 1)) as char).into_bytes(),
                (5..=12, false) => format!("\x1b[{}~", code).into_bytes(),
                (5..=12, true) => format!("\x1b[{};{}~", code, mod_code).into_bytes(),
                _ => Vec::new(),
            }
        }
```

Add below `xterm_modifier_code`:

```rust
/// Control code produced by Ctrl+`c`, if any.
fn control_byte(c: char) -> Option<u8> {
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_lowercase() as u8 - b'a' + 1);
    }
    match c {
        '@' | ' ' => Some(0),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' => Some(0x1f),
        '?' => Some(0x7f),
        _ => None,
    }
}
```

- [ ] **Step 4:** `cargo test` → all pass. Manual: in shell-panel, `echo one` Shift+Enter shows the `>>` continuation prompt; Ctrl+Backspace deletes a word.

- [ ] **Step 5: Commit**

```bash
git add src/io/key_event.rs tests/io_test.rs
git commit -m "fix(io): send AltGr text, Ctrl/Alt+Backspace, Shift/Ctrl+Enter and modified function keys correctly"
```

---

### Task 11: Lexer — script blocks, parentheses and call operators (N10)

**Files:**
- Modify: `src/engine/lexer.rs`
- Test: `tests/lexer_test.rs`

- [ ] **Step 1: Write failing tests** — append to `tests/lexer_test.rs`:

```rust
fn texts(input: &str) -> Vec<String> {
    lex_command_line(input).into_iter().map(|t| t.text).collect()
}

#[test]
fn test_command_inside_script_block_or_parentheses() {
    assert_eq!(texts("if ($x) { git sta"), vec!["git", "sta"]);
    assert_eq!(texts("$r = (git sta"), vec!["git", "sta"]);
    assert_eq!(texts("foreach ($f in $files) { docker "), vec!["docker", ""]);
    // Quoted braces are text.
    assert_eq!(texts("echo '{ git' sta"), vec!["echo", "{ git", "sta"]);
}

#[test]
fn test_call_operators_are_not_the_command() {
    assert_eq!(texts("& git sta"), vec!["git", "sta"]);
    assert_eq!(texts(". git sta"), vec!["git", "sta"]);
    assert_eq!(texts("& 'C:\\Program Files\\Git\\cmd\\git.exe' sta"), vec!["C:\\Program Files\\Git\\cmd\\git.exe", "sta"]);
    assert_eq!(texts("./build.ps1 -Fa"), vec!["./build.ps1", "-Fa"]);
}
```

- [ ] **Step 2:** `cargo test --test lexer_test` → the new tests fail.

- [ ] **Step 3: Implement**

In `split_segments`, inside `DelimQuoteState::Normal`, add a branch before the final `else`:

```rust
                } else if ch == '(' || ch == '{' {
                    // A sub-expression or script block starts a new command.
                    segments.push(&input[start..byte_pos]);
                    i += 1;
                    start = if i < len { chars[i].0 } else { input.len() };
```

In `lex_command_line`, right after `let raw_tokens = lex_segment(last_segment);`, make it mutable and drop a leading call operator:

```rust
    let mut raw_tokens = lex_segment(last_segment);
    // `& cmd` and `. cmd` invoke `cmd`: the operator is not the command.
    if raw_tokens.len() > 1
        && !raw_tokens[0].closed_quote
        && (raw_tokens[0].text == "&" || raw_tokens[0].text == ".")
    {
        raw_tokens.remove(0);
    }
```

In `active_token_raw` add `'(' | '{'` to the characters that reset `start`.

- [ ] **Step 4:** `cargo test` → all pass.

- [ ] **Step 5: Commit**

```bash
git add src/engine/lexer.rs tests/lexer_test.rs
git commit -m "fix(engine): lex commands inside script blocks and after call operators"
```

---

## Phase D — robustness

### Task 12: Stop the busy loop on closed input and cap message buffering (M1, M3)

**Files:**
- Modify: `src/core/app.rs`, `src/shell/stream.rs`
- Test: `tests/stream_test.rs`

**Interfaces:**
- Produces: `pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;` in `shell_panel::shell::stream`.

- [ ] **Step 1: Write the failing test** — append to `tests/stream_test.rs` (extend the import with `MAX_MESSAGE_BYTES`):

```rust
#[test]
fn test_unterminated_message_is_not_buffered_forever() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::default();
    let mut residual = Vec::new();

    let mut chunk = b"\x1b]6973;CMP;".to_vec();
    chunk.extend(std::iter::repeat(b'a').take(MAX_MESSAGE_BYTES + 1));
    let out = ingest_pty_chunk(&chunk, &mut term, &mut state, &mut residual);

    assert!(residual.is_empty());
    assert_eq!(out.len(), chunk.len());
}
```

- [ ] **Step 2:** `cargo test --test stream_test` → compile error.

- [ ] **Step 3: Implement**

`src/shell/stream.rs` — add below `OSC_PREFIX`:

```rust
/// Largest unterminated message kept waiting for its terminator. A completion report is capped
/// at 100 matches (tens of kilobytes); anything larger is passed through as ordinary output.
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
```

and replace the `let Some(...) = terminator else { ... };` block with:

```rust
        let Some((offset, terminator_len)) = terminator else {
            if data.len() - i > MAX_MESSAGE_BYTES {
                // Never terminated: stop waiting and treat it as ordinary output.
                term.process(&data[i..]);
                clean_output.extend_from_slice(&data[i..]);
            } else {
                // Unterminated message at the chunk boundary: wait for the next chunk.
                residual.extend_from_slice(&data[i..]);
            }
            return clean_output;
        };
```

`src/core/app.rs` — before `loop {` add `let mut input_open = true;` and change the key branch header and event match to:

```rust
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
```

- [ ] **Step 4:** `cargo test` → all pass.

- [ ] **Step 5: Commit**

```bash
git add src/shell/stream.rs src/core/app.rs tests/stream_test.rs
git commit -m "fix(core): stop polling closed console input and cap unterminated message buffering"
```

---

### Task 13: Restore covered rows with their colors (M5)

**Files:**
- Modify: `src/ui/patch.rs`
- Test: `tests/renderer_test.rs`

vt100 0.15.2: `Screen::rows_formatted(&self, start: u16, width: u16) -> impl Iterator<Item = Vec<u8>>`, one item per visible row (`start`/`width` are columns).

- [ ] **Step 1: Write the failing test** — append to `tests/renderer_test.rs`:

```rust
#[test]
fn test_restore_line_keeps_colors() {
    let mut term = HeadlessTerminal::new(80, 24);
    term.process(b"\x1b[31mRED\x1b[0m plain");

    let restored = restore_line(0, &term);

    assert!(restored.starts_with("\x1b[1;1H"));
    assert!(restored.contains("31m"), "color lost: {:?}", restored);
    assert!(restored.contains("RED"));
    assert!(restored.contains("plain"));

    // Replaying the restore on a blank screen reproduces the colored cell.
    let mut replay = HeadlessTerminal::new(80, 24);
    replay.process(restored.as_bytes());
    assert_eq!(replay.screen().cell(0, 0).unwrap().fgcolor(), vt100::Color::Idx(1));
    assert_eq!(replay.screen().cell(0, 4).unwrap().fgcolor(), vt100::Color::Default);
}
```

- [ ] **Step 2:** `cargo test --test renderer_test test_restore_line_keeps_colors` → FAIL `color lost`.

- [ ] **Step 3: Implement** — replace `restore_line` in `src/ui/patch.rs`:

```rust
/// Returns an ANSI sequence that moves to the start of `row`, clears it and redraws the row
/// exactly as the headless terminal holds it, colors and attributes included.
pub fn restore_line(row: u16, term: &HeadlessTerminal) -> String {
    let formatted = term
        .screen()
        .rows_formatted(0, term.cols)
        .nth(row as usize)
        .unwrap_or_default();
    format!(
        "\x1b[{};1H\x1b[0m\x1b[2K{}\x1b[0m",
        row + 1,
        String::from_utf8_lossy(&formatted)
    )
}
```

- [ ] **Step 4:** `cargo test` → all pass.

- [ ] **Step 5: Commit**

```bash
git add src/ui/patch.rs tests/renderer_test.rs
git commit -m "fix(ui): restore rows under the dropdown with their original colors"
```

---

### Task 14: Log to a file, refuse nested sessions under an own variable, enable host VT processing (M6, M7, N12, N13)

**Files:**
- Modify: `src/main.rs`, `src/cli.rs`, `src/pty/conpty.rs`
- Test: `tests/cli_test.rs`

**Interfaces:**
- Produces: `pub const SESSION_ENV: &str = "SHELL_PANEL_SESSION";` in `shell_panel::pty::conpty` (replaces `ISTERM`).

- [ ] **Step 1: Write the failing test** — append to `tests/cli_test.rs`:

```rust
fn run_with_session_env(args: &[&str], in_session: bool) -> std::process::Output {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_shell-panel"));
    cmd.args(args).env_remove("SHELL_PANEL_SESSION");
    if in_session {
        cmd.env("SHELL_PANEL_SESSION", "1");
    }
    cmd.output().unwrap()
}

#[test]
fn test_refuses_to_start_inside_existing_session() {
    let out = run_with_session_env(&[], true);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("already running"));
}

#[test]
fn test_check_reports_session_state() {
    assert_eq!(run_with_session_env(&["--check"], true).status.code(), Some(0));
    assert_eq!(run_with_session_env(&["--check"], false).status.code(), Some(1));
}
```

- [ ] **Step 2:** `cargo test --test cli_test` → the new tests fail.

- [ ] **Step 3: Implement**

`src/pty/conpty.rs`: add

```rust
/// Environment variable set to `1` inside a shell-panel session.
pub const SESSION_ENV: &str = "SHELL_PANEL_SESSION";
```

and replace `cmd.env("ISTERM", "1");` with `cmd.env(SESSION_ENV, "1");`.

`src/cli.rs`: doc comment of `verbose` becomes `/// Write a debug log to %TEMP%\shell-panel\shell-panel.log`.

Replace `src/main.rs` with:

```rust
use std::path::PathBuf;

use clap::Parser;
use shell_panel::cli::Cli;
use shell_panel::core;
use shell_panel::pty::conpty::SESSION_ENV;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Sends `tracing` output to a log file: the terminal is in raw mode and owned by the shell.
fn init_file_logging() -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join("shell-panel");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("shell-panel.log");
    let file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("shell_panel=debug"))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(file)),
        )
        .init();
    Ok(path)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.print_default_config {
        println!("{}", core::config::default_sample_toml());
        return Ok(());
    }

    let in_session = std::env::var(SESSION_ENV).as_deref() == Ok("1");
    if cli.check {
        if in_session {
            println!("shell-panel session active.");
            std::process::exit(0);
        }
        println!("Not in a shell-panel session.");
        std::process::exit(1);
    }

    if in_session {
        eprintln!("shell-panel: already running in this terminal ({SESSION_ENV}=1); refusing to start a nested session.");
        std::process::exit(1);
    }

    if cli.verbose {
        let path = init_file_logging()?;
        eprintln!("shell-panel: writing debug log to {}", path.display());
    }

    // The shell's output is raw VT. Windows Terminal always interprets it; a legacy console
    // window only does after this call (it enables ENABLE_VIRTUAL_TERMINAL_PROCESSING).
    #[cfg(windows)]
    let _ = crossterm::ansi_support::supports_ansi();

    // Restore the console if we panic while it is in raw mode.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
        default_hook(info);
    }));

    let config = core::config::Config::load_or_default(cli.config.as_deref());
    let shell = cli.shell.or_else(|| config.shell.clone());
    let mut app = core::app::App::new(config, shell);
    app.no_profile = cli.no_profile;
    let exit_code = app.run().await?;

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }

    Ok(())
}
```

Add a few `tracing::debug!` calls in `src/core/app.rs` so the log is useful: when the report is requested, when it arrives (`matches = report.matches.len()`), when it times out, and the number of merged results.

- [ ] **Step 4:** `cargo test` → all pass.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/cli.rs src/pty/conpty.rs src/core/app.rs tests/cli_test.rs
git commit -m "fix(cli): log to a file, guard nested sessions with SHELL_PANEL_SESSION and enable host VT processing"
```

---

### Task 15: Validate the shell and the configuration file (M8, M9)

**Files:**
- Modify: `src/pty/shell.rs`, `src/core/config.rs`, `src/main.rs`
- Test: `tests/pty_test.rs`, `tests/config_test.rs`, `tests/e2e_pty_test.rs`

**Interfaces:**
- Produces: `is_supported_shell(name: &str) -> bool` in `shell_panel::pty::shell`; `Config::load(custom_path: Option<&Path>) -> (Config, Vec<String>)`; `Config::new(max_suggestions: usize)`; field `debounce_ms` removed.

An unknown key produces a warning but the rest of the file is still used (rejecting the whole file because of one typo — or because of the removed `debounce_ms` — would silently reset a user's theme).

- [ ] **Step 1: Write failing tests**

`tests/pty_test.rs` (extend the import with `is_supported_shell`):

```rust
#[test]
fn test_is_supported_shell() {
    for name in ["pwsh", "PWSH.exe", "powershell", "PowerShell.EXE"] {
        assert!(is_supported_shell(name), "{name}");
    }
    for name in ["cmd", "bash", "C:\\tools\\pwsh.exe", ""] {
        assert!(!is_supported_shell(name), "{name}");
    }
}
```

`tests/config_test.rs` (add `use std::path::Path;` if missing):

```rust
fn write_temp(tag: &str, content: &str) -> std::path::PathBuf {
    let file = std::env::temp_dir().join(format!("sp_cfg_{}_{}.toml", tag, std::process::id()));
    std::fs::write(&file, content).unwrap();
    file
}

#[test]
fn test_unknown_keys_are_reported_but_the_rest_is_used() {
    let file = write_temp("typo", "max_suggestions = 9\ndebounce_ms = 30\n[colors]\nselected_gb = \"red\"\n");
    let (config, warnings) = Config::load(Some(&file));
    let _ = std::fs::remove_file(&file);

    assert_eq!(config.max_suggestions, 9);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("debounce_ms")));
    assert!(warnings.iter().any(|w| w.contains("colors.selected_gb")));
}

#[test]
fn test_invalid_or_missing_explicit_file_is_reported() {
    let file = write_temp("bad", "max_suggestions = \"many\"\n");
    let (config, warnings) = Config::load(Some(&file));
    let _ = std::fs::remove_file(&file);
    assert_eq!(config, Config::default());
    assert_eq!(warnings.len(), 1);

    let (config, warnings) = Config::load(Some(Path::new("non_existent_config_file_12345.toml")));
    assert_eq!(config, Config::default());
    assert_eq!(warnings.len(), 1);
}
```

`tests/e2e_pty_test.rs` `test_app_new_and_config`: `Config::new(10, 20)` → `Config::new(10)`; delete the `debounce_ms` assertion.

- [ ] **Step 2:** `cargo test --test pty_test --test config_test --test e2e_pty_test` → compile errors.

- [ ] **Step 3: Implement**

`src/pty/shell.rs`:

```rust
/// True for the shell names shell-panel can drive; its integration is PowerShell-only.
pub fn is_supported_shell(name: &str) -> bool {
    ["pwsh", "pwsh.exe", "powershell", "powershell.exe"]
        .iter()
        .any(|s| s.eq_ignore_ascii_case(name))
}
```

`src/core/config.rs`: delete `default_debounce_ms`, the `debounce_ms` field and its initializer; replace `Config::new` and `load_or_default` with:

```rust
    pub fn new(max_suggestions: usize) -> Self {
        Self {
            max_suggestions,
            ..Default::default()
        }
    }

    /// Loads the configuration and returns it with messages for the user. A file that cannot be
    /// read or parsed yields the defaults; unknown keys are reported and ignored; a missing file
    /// at the default location is not a problem.
    pub fn load(custom_path: Option<&Path>) -> (Self, Vec<String>) {
        let (path, explicit) = match custom_path {
            Some(p) => (p.to_path_buf(), true),
            None => match default_config_path() {
                Some(p) => (p, false),
                None => return (Self::default(), Vec::new()),
            },
        };

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if !explicit && err.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), Vec::new())
            }
            Err(err) => {
                return (
                    Self::default(),
                    vec![format!("could not read config {}: {}", path.display(), err)],
                )
            }
        };

        match toml::from_str::<Self>(&content) {
            Ok(config) => {
                let warnings = content
                    .parse::<toml::Table>()
                    .map(|table| unknown_keys(&table))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|key| format!("unknown key `{}` in {} (ignored)", key, path.display()))
                    .collect();
                (config, warnings)
            }
            Err(err) => (
                Self::default(),
                vec![format!("invalid config {}: {}; using defaults", path.display(), err)],
            ),
        }
    }

    /// Like [`Config::load`], discarding the warnings.
    pub fn load_or_default(custom_path: Option<&Path>) -> Self {
        Self::load(custom_path).0
    }
```

and add below `impl Config`:

```rust
const TOP_LEVEL_KEYS: &[&str] = &["max_suggestions", "shell", "colors", "icons"];
const COLOR_KEYS: &[&str] = &[
    "selected_bg", "selected_fg", "unselected_fg", "description_fg", "selected_prefix", "unselected_prefix",
];
const ICON_KEYS: &[&str] = &[
    "directory", "file", "command", "subcommand", "option", "powershell_cmdlet", "alias", "other",
];

/// Keys serde would silently ignore, as dotted paths.
fn unknown_keys(table: &toml::Table) -> Vec<String> {
    let mut unknown = Vec::new();
    for (key, value) in table {
        let section_keys = match key.as_str() {
            "colors" => COLOR_KEYS,
            "icons" => ICON_KEYS,
            k if TOP_LEVEL_KEYS.contains(&k) => continue,
            _ => {
                unknown.push(key.clone());
                continue;
            }
        };
        if let toml::Value::Table(section) = value {
            unknown.extend(
                section
                    .keys()
                    .filter(|k| !section_keys.contains(&k.as_str()))
                    .map(|k| format!("{key}.{k}")),
            );
        }
    }
    unknown
}
```

In `default_sample_toml` replace the two shell lines with:

```toml
# Shell to launch: "pwsh" or "powershell" (default: pwsh.exe when on PATH, else powershell.exe)
# shell = "pwsh"
```

`src/main.rs`: add `use shell_panel::pty::shell::is_supported_shell;` and replace the two config/shell lines with:

```rust
    let (config, config_warnings) = core::config::Config::load(cli.config.as_deref());
    for warning in config_warnings {
        eprintln!("shell-panel: {warning}");
    }

    let shell = cli.shell.or_else(|| config.shell.clone());
    if let Some(name) = shell.as_deref() {
        if !is_supported_shell(name) {
            eprintln!("shell-panel: unsupported shell {name:?}; use \"pwsh\" or \"powershell\"");
            std::process::exit(2);
        }
    }
```

- [ ] **Step 4:** `cargo test` → all pass.

- [ ] **Step 5: Commit**

```bash
git add src/pty/shell.rs src/core/config.rs src/main.rs tests/pty_test.rs tests/config_test.rs tests/e2e_pty_test.rs
git commit -m "fix(config): report config errors and unknown keys, drop unused debounce_ms, reject unsupported shells"
```

---

## Phase E — cleanup

### Task 16: Remove dead code and low-value tests, trim dependencies, make clippy clean (L1, T2)

**Files:**
- Delete: `src/vt/cpr.rs`
- Modify: `src/vt/mod.rs`, `src/pty/conpty.rs`, `src/ui/suggestion_state.rs`, `src/engine/provider.rs`, `src/ui/patch.rs`, `src/ui/renderer.rs`, `src/ui/theme.rs`, `src/ui/mod.rs`, `src/engine/providers/json_spec.rs`, `Cargo.toml`, `.gitignore`, tests

- [ ] **Step 1: Delete unused code and the tests that only exercised it**

1. `src/vt/cpr.rs` (never called): delete; `src/vt/mod.rs` becomes `pub mod emulator;` + `pub use emulator::HeadlessTerminal;`. In `tests/vt_test.rs` delete the `has_cpr_query` import and `test_cpr_query_detection`.
2. `src/pty/conpty.rs`: delete `ConPtySession::kill` and `try_wait`, drop `ExitStatus` from the import, and change the context message to `format!("Failed to start shell process {}", shell_type.executable_name())`. In `tests/pty_test.rs` `test_conpty_session_spawn_and_resize`: `.expect("Failed to create ConPTY session")` and `let _ = session.child.kill();`.
3. `src/ui/suggestion_state.rs`: delete `page_info`; in `tests/renderer_test.rs` delete the three `page_info` assertions.
4. `src/engine/provider.rs`: delete `impl SuggestionKind { fn icon }` (the theme's `IconConfig` is the single source); in `tests/theme_test.rs` delete `test_all_suggestion_kind_icons`.
5. `src/ui/patch.rs`: delete `LinePatch`; in `tests/renderer_test.rs` delete `test_line_patch_struct` and import only `restore_line`.
6. `src/ui/renderer.rs`: delete the free functions `render_dropdown` and `clear_dropdown`.
7. `src/ui/theme.rs`: delete the free functions `format_selected`, `format_description`, `format_suggestion_line_with_min_width`.
8. `src/ui/mod.rs` re-exports become:
   ```rust
   pub use color::{parse_color_bg, parse_color_fg};
   pub use patch::restore_line;
   pub use renderer::{DropdownLayout, Renderer};
   pub use suggestion_state::SuggestionState;
   pub use theme::{
       format_suggestion_line, format_suggestion_line_with_theme,
       format_suggestion_line_with_theme_and_min_width, Theme, SELECTED_PREFIX, UNSELECTED_PREFIX,
   };
   ```
9. `src/engine/providers/json_spec.rs`: delete `from_specs`.

- [ ] **Step 2: Trim dependencies** — in `Cargo.toml` delete `thiserror` and set

```toml
tokio = { version = "1.40", features = ["macros", "rt-multi-thread", "sync", "time", "process"] }
```

Run `cargo build --all-targets`; if a missing tokio feature is reported, add exactly that one.

- [ ] **Step 3: Clippy** — `cargo clippy --fix --all-targets --allow-dirty`, then `cargo clippy --all-targets -- -D warnings`. Fix the rest by hand; for `field_reassign_with_default` in tests rewrite `let mut config = Config::default(); config.colors = ColorConfig { .. };` as `let config = Config { colors: ColorConfig { .. }, ..Default::default() };`. Expected at the end: exit code 0.

- [ ] **Step 4:** `cargo fmt`, `cargo test` → all pass.

- [ ] **Step 5: Commit**

```bash
git add -A src tests Cargo.toml Cargo.lock .gitignore
git commit -m "refactor: remove dead code and tests of it, trim dependencies, fix clippy warnings"
```

---

### Task 17: Move Fig specs to JSON and load user specs (L2)

**Files:**
- Create: `assets/specs/git.json`, `assets/specs/docker.json` (generated)
- Modify: `src/engine/providers/json_spec.rs`, `src/core/app.rs`
- Test: `tests/engine_test.rs`, `tests/e2e_pty_test.rs`

**Interfaces:**
- Produces: `JsonSpecProvider::with_embedded_specs() -> Self`, `JsonSpecProvider::load_dir(&mut self, dir: &Path) -> Vec<String>`, `default_specs_dir() -> Option<PathBuf>` (= `%USERPROFILE%\.config\shell-panel\specs`). `default_json_spec_provider`, `default_git_spec`, `default_docker_spec` are removed from `core::app`.

- [ ] **Step 1: Generate the JSON from the current Rust specs** — create a throw-away `tests/zz_dump_specs.rs`:

```rust
use shell_panel::core::app::{default_docker_spec, default_git_spec};

#[test]
fn dump_specs() {
    std::fs::create_dir_all("assets/specs").unwrap();
    std::fs::write("assets/specs/git.json", serde_json::to_string_pretty(&default_git_spec()).unwrap()).unwrap();
    std::fs::write("assets/specs/docker.json", serde_json::to_string_pretty(&default_docker_spec()).unwrap()).unwrap();
}
```

Run `cargo test --test zz_dump_specs`, delete the file, check `assets/specs/git.json` contains `"name": "git"`.

- [ ] **Step 2: Write failing tests** — append to `tests/engine_test.rs` (temporary import `use shell_panel::core::app::{default_docker_spec, default_git_spec};`):

```rust
#[test]
fn test_embedded_specs_match_previous_rust_definitions() {
    let provider = JsonSpecProvider::with_embedded_specs();
    assert_eq!(provider.specs["git"], default_git_spec());
    assert_eq!(provider.specs["docker"], default_docker_spec());
}

#[tokio::test]
async fn test_load_dir_adds_user_specs_and_reports_bad_files() {
    let dir = std::env::temp_dir().join(format!("sp_specs_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mytool.json"),
        r#"{"name":"mytool","subcommands":[{"name":"deploy","description":"Ship it"}]}"#,
    )
    .unwrap();
    std::fs::write(dir.join("broken.json"), "{").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();

    let mut provider = JsonSpecProvider::with_embedded_specs();
    let warnings = provider.load_dir(&dir);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(warnings.len(), 1, "{:?}", warnings);
    assert!(warnings[0].contains("broken.json"));
    let sugs = provider.complete("mytool dep", "").await;
    assert_eq!(sugs[0].name, "deploy");
}
```

`cargo test --test engine_test` → compile errors.

- [ ] **Step 3: Implement** — in `src/engine/providers/json_spec.rs` add `use std::path::{Path, PathBuf};` and:

```rust
const EMBEDDED_SPECS: &[&str] = &[
    include_str!("../../../assets/specs/git.json"),
    include_str!("../../../assets/specs/docker.json"),
];

/// Directory for user Fig specs: `%USERPROFILE%\.config\shell-panel\specs`.
pub fn default_specs_dir() -> Option<PathBuf> {
    crate::core::config::default_config_path()
        .map(|config| config.with_file_name("shell-panel").join("specs"))
}
```

inside `impl JsonSpecProvider`:

```rust
    /// Provider preloaded with the specs embedded in the binary.
    pub fn with_embedded_specs() -> Self {
        let mut provider = Self::new();
        for json in EMBEDDED_SPECS {
            let spec: FigSpec = serde_json::from_str(json).expect("embedded spec is valid JSON");
            provider.add_spec(spec);
        }
        provider
    }

    /// Loads every `*.json` Fig spec in `dir`; a user spec replaces an embedded one of the same name.
    /// Returns one message per file that could not be read or parsed.
    pub fn load_dir(&mut self, dir: &Path) -> Vec<String> {
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => return vec![format!("could not read spec directory {}: {}", dir.display(), err)],
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let parsed = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|json| serde_json::from_str::<FigSpec>(&json).map_err(|e| e.to_string()));
            match parsed {
                Ok(spec) => self.add_spec(spec),
                Err(err) => warnings.push(format!("invalid spec {}: {}", path.display(), err)),
            }
        }
        warnings
    }
```

- [ ] **Step 4:** `cargo test --test engine_test` → pass (the equality test proves the JSON equals the Rust specs).

- [ ] **Step 5: Remove the Rust specs**

1. `tests/engine_test.rs`: delete `test_embedded_specs_match_previous_rust_definitions` and the temporary import.
2. `tests/e2e_pty_test.rs`: in `test_default_json_spec_provider_git_and_docker` build the provider with `JsonSpecProvider::with_embedded_specs()`; fix imports.
3. `src/core/app.rs`: delete `default_json_spec_provider`, `default_git_spec`, `default_docker_spec`; import `use crate::engine::providers::json_spec::{default_specs_dir, JsonSpecProvider};`; in `App::run`, before `RawModeGuard::enter()`:
   ```rust
        let mut json_specs = JsonSpecProvider::with_embedded_specs();
        if let Some(dir) = default_specs_dir().filter(|d| d.is_dir()) {
            for warning in json_specs.load_dir(&dir) {
                eprintln!("shell-panel: {warning}");
            }
        }
   ```
   and `let engine = CompletionEngine::new(json_specs);`.
4. `tests/aggregate_test.rs` needs no change (it builds its own provider).

- [ ] **Step 6:** `cargo test` → all pass; `cargo clippy --all-targets -- -D warnings` → exit 0.

- [ ] **Step 7: Commit**

```bash
git add -A assets/specs src tests
git commit -m "refactor(engine): embed fig specs as JSON and load user specs from ~/.config/shell-panel/specs"
```

---

## Phase F — documentation

### Task 18: Rewrite the README and mark old plans (D1)

**Files:**
- Modify: `README.md`, the four older files in `docs/superpowers/plans/`

- [ ] **Step 1: Collect the facts** — run `cargo run -q -- --help` and `cargo run -q -- --print-default-config`. Where their output differs from the README text below, the README follows the output.

- [ ] **Step 2: Replace `README.md`**

````markdown
# shell-panel

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

IDE-style Tab completion dropdown for PowerShell on Windows, written in Rust.
Inspired by [`@microsoft/inshellisense`](https://github.com/microsoft/inshellisense), without a Node.js runtime.

## How it works

shell-panel starts PowerShell inside a ConPTY pseudo-terminal and sits between it and your terminal.

- A small integration script is embedded in the binary and passed to PowerShell with `-EncodedCommand`: nothing is written to disk and your execution policy is not touched. It wraps `PSConsoleHostReadLine` to tell shell-panel when PSReadLine is reading a line (and in which directory), and binds **Ctrl+Alt+Shift+F12** to a handler that reports the current line, the cursor and PowerShell's own completions for it.
- When you press **Tab**, shell-panel sends that chord, receives the report from *your* session — so variables, functions, registered argument completers and the current location are all known — merges it with its other sources and draws a dropdown over the terminal. A headless VT100 emulator mirrors the screen so the rows under the dropdown are restored exactly, colors included.
- If PSReadLine is not reading (a program is running, `Read-Host`, a full-screen app) or no report arrives within 3 seconds, Tab goes to PowerShell unchanged.

```
Terminal (Windows Terminal, conhost, VS Code)
   │ keys                         ▲ output (+ dropdown)
   ▼                              │
shell-panel: key routing · OSC 6973 messages · headless VT100 · completion engine · dropdown renderer
   │ input (+ report request)     ▲ output (+ reports)
   ▼                              │
ConPTY ── pwsh.exe / powershell.exe + integration script (ReadLine markers, completion report)
```

## Completion sources

| Source | Provides | Requirement |
|--------|----------|-------------|
| PowerShell (your session) | Cmdlets, parameters, variables, types, paths (quoted when needed), registered argument completers | PSReadLine 2.x |
| Fig-style JSON specs | Subcommands and options of `git` and `docker` (built in) and of any spec in `%USERPROFILE%\.config\shell-panel\specs\*.json` | — |
| Carapace | Subcommands, options and arguments of hundreds of CLIs | `carapace` on PATH |
| Zoxide | Frequent directories for `cd`, `z`, `zi` | `zoxide` on PATH |

Sources are queried concurrently and merged: highest priority first, one entry per name. When a command has subcommand or option suggestions, files are only shown if the word being completed looks like a path.

## Requirements

- Windows 10 1809 or later (ConPTY). Developed and tested on Windows 11.
- PowerShell 7 (`pwsh.exe`) or Windows PowerShell 5.1 (`powershell.exe`), with PSReadLine 2.0 or later (bundled with both).
- A recent stable Rust toolchain to build.

## Build and run

```powershell
cargo build --release
.\target\release\shell-panel.exe
```

The shell starts in the directory you start shell-panel from. Starting shell-panel inside a shell-panel session is refused (`SHELL_PANEL_SESSION=1` is set inside a session; `shell-panel --check` tests for it).

## Keys

| Key | Dropdown closed | Dropdown open |
|-----|-----------------|---------------|
| Tab | Complete: one match is inserted directly, several open the dropdown, none falls back to PowerShell's Tab | Insert the highlighted suggestion |
| Down / Up, Shift+Tab | Passed to PowerShell | Move the highlight (wraps around) |
| Esc | Passed to PowerShell | Close the dropdown |
| Any other key | Passed to PowerShell | Close the dropdown and pass the key on |

Completion works at the cursor, also in the middle of a line and on continuation lines. **Ctrl+Alt+Shift+F12** is reserved for shell-panel inside the session.

## Command-line options

```
shell-panel [OPTIONS]

  -s, --shell <SHELL>         pwsh or powershell (default: pwsh.exe when on PATH, else powershell.exe)
  -v, --verbose               Write a debug log to %TEMP%\shell-panel\shell-panel.log
  -c, --check                 Exit 0 when running inside a shell-panel session, 1 otherwise
      --config <CONFIG>       Configuration file (default: %USERPROFILE%\.config\shell-panel.toml)
      --no-profile            Start PowerShell without loading profiles
      --print-default-config  Print a sample configuration and exit
  -h, --help                  Print help
  -V, --version               Print version
```

## Configuration

```powershell
New-Item -ItemType Directory -Force "$HOME\.config" | Out-Null
shell-panel --print-default-config | Set-Content "$HOME\.config\shell-panel.toml"
```

| Key | Meaning | Default |
|-----|---------|---------|
| `max_suggestions` | Rows per dropdown page (fewer when the terminal is short) | `5` |
| `shell` | `"pwsh"` or `"powershell"`; `--shell` overrides it | auto |
| `[colors] selected_bg`, `selected_fg` | Highlighted row | `cyan`, `black` |
| `[colors] unselected_fg`, `description_fg` | Other rows, descriptions | terminal default, `gray` |
| `[colors] selected_prefix`, `unselected_prefix` | Row prefixes | `"> "`, `"  "` |
| `[icons] directory`, `file`, `command`, `subcommand`, `option`, `powershell_cmdlet`, `alias`, `other` | Icon per suggestion kind | see sample |

Colors accept names (`red`, `bright_blue`, `gray`, …), 256-color indices (`"244"`), hex (`"#3b82f6"`, `"#38f"`), `"reverse"` and `"default"`.
Problems are printed at start-up: an unreadable or invalid file falls back to the defaults; unknown keys are named and ignored.

## Custom command specs

Put Fig-style JSON files in `%USERPROFILE%\.config\shell-panel\specs\`. A spec with the same `name` as a built-in one replaces it.

```json
{
  "name": "mytool",
  "description": "Internal deployment tool",
  "subcommands": [
    { "name": "deploy", "description": "Ship it", "options": [ { "name": ["-f", "--force"], "description": "Skip checks" } ] }
  ],
  "options": [ { "name": "--help", "description": "Show help" } ]
}
```

## Testing

```powershell
cargo test
```

Most tests are pure. The PTY and end-to-end tests start real PowerShell sessions (with `-NoProfile`) through ConPTY — the end-to-end ones run the actual `shell-panel` binary — and need `pwsh.exe` or `powershell.exe` on PATH.

## Known limitations

- Windows and PowerShell only.
- PowerShell's completions are computed on the shell's thread, like native Tab: a slow completer delays the dropdown (after 3 seconds Tab falls back to PowerShell).
- The report travels through the terminal stream as an OSC sequence. This is verified on Windows 11; very old Windows 10 console hosts may truncate long sequences.
- A Tab character inside pasted text triggers completion instead of being inserted.

## License

MIT — see [LICENSE](LICENSE).
````

- [ ] **Step 3: Mark the old plans** — insert directly under the `# ...` title of each plan in `docs/superpowers/plans/` except `2026-09-21-review-fixes.md`:

```markdown
> **Status:** executed — see the git history for the resulting commits. Checkbox state below was not maintained during execution. Parts of the design were later replaced: see `2026-09-21-review-fixes.md`.
```

- [ ] **Step 4: Verify** — `cargo test` → all pass. Every option in the README block exists in the `--help` output of Step 1.

- [ ] **Step 5:** Tick the completed checkboxes in this file.

- [ ] **Step 6: Commit**

```bash
git add README.md docs/superpowers/plans
git commit -m "docs: rewrite README for session-based completion, config and specs; mark executed plans"
```
