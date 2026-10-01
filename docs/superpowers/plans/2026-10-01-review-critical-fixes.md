# Review Critical Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four Critical findings of the 2026-10-01 general review (C1–C4) and the Important finding I8 that shares C1's code, without regressing any existing behaviour.

**Architecture:** Three independent defects. (1) Shell messages are trusted without proof of origin and reports are not paired with the request that caused them: every message gets a per-session secret token (Task 1), and `CommandState` pairs reports with requests by counting them (Task 2). (2) The integration script writes non-ASCII text through the console code page: it escapes every byte outside printable ASCII (Task 3). (3) crossterm 0.28.1 joins the two halves of a surrogate pair without looking at key-down/key-up and loses the character: a one-guard patch on a vendored copy (Task 4).

**Tech Stack:** Rust 2021, tokio, crossterm 0.28.1, portable-pty 0.8, vt100 0.15, PowerShell 7 / Windows PowerShell 5.1 with PSReadLine 2.x.

**Spec:** `docs/reviews/2026-10-01-revisao-geral.md` (sections C1–C4, I8). Raw evidence per finding: `docs/reviews/2026-10-01-revisao-geral.json`.

## Global Constraints

- Windows only; PowerShell 7 and Windows PowerShell 5.1 must both keep working.
- stdout belongs to the user's terminal: no `println!`/`eprintln!` after raw mode is entered; logs only through `tracing` (file, `--verbose`).
- Indices in a report (`cursor`, `replacementIndex`, `replacementLength`) are UTF-16 code units.
- A Tab must never be lost and never be applied twice; with no usable report within 3 s Tab goes to PowerShell unchanged.
- The integration script must never print over the edited line, must not change `$?`/`$LASTEXITCODE` seen by the prompt, and must not use `-ExecutionPolicy Bypass`.
- No new crates.io dependency (Task 4 vendors an existing one, it does not add one).
- Gate after every task: `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/verify.ps1` → `[OK] GATE VERDE`.
- Mutation proofs: `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path <src file> -Anchor '<exact text>' -Replacement '<text>' -TestCommand '<one test>'`; exit 0 = test caught it.
- New end-to-end tests call `Terminal::quiet_session()` (Task 1 adds it) before typing anything, so they neither read nor write the user's real PSReadLine history.
- Commit messages: English, conventional (`fix(scope): ...`), ending with the two attribution lines used in this repository.

## Review Focus

1. A report that arrives while no Tab waits (forged or late) — expected: ignored, nothing typed. Pinned in Task 2 (`test_unrequested_report_is_dropped`, `test_late_answer_after_abandon_is_dropped`).
2. A program printing `ESC]6973;...` text (a file, curl output) — expected: no effect on shell-panel's state. Pinned in Task 1 (`test_message_without_the_session_token_is_ignored`, `test_forged_marker_in_output_has_no_effect`).
3. A request whose chord PSReadLine never answers (swallowed by a command that was starting) — expected: the next prompt works normally, not every later Tab timing out. Pinned in Task 2 (`test_new_line_forgets_requests_that_were_never_answered`).
4. Paths and lines with characters outside the console code page (CJK, emoji) — expected: reported and inserted intact. Pinned in Task 3.
5. Characters outside the BMP typed or pasted — expected: reach PowerShell once, not zero times and not twice. Pinned in Task 4.

## Execution order and file overlap

Tasks run strictly in order 1 → 2 → 3 → 4. Tasks 1 and 2 both edit `src/core/app.rs` and `src/shell/command_state.rs`; Tasks 1 and 3 both edit `assets/shellIntegration.ps1` and `tests/shell_report_test.rs`. Task 4 is independent but runs last so the gate it ends with covers everything.

---

### Task 1: Tag every shell message with a per-session secret token (C3)

#### Where this fits
First of four. It changes the wire format of the OSC 6973 protocol; Task 2 builds on `CommandState::new(token)` and on `Terminal::quiet_session()` added here.

#### What binds this task
- Threat model, decided: the token protects against **data printed by programs** in the session (a file, a download, a log). It does not and need not protect against code the user runs in the session — such code can already type into the console. So the token lives in a PowerShell global variable and is passed inside the `-EncodedCommand` script, never in an environment variable (child processes inherit the environment and could print it).
- Wire format, decided: `ESC ] 6973;<token>;RS;<cwd> BEL`, `ESC ] 6973;<token>;RE BEL`, `ESC ] 6973;<token>;CMP;<json> BEL`. A terminated message with the `6973;` prefix and a wrong or missing token is **consumed and ignored** (hidden from the output and not applied), the same as an unknown 6973 message today.
- Token, decided: 32 lowercase hex chars from `std::collections::hash_map::RandomState` (keyed from the OS random source). No new crate.
- An empty token never matches anything.

#### The measured evidence of the defect
Review run `wf_6a3690b0-aeb`, finding C3: a file containing `ESC]6973;RS;BEL ESC]6973;CMP;{...,"matches":[["echo INJECTED-FROM-FILE","a","Command",""]]} BEL` printed in a loop, user presses Tab → the line became `echo INJECTED-FROM-FILE ` and the user's `Get-ChildIt` was erased.

#### Files
- Modify: `src/shell/integration.rs` (token, script assembly)
- Modify: `src/shell/osc.rs:52-69` (`parse_osc_sequence` takes the token)
- Modify: `src/shell/command_state.rs` (holds the token; `new`)
- Modify: `src/shell/stream.rs:98-102` (passes the token)
- Modify: `src/pty/conpty.rs` (`ConPtySession` gains `pub token: String`, generated in `spawn`)
- Modify: `src/core/app.rs:114,154` (destructure `token`, `CommandState::new(token)`)
- Modify: `assets/shellIntegration.ps1:1-12` (header comment, `__SP-Send`)
- Modify: `tests/common/mod.rs` (add `quiet_session`)
- Modify: `tests/osc_test.rs`, `tests/stream_test.rs`, `tests/shell_report_test.rs`, `tests/e2e_pty_test.rs`
- Modify: `README.md` (How it works, Known limitations)

#### Interfaces
- Produces:
  - `pub fn new_session_token() -> String` in `shell::integration`
  - `pub fn script(token: &str) -> String` and `pub fn encoded_command(token: &str) -> String` in `shell::integration` (the old zero-argument `encoded_command()` is removed)
  - `pub fn parse_osc_sequence(payload: &str, token: &str) -> Option<OscEvent>`
  - `CommandState { pub token: String, .. }` and `pub fn CommandState::new(token: impl Into<String>) -> Self`
  - `ConPtySession { pub pair, pub child, pub token: String }`
  - `Terminal::quiet_session(&mut self)` in `tests/common/mod.rs`

- [ ] **Step 1: Add the history-safe session helper to the test harness**

Append inside `impl Terminal` in `tests/common/mod.rs`:

```rust
    /// Keeps this test session out of the user's real PSReadLine history: nothing typed here is
    /// saved, and no inline prediction from the real history is drawn on the line under test.
    /// Call it once the first prompt is on screen.
    pub fn quiet_session(&mut self) {
        // PredictionSource does not exist in PSReadLine 2.0 (Windows PowerShell 5.1): try/catch.
        // The marker is built by concatenation so the echoed command cannot satisfy the wait.
        self.send(
            b"Set-PSReadLineOption -HistorySaveStyle SaveNothing; \
try { Set-PSReadLineOption -PredictionSource None } catch {}; 'QUIET' + 'READY'\r",
        );
        assert!(
            self.wait_for_text("QUIETREADY", Duration::from_secs(15)),
            "quiet_session did not finish: {}",
            self.screen()
        );
    }
```

- [ ] **Step 2: Write the failing unit tests for the token**

Replace the body of `tests/osc_test.rs` tests that call `parse_osc_sequence` so every call passes `TOKEN`, and add the new tests. Top of file:

```rust
use shell_panel::shell::command_state::CommandState;
use shell_panel::shell::integration::{new_session_token, script};
use shell_panel::shell::osc::{parse_osc_sequence, unescape_value, OscEvent};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
```

Rewrite `test_unknown_messages_are_ignored`, `test_parse_readline_markers`, `test_parse_completion_report` with the token inserted after `6973;` (e.g. `"6973;0123456789abcdef0123456789abcdef;RE"`; build them with `format!("6973;{TOKEN};RE")`). Add:

```rust
#[test]
fn test_message_without_the_session_token_is_ignored() {
    // The protocol before this change, and what a printed file can contain.
    assert_eq!(parse_osc_sequence("6973;RE", TOKEN), None);
    assert_eq!(parse_osc_sequence("6973;RS;C:\\x5cp", TOKEN), None);
    // A different token.
    assert_eq!(
        parse_osc_sequence("6973;ffffffffffffffffffffffffffffffff;RE", TOKEN),
        None
    );
    // The token as a prefix of a longer one is not the token.
    assert_eq!(parse_osc_sequence(&format!("6973;{TOKEN}0;RE"), TOKEN), None);
    // An empty session token never matches, not even an empty field.
    assert_eq!(parse_osc_sequence("6973;;RE", ""), None);
    assert_eq!(
        parse_osc_sequence(&format!("6973;{TOKEN};RE"), TOKEN),
        Some(OscEvent::ReadLineEnded)
    );
}

#[test]
fn test_session_tokens_are_fresh_hex() {
    let a = new_session_token();
    let b = new_session_token();
    assert_eq!(a.len(), 32);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(a, b);
}

#[test]
fn test_script_carries_the_token_before_the_integration() {
    let s = script(TOKEN);
    let assignment = format!("$Global:__SP_Token = '{TOKEN}'");
    assert!(s.starts_with(&assignment), "script starts with: {:?}", &s[..80.min(s.len())]);
    assert!(s.contains("Set-PSReadLineKeyHandler"));
}
```

In `test_command_state_follows_readline_markers` replace `CommandState::default()` with `CommandState::new(TOKEN)`.

- [ ] **Step 3: Write the failing stream tests**

In `tests/stream_test.rs`, add `const TOKEN: &str = "0123456789abcdef0123456789abcdef";`, replace every `CommandState::default()` with `CommandState::new(TOKEN)`, and insert the token into every message literal: `b"hi\x1b]6973;RS;C:/p"` becomes `format!("hi\x1b]6973;{TOKEN};RS;C:/p").as_bytes()`, and so on for all seven tests. In `test_split_prefix_and_st_terminator` keep the split inside the prefix: first chunk `b"a\x1b]69"`, second `format!("73;{TOKEN};RE\x1b\\b")`. Add:

```rust
#[test]
fn test_forged_marker_in_output_has_no_effect() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::new(TOKEN);
    let mut residual = Vec::new();

    // What `Get-Content forged.txt` prints: the old wire format, no token.
    let out = ingest_pty_chunk(
        b"before\x1b]6973;RS;C:/evil\x07\x1b]6973;CMP;{\"line\":\"x\",\"cursor\":1}\x07after",
        &mut term,
        &mut state,
        &mut residual,
    );

    assert!(!state.reading_line, "a forged RS must not start a line");
    assert_eq!(state.cwd, "");
    assert_eq!(state.report, None);
    assert_eq!(out, b"beforeafter");
    assert!(residual.is_empty());
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -q --test osc_test --test stream_test`
Expected: compile errors — `parse_osc_sequence` takes 1 argument, `CommandState::new`, `new_session_token` and `script` not found.

- [ ] **Step 5: Implement the token**

`src/shell/integration.rs` — keep `SCRIPT` and `base64_encode`; replace `encoded_command` and add:

```rust
/// A secret that tags every message the integration script sends. Programs run in the session
/// print into the same stream, but their output cannot know this value, so it cannot forge a
/// ReadLine marker or a completion report. (Code the user runs in the session is not the threat:
/// it can type into the console anyway.)
pub fn new_session_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    // RandomState is keyed from the OS random source; two 64-bit hashes make 128 bits.
    (0..2u8)
        .map(|i| {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u8(i);
            format!("{:016x}", hasher.finish())
        })
        .collect()
}

/// The integration script for one session: the token assignment, then [`SCRIPT`].
pub fn script(token: &str) -> String {
    format!("$Global:__SP_Token = '{token}'\n{SCRIPT}")
}

/// [`script`] as `-EncodedCommand` expects it: base64 of its UTF-16LE bytes.
pub fn encoded_command(token: &str) -> String {
    let utf16: Vec<u8> = script(token)
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    base64_encode(&utf16)
}
```

`src/shell/osc.rs` — new signature and doc:

```rust
/// Parses an OSC 6973 payload sent by the integration script of this session.
///
/// - `6973;<token>;RS;<escaped_path>` -> `OscEvent::ReadLineStarted`
/// - `6973;<token>;RE` -> `OscEvent::ReadLineEnded`
/// - `6973;<token>;CMP;<escaped_json>` -> `OscEvent::Report`
///
/// A payload without this session's `token` is not ours (a program printed it) and yields `None`.
pub fn parse_osc_sequence(payload: &str, token: &str) -> Option<OscEvent> {
    if token.is_empty() {
        return None;
    }
    let body = payload
        .strip_prefix("6973;")?
        .strip_prefix(token)?
        .strip_prefix(';')?;
    // ... the existing RE / RS; / CMP; branches, unchanged, on `body` ...
}
```

`src/shell/command_state.rs` — add the field and constructor:

```rust
    /// Secret of this session; messages without it are ignored (see `new_session_token`).
    pub token: String,
```

```rust
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            ..Self::default()
        }
    }
```

`src/shell/stream.rs:99` — `parse_osc_sequence(payload, &command_state.token)`.

`src/pty/conpty.rs` — add `pub token: String` to `ConPtySession` (doc: "Secret tagging this session's shell messages."), and in `spawn`:

```rust
        let token = crate::shell::integration::new_session_token();
        // ...
        cmd.arg(crate::shell::integration::encoded_command(&token));
        // ...
        Ok(Self { pair, child, token })
```

`src/core/app.rs` — `let ConPtySession { pair, child, token } = ConPtySession::spawn(...)?;` and `let mut command_state = CommandState::new(token);`.

`assets/shellIntegration.ps1` — header and sender:

```powershell
# shell-panel integration for PowerShell with PSReadLine 2.x. Passed with -EncodedCommand, after a
# line that sets $Global:__SP_Token to this session's secret.
# Messages to shell-panel: ESC ] 6973;<token>;<payload> BEL with payloads RS;<cwd>, RE and
# CMP;<json>. Output of programs cannot know the token, so it cannot forge a message.
```

```powershell
function Global:__SP-Send([string]$payload) {
    [Console]::Write("$([char]0x1b)]6973;$($Global:__SP_Token);$payload$([char]0x07)")
}
```

- [ ] **Step 6: Update the shell tests to the new format**

`tests/shell_report_test.rs`:
- `use shell_panel::shell::integration::{base64_encode, script, SCRIPT};` (drop `encoded_command` if unused there; `test_encoded_command_fits_the_windows_command_line` calls `encoded_command(TOKEN)` — keep that import if so).
- Add `const TOKEN: &str = "0123456789abcdef0123456789abcdef";`.
- `shell_with_prelude`: `let script = format!("{prelude}\n{}", script(TOKEN));`.
- `assert_prediction_view`: wait for `format!("\x1b]6973;{TOKEN};RS;").as_bytes()`.
- `last_report(raw, token)`: marker `format!("\x1b]6973;{token};CMP;")`, and `parse_osc_sequence(..., token)`.
- `test_session_reports_readline_state_line_cursor_and_completions`: `let ConPtySession { pair, child, token } = ...`; wait for `format!("\x1b]6973;{token};RS;")` and `format!("\x1b]6973;{token};RE\x07")`; `last_report(&t.raw, &token)`.

`tests/e2e_pty_test.rs::test_embedded_script_defines_the_protocol`: keep the markers check on `SCRIPT`, and add `"$($Global:__SP_Token)"` to the list.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -q --test osc_test --test stream_test --test shell_report_test --test e2e_pty_test --test e2e_binary_test -- --test-threads=1`
Expected: all pass. The e2e binary tests prove the real script and the real reactor still agree on the format.

- [ ] **Step 8: Mutation proofs (paste each output in the report)**

```
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path src/shell/osc.rs -Anchor '.strip_prefix(token)?' -Replacement '.strip_prefix("")?' -TestCommand 'cargo test -q --test osc_test test_message_without_the_session_token_is_ignored'
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path src/shell/osc.rs -Anchor '    if token.is_empty() {
        return None;
    }' -Replacement '' -TestCommand 'cargo test -q --test osc_test test_message_without_the_session_token_is_ignored'
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path src/shell/stream.rs -Anchor 'parse_osc_sequence(payload, &command_state.token)' -Replacement 'parse_osc_sequence(&payload.replacen("6973;", &format!("6973;{};", command_state.token), 1), &command_state.token)' -TestCommand 'cargo test -q --test stream_test test_forged_marker_in_output_has_no_effect'
```
Expected: exit 0 for all three (the third makes every 6973 message look signed, which is what a forged one needs). If one is INCONCLUSIVO because the anchor whitespace differs, copy the exact lines from the file and rerun.

- [ ] **Step 9: Docs**

`README.md`: in "How it works", after the sentence about binding Ctrl+Alt+Shift+F12, add: "Every message the script sends carries a secret generated for the session, so text printed by a program cannot pose as a report." In "Known limitations", change "Shell messages longer than 1 MiB, or containing raw control bytes, are treated as ordinary output" to keep the size/control-byte sentence and add "Messages without the session's secret are ignored."

- [ ] **Step 10: Gate and commit**

Run: `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/verify.ps1` → `[OK] GATE VERDE`.

```bash
git add src/shell/integration.rs src/shell/osc.rs src/shell/command_state.rs src/shell/stream.rs src/pty/conpty.rs src/core/app.rs assets/shellIntegration.ps1 tests/common/mod.rs tests/osc_test.rs tests/stream_test.rs tests/shell_report_test.rs tests/e2e_pty_test.rs README.md
git commit -m "fix(shell): tag shell messages with a per-session secret so program output cannot forge them"
```

#### Traps already paid for that apply here
- `wait_for_text` matches the echo of what was typed: every marker you wait for must be built by concatenation (`'QUIET' + 'READY'`).
- e2e tests flake under load (review lesson): run them with `--test-threads=1` and nothing else heavy running. A failure of `test_completion_uses_the_real_line_and_the_real_session` at line ~95 also happened on the untouched base; rerun once before concluding.
- Do not put the token in `cmd.env(...)`.

#### Execution rules
Work only on branch `fix/revisao-2026-10-01` in the main checkout. Do not touch files outside the list. No `git stash/reset/checkout/clean` on files you did not create. Do not add dependencies.

#### Report contract
Status; commit SHA and subject; RED output of Step 4 (the compile errors); GREEN output of Step 7 (the `test result:` lines); the three mutation outputs from Step 8 with exit codes; gate output; `git status --porcelain` (empty).

---

### Task 2: Pair each report with the Tab that requested it (C1, I8)

#### Where this fits
Second. Builds on `CommandState::new(token)` and `Terminal::quiet_session()` from Task 1.

#### What binds this task
- Decided mechanism: count requests. `CommandState` keeps `outstanding_reports: u32` (chords written, reports not yet arrived) and `awaiting_report: bool` (a Tab is waiting right now). A report decrements the count (saturating) and is kept **only** when the count reaches 0 **and** a Tab is waiting. PSReadLine answers chords strictly in order, so the report that brings the count to 0 is the answer to the latest request.
- `ReadLineStarted` resets the count to 0: a chord that was never answered (swallowed by a command that was starting) must not make every later Tab time out. Reports of the previous line always precede the new `RS` in the stream.
- The timeout and every key press abandon the wait (`awaiting_report = false`) but do **not** reset the count: the late report is still on its way and must be recognised as stale when it lands.
- Keep the existing `generation` check on engine results and the existing `report_deadline.take().is_some()` check (now redundant, kept as a guard).

#### The measured evidence of the defect
Finding C1 (reproduced by lens and refuter): completer sleeping 4 s; `zzf z` + Tab, Tab again 3.5 s later → final line `zzf zzalphazalpha` (completion inserted twice). Completer sleeping 2.5 s; Tab, `q` 0.5 s later, Tab 0.5 s later → `zzf zzalphaqzalpha`. Finding I8: no test fails when a report arrives with no Tab pending.

#### Files
- Modify: `src/shell/command_state.rs`
- Modify: `src/core/app.rs:226-282` (key branch, timeout branch) and `:303-354` (`handle_key` takes `&mut CommandState`)
- Modify: `tests/osc_test.rs` (unit tests)
- Create: `tests/e2e_report_order_test.rs`

#### Interfaces
- Consumes: `CommandState::new(token)`, `Terminal::quiet_session()` (Task 1).
- Produces: `CommandState::request_report(&mut self)`, `CommandState::abandon_report(&mut self)`; `handle_osc` keeps its signature.

- [ ] **Step 1: Write the failing unit tests**

Append to `tests/osc_test.rs`:

```rust
fn report_of(line: &str) -> ShellReport {
    ShellReport {
        line: line.into(),
        cursor: line.encode_utf16().count(),
        replacement_index: 0,
        replacement_length: 0,
        matches: vec![],
    }
}

fn reading() -> CommandState {
    let mut state = CommandState::new(TOKEN);
    state.handle_osc(OscEvent::ReadLineStarted { cwd: None });
    state
}

#[test]
fn test_unrequested_report_is_dropped() {
    let mut state = reading();
    state.handle_osc(OscEvent::Report(report_of("x")));
    assert_eq!(state.report, None);
}

#[test]
fn test_only_the_answer_to_the_last_request_is_kept() {
    let mut state = reading();
    state.request_report(); // Tab 1
    state.abandon_report(); // a key, or the 3 s timeout
    state.request_report(); // Tab 2
    state.handle_osc(OscEvent::Report(report_of("old")));
    assert_eq!(state.report, None, "the answer to Tab 1 was taken for Tab 2");
    state.handle_osc(OscEvent::Report(report_of("new")));
    assert_eq!(state.report.take().map(|r| r.line), Some("new".to_string()));
}

#[test]
fn test_late_answer_after_abandon_is_dropped() {
    let mut state = reading();
    state.request_report();
    state.abandon_report();
    state.handle_osc(OscEvent::Report(report_of("late")));
    assert_eq!(state.report, None);
}

#[test]
fn test_answer_is_kept_once() {
    let mut state = reading();
    state.request_report();
    state.handle_osc(OscEvent::Report(report_of("a")));
    assert!(state.report.take().is_some());
    // A duplicate (or forged) second report finds no Tab waiting.
    state.handle_osc(OscEvent::Report(report_of("b")));
    assert_eq!(state.report, None);
}

#[test]
fn test_new_line_forgets_requests_that_were_never_answered() {
    let mut state = reading();
    state.request_report(); // chord swallowed: no report will ever come
    state.abandon_report();
    state.handle_osc(OscEvent::ReadLineStarted { cwd: None });
    state.request_report();
    state.handle_osc(OscEvent::Report(report_of("fresh")));
    assert_eq!(state.report.take().map(|r| r.line), Some("fresh".to_string()));
}
```

Also update `test_command_state_follows_readline_markers`: call `state.request_report();` right before `state.handle_osc(OscEvent::Report(report.clone()));`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q --test osc_test`
Expected: compile error `no method named request_report`.

- [ ] **Step 3: Implement the pairing in `CommandState`**

```rust
/// What shell-panel knows about the shell from its integration messages.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandState {
    /// Current filesystem location of the shell.
    pub cwd: String,
    /// True while PSReadLine is reading a line: the only time the report request is answered.
    pub reading_line: bool,
    /// The report answering the Tab that waits now, taken by the reactor loop.
    pub report: Option<ShellReport>,
    /// Secret of this session; messages without it are ignored (see `new_session_token`).
    pub token: String,
    /// Report requests written to the shell whose report has not arrived yet.
    outstanding_reports: u32,
    /// True while a Tab waits for the answer to the latest request.
    awaiting_report: bool,
}

impl CommandState {
    pub fn new(token: impl Into<String>) -> Self { /* from Task 1 */ }

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
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -q --test osc_test`
Expected: all pass.

- [ ] **Step 5: Wire it into the reactor**

`src/core/app.rs`:
- Timeout branch (`_ = tokio::time::sleep_until(deadline) ...`): add `command_state.abandon_report();` next to `report_deadline = None;`.
- Key branch (`Event::Key(key_event) if ...`): add `command_state.abandon_report();` right after `report_deadline = None;`, and pass `&mut command_state` to `handle_key`.
- `handle_key`: parameter `command_state: &mut CommandState`; in the `if completes` branch call `command_state.request_report();` immediately after `write_to_pty(pty_writer, REPORT_REQUEST_KEY);`.
- Update the comment at the report branch from "Only the answer to the latest Tab is wanted." to "`CommandState` keeps a report only when it answers the Tab that waits now."

- [ ] **Step 6: Write the end-to-end tests (they fail on the Task 1 commit)**

Create `tests/e2e_report_order_test.rs`:

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

/// The screen row holding the line being edited.
fn edited_line(term: &Terminal) -> String {
    term.screen()
        .lines()
        .filter(|l| l.contains("> zzf"))
        .last()
        .unwrap_or("")
        .trim_end()
        .to_string()
}

/// A session where `zzf <Tab>` runs a completer that sleeps `delay_ms` and offers `zzalpha`
/// when it matches the word being completed. Ends with `zzf z` typed.
fn session_with_slow_completer(tag: &str, delay_ms: u32) -> Terminal {
    let mut term = Terminal::shell_panel(&temp_dir(tag));
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();
    let setup = format!(
        "function zzf {{ param($p) }}; Register-ArgumentCompleter -CommandName zzf -ParameterName p \
-ScriptBlock {{ param($c, $p, $w) Start-Sleep -Milliseconds {delay_ms}; 'zzalpha' | Where-Object {{ $_ -like \"$w*\" }} }}; \
'SETUP' + 'DONE'\r"
    );
    term.send(setup.as_bytes());
    assert!(term.wait_for_text("SETUPDONE", STEP), "screen: {}", term.screen());
    term.send(b"zzf z");
    assert!(
        term.wait_until(STEP, |t| edited_line(t).ends_with("> zzf z")),
        "screen: {}",
        term.screen()
    );
    term
}

/// Lets queued keys, completers and reports finish, pumping output meanwhile.
fn settle(term: &mut Terminal, secs: u64) {
    term.wait_until(Duration::from_secs(secs), |_| false);
}

#[test]
fn test_late_report_of_a_timed_out_tab_does_not_answer_the_next_tab() {
    // Tab 1 times out at 3 s (its `\t` goes to PowerShell), Tab 2 at 3.5 s; Tab 1's report
    // lands at 4 s while Tab 2 waits.
    let mut term = session_with_slow_completer("late", 4000);
    term.send(b"\t");
    std::thread::sleep(Duration::from_millis(3500));
    term.send(b"\t");
    assert!(
        term.wait_until(Duration::from_secs(25), |t| edited_line(t).contains("zzalpha")),
        "screen: {}",
        term.screen()
    );
    settle(&mut term, 12);
    let line = edited_line(&term);
    assert!(line.ends_with("> zzf zzalpha"), "line: {line:?}");
}

#[test]
fn test_report_of_the_first_tab_does_not_answer_a_tab_typed_after_more_text() {
    // Tab, `q` (Tab 1 is handed to PowerShell before it), Tab: the first report describes
    // `zzf z` and must not be applied to `zzf zzalphaq`.
    let mut term = session_with_slow_completer("typed", 2500);
    term.send(b"\t");
    std::thread::sleep(Duration::from_millis(500));
    term.send(b"q");
    std::thread::sleep(Duration::from_millis(500));
    term.send(b"\t");
    assert!(
        term.wait_until(Duration::from_secs(25), |t| edited_line(t).contains("zzalphaq")),
        "screen: {}",
        term.screen()
    );
    settle(&mut term, 12);
    let line = edited_line(&term);
    assert!(line.ends_with("> zzf zzalphaq"), "line: {line:?}");
}
```

- [ ] **Step 7: Prove the e2e tests fail without the fix (RED evidence)**

Do not get the RED run with `git stash` or `git checkout` (forbidden by the execution rules). Use the mutation tool, which restores byte for byte:

```
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path src/shell/command_state.rs -Anchor 'if self.outstanding_reports == 0 && self.awaiting_report {' -Replacement 'if self.awaiting_report {' -TestCommand 'cargo test -q --test e2e_report_order_test -- --test-threads=1'
```
Expected: exit 0 (MORTA) — at least one of the two e2e tests fails with a `line:` showing `zzalphazalpha` or `zzalphaqzalpha`. Then run the same mutation against the unit test:
```
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path src/shell/command_state.rs -Anchor 'if self.outstanding_reports == 0 && self.awaiting_report {' -Replacement 'if self.awaiting_report {' -TestCommand 'cargo test -q --test osc_test test_only_the_answer_to_the_last_request_is_kept'
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path src/shell/command_state.rs -Anchor '                self.outstanding_reports = 0;' -Replacement '' -TestCommand 'cargo test -q --test osc_test test_new_line_forgets_requests_that_were_never_answered'
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path src/core/app.rs -Anchor '                    report_deadline = None;
                    tab_pending = false;
                    command_state.abandon_report();' -Replacement '                    report_deadline = None;
                    tab_pending = false;' -TestCommand 'cargo test -q --test e2e_report_order_test test_late_report -- --test-threads=1'
```
(Adjust the last anchor to the exact lines you wrote in the timeout branch.) Expected: exit 0 for all four. If the e2e mutation is INCONCLUSIVO or SURVIVES, first run `cargo test -q --test e2e_report_order_test -- --test-threads=1` on the unmutated code twice; report both runs.

- [ ] **Step 8: Gate and commit**

Run: `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/verify.ps1` → `[OK] GATE VERDE`.

```bash
git add src/shell/command_state.rs src/core/app.rs tests/osc_test.rs tests/e2e_report_order_test.rs
git commit -m "fix(core): apply a completion report only when it answers the Tab that waits now"
```

#### Traps already paid for that apply here
- The completer must filter by the word (`Where-Object { $_ -like "$w*" }`); without it every report offers `zzalpha` and the expected final lines change.
- `edited_line` must look for `> zzf` (prompt + command) — the setup line also contains `zzf` but not after `> `.
- Inline predictions would append gray text to the line; `quiet_session()` turns them off. Do not remove that call.
- Never use `git stash`/`checkout` to get a RED run: use `mutate.ps1`.

#### Execution rules
Same as Task 1. Only the four files listed.

#### Report contract
Status; SHA; RED (Step 2 compile error); GREEN (Step 4 and the e2e run); the four mutation outputs with exit codes; gate; `git status --porcelain`.

---

### Task 3: Escape every non-ASCII character in shell messages (C2)

#### Where this fits
Third. Touches only the integration script and one test file.

#### What binds this task
- Decided: `__SP-Escape` emits **only printable ASCII**. Every other UTF-16 unit is written as the `\xHH` escapes of its UTF-8 bytes; a surrogate pair is matched whole and becomes one 4-byte sequence. Then the console code page (850, 437, 1252, 65001) cannot change a message. The Rust side already decodes `\xHH` byte sequences as UTF-8 (`unescape_value`), so `src/` does not change.
- Do not call `chcp` or change `[Console]::OutputEncoding`: that would change how every program in the user's session writes its output.
- A report's size grows (up to 16 chars per non-ASCII char); 100 matches × 120-char tooltips stay far below `MAX_MESSAGE_BYTES` (1 MiB).

#### The measured evidence of the defect
Finding C2: inside the session `[Console]::OutputEncoding.CodePage` = 850; `echo '日本😀' > $sp_rep_` + report request → `line="echo '????' > $sp_rep_"`; in a directory holding only `日本.txt`, `Get-Item .\` + Tab → `Get-Item .\??.txt` (a wildcard).

#### Files
- Modify: `assets/shellIntegration.ps1:4-8`
- Modify: `tests/shell_report_test.rs` (two tests)

#### Interfaces
- Consumes: `script(token)`, `ConPtySession { token, .. }`, `last_report(raw, token)` (Task 1).

- [ ] **Step 1: Write the failing tests**

Append to `tests/shell_report_test.rs` (adjust imports: `use std::path::Path;`):

```rust
/// The integration script in a real shell started in `dir`, with the test token.
fn shell_in(dir: &Path) -> Terminal {
    let utf16: Vec<u8> = script(TOKEN)
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut cmd = CommandBuilder::new(detect_shell(None).executable_name());
    for arg in ["-NoLogo", "-NoProfile", "-NoExit", "-EncodedCommand"] {
        cmd.arg(arg);
    }
    cmd.arg(base64_encode(&utf16));
    cmd.cwd(dir);
    cmd.env_remove("SHELL_PANEL_SESSION");
    Terminal::spawn(cmd)
}

fn last_cwd(raw: &[u8]) -> Option<String> {
    let marker = format!("\x1b]6973;{TOKEN};RS;");
    let marker = marker.as_bytes();
    let start = raw.windows(marker.len()).rposition(|w| w == marker)?;
    let end = start + raw[start..].iter().position(|&b| b == 0x07)?;
    match parse_osc_sequence(std::str::from_utf8(&raw[start + 2..end]).ok()?, TOKEN)? {
        OscEvent::ReadLineStarted { cwd } => cwd,
        _ => None,
    }
}

#[test]
fn test_text_outside_the_console_code_page_survives_the_report() {
    let dir = std::env::temp_dir().join(format!("sp_cp_日本😀_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut term = shell_in(&dir);
    assert!(
        term.wait_until(Duration::from_secs(40), |t| last_cwd(&t.raw).is_some()),
        "no ReadLine marker"
    );
    // The location is reported intact, not as `sp_cp_???_`.
    assert_eq!(last_cwd(&term.raw).as_deref(), dir.to_str());

    let line = "echo '日本😀ação' > $sp_cp_";
    term.send(line.as_bytes());
    assert!(term.wait_for_text("$sp_cp_", Duration::from_secs(15)));
    term.send(REPORT_REQUEST_KEY);
    assert!(
        term.wait_until(Duration::from_secs(20), |t| last_report(&t.raw, TOKEN).is_some()),
        "no report"
    );
    let Some(OscEvent::Report(report)) = last_report(&term.raw, TOKEN) else {
        unreachable!()
    };
    assert_eq!(report.line, line);
    // 😀 is two UTF-16 units: the cursor counts them both.
    assert_eq!(report.cursor, line.encode_utf16().count());
    assert_eq!(report.text_before_cursor(), Some(line));
}

#[test]
fn test_completion_of_a_name_outside_the_code_page_is_not_a_wildcard() {
    let dir = std::env::temp_dir().join(format!("sp_cpname_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("日本.txt"), b"x").unwrap();
    let mut term = shell_in(&dir);
    assert!(term.wait_until(Duration::from_secs(40), |t| last_cwd(&t.raw).is_some()));
    term.send(b"Get-Item .\\");
    assert!(term.wait_for_text("Get-Item .\\", Duration::from_secs(15)));
    term.send(REPORT_REQUEST_KEY);
    assert!(term.wait_until(Duration::from_secs(20), |t| last_report(&t.raw, TOKEN).is_some()));
    let Some(OscEvent::Report(report)) = last_report(&term.raw, TOKEN) else {
        unreachable!()
    };
    assert!(
        report.matches.iter().any(|m| m.0 == ".\\日本.txt"),
        "matches: {:?}",
        report.matches
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q --test shell_report_test text_outside -- --test-threads=1` and `... name_outside ...`
Expected: FAIL — cwd ends in `sp_cp_???_…` / line `echo '???ação'…` (the `ç`/`ã` exist in 850 and survive; the CJK and emoji do not), and the match is `.\??.txt`. Paste the assertion output. If the console code page in your session is 65001 (UTF-8 beta option on) the RED cannot be reproduced: report that, with the output of `pwsh -NoProfile -Command "[Console]::OutputEncoding.CodePage"`, and continue.

- [ ] **Step 3: Implement**

Replace `__SP-Escape` in `assets/shellIntegration.ps1`:

```powershell
# Everything outside printable ASCII, and `\` and `;`, goes as \xHH escapes of its UTF-8 bytes.
# [Console]::Write encodes with the console code page (OEM 850, 437, ...), which turns every
# character it cannot represent into '?'; ASCII is the same in all of them. A surrogate pair is
# matched whole so it becomes one 4-byte UTF-8 sequence.
function Global:__SP-Escape([string]$value) {
    [regex]::Replace($value, '[\uD800-\uDBFF][\uDC00-\uDFFF]|[^\x20-\x3a\x3c-\x5b\x5d-\x7e]', { param($match)
        -join ([System.Text.Encoding]::UTF8.GetBytes($match.Value) | ForEach-Object { '\x{0:x2}' -f $_ })
    })
}
```

(`\x3b` is `;` and `\x5c` is `\`; both fall outside the kept ranges, so they are escaped as before.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -q --test shell_report_test --test osc_test --test e2e_binary_test -- --test-threads=1`
Expected: all pass, including the existing `ação` tests.

- [ ] **Step 5: Mutation proof**

The script is not Rust; mutate it with the same tool (it accepts any production file):
```
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path assets/shellIntegration.ps1 -Anchor '[^\x20-\x3a\x3c-\x5b\x5d-\x7e]' -Replacement '[\x00-\x1f\x7f\\;]' -TestCommand 'cargo test -q --test shell_report_test text_outside -- --test-threads=1'
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path assets/shellIntegration.ps1 -Anchor '[\uD800-\uDBFF][\uDC00-\uDFFF]|' -Replacement '' -TestCommand 'cargo test -q --test shell_report_test text_outside -- --test-threads=1'
```
Expected: exit 0 for both (the second proves the surrogate-pair alternative matters: without it each half becomes `EF BF BD` and the emoji turns into two U+FFFD).

- [ ] **Step 6: Gate and commit**

```bash
git add assets/shellIntegration.ps1 tests/shell_report_test.rs
git commit -m "fix(shell): send shell messages as pure ASCII so the console code page cannot corrupt them"
```

#### Traps already paid for that apply here
- `include_str!` embeds the script at compile time; the mutation tool rebuilds, so the mutation reaches the binary. Do not test the script by running it outside cargo.
- In PowerShell single-quoted strings `\u` is literal and reaches the .NET regex engine, which understands it. Do not switch to double quotes.

#### Execution rules / Report contract
As in Task 1, files limited to the two listed. Report RED, GREEN, both mutation outputs, gate, `git status --porcelain`.

---

### Task 4: Keep characters outside the BMP typed into shell-panel (C4)

#### Where this fits
Last. Independent of the protocol work.

#### What binds this task
- Root cause, read in the source (`~/.cargo/registry/src/*/crossterm-0.28.1/src/event/sys/windows/parse.rs:54-58`): `handle_key_event` passes every `WindowsKeyEvent::Surrogate` to `handle_surrogate` whether the record is a key down or a key up. The console sends down and up records for each half; pairing them in arrival order joins high+high or low+low and drops the character (or, in another order, emits it twice). Our filter `kind != Release` cannot help: the surrogate path builds the event with `KeyEvent::new`, always `Press`.
- Decided fix: vendor crossterm 0.28.1 into `vendor/crossterm` and patch one guard: a surrogate from a key-up record is ignored, **except** the Alt-code case (`VK_MENU` release carrying `u_char`), which crossterm deliberately reports on release. Use it through `[patch.crates-io]`. Upgrading crossterm is not a fix: 0.29.0 has the same code.
- Declare `[workspace]` with `exclude = ["vendor"]` in `Cargo.toml` so the vendored crate is not formatted or linted as ours.
- If the e2e test still fails after the patch, the hypothesis is wrong: stop and report BLOCKED with the output. Do not try other fixes.

#### The measured evidence of the defect
Finding C4: through shell-panel `'X😀Y'.Length` printed `2` and the screen showed `'XY'.Length`; through plain `pwsh -NoProfile` in the same ConPTY harness it printed `4`.

#### Files
- Create: `vendor/crossterm/` (copy of the registry crate, plus the patch)
- Modify: `Cargo.toml`, `Cargo.lock`
- Create: `tests/e2e_unicode_input_test.rs`
- Modify: `README.md` (one line under Build and run)

- [ ] **Step 1: Write the failing end-to-end test**

```rust
mod common;

use std::time::Duration;

use common::Terminal;

const START: Duration = Duration::from_secs(40);
const STEP: Duration = Duration::from_secs(15);

/// The line printed right above `marker` (the answer to the command before it).
fn answer_before(term: &Terminal, marker: &str) -> Option<String> {
    let screen = term.screen();
    let lines: Vec<&str> = screen.lines().map(str::trim_end).collect();
    let at = lines.iter().rposition(|l| *l == marker)?;
    at.checked_sub(1).map(|i| lines[i].to_string())
}

#[test]
fn test_characters_outside_the_bmp_reach_powershell_once() {
    let dir = std::env::temp_dir().join(format!("sp_e2e_astral_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();

    // 😀 and 🐛 are two UTF-16 units each; 日 is one. Expected length: 1+2+1+2+1 = 7.
    term.send("'X😀日🐛Y'.Length; 'LEN' + 'ONE'\r".as_bytes());
    assert!(term.wait_for_text("LENONE", STEP), "screen: {}", term.screen());
    assert_eq!(
        answer_before(&term, "LENONE").as_deref(),
        Some("7"),
        "screen: {}",
        term.screen()
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -q --test e2e_unicode_input_test -- --test-threads=1`
Expected: FAIL with a length other than `7` (the review saw the emoji dropped, which gives `3`; a duplicated emoji would give more). Paste it.

- [ ] **Step 3: Vendor crossterm**

```powershell
$src = Get-ChildItem "$env:USERPROFILE\.cargo\registry\src\*\crossterm-0.28.1" -Directory | Select-Object -First 1
New-Item -ItemType Directory -Force vendor | Out-Null
Copy-Item -Recurse $src.FullName vendor\crossterm
Remove-Item vendor\crossterm\.cargo_vcs_info.json -ErrorAction Ignore
```

Append to `Cargo.toml`:

```toml
[workspace]
# Vendored dependencies are not ours to format or lint.
exclude = ["vendor"]

[patch.crates-io]
# crossterm 0.28.1 with one fix: surrogate halves are paired only from key-down records (see
# vendor/crossterm/SHELL-PANEL-PATCH.md). Drop this once upstream releases the fix.
crossterm = { path = "vendor/crossterm" }
```

- [ ] **Step 4: Patch the guard**

In `vendor/crossterm/src/event/sys/windows/parse.rs`, `handle_key_event`, replace the `WindowsKeyEvent::Surrogate` arm with:

```rust
        WindowsKeyEvent::Surrogate(new_surrogate) => {
            // shell-panel patch: the console sends a key down and a key up for each half of a
            // surrogate pair. Pairing halves regardless of direction joined two high (or two
            // low) halves and lost the character. Only key downs carry text; an Alt code is the
            // exception, reported on the Alt release.
            let is_alt_code_release = key_event.virtual_key_code as i32 == VK_MENU;
            if !key_event.key_down && !is_alt_code_release {
                return None;
            }
            let ch = handle_surrogate(surrogate_buffer, new_surrogate)?;
            let modifiers = KeyModifiers::from(&key_event.control_key_state);
            let key_event = KeyEvent::new(KeyCode::Char(ch), modifiers);
            Some(Event::Key(key_event))
        }
```

Create `vendor/crossterm/SHELL-PANEL-PATCH.md` with three lines: the upstream version (0.28.1), the file and function patched, and the reason (copy the code comment).

- [ ] **Step 5: Build and run the test**

Run: `cargo build -q` then `cargo test -q --test e2e_unicode_input_test -- --test-threads=1`
Expected: PASS. `Cargo.lock` now shows crossterm without a registry `source` line. If it still fails: report BLOCKED with the test output and the `cargo tree -i crossterm` output (to prove the patch is the crate in use).

- [ ] **Step 6: Mutation proof on the guard**

```
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path vendor/crossterm/src/event/sys/windows/parse.rs -Anchor 'if !key_event.key_down && !is_alt_code_release {' -Replacement 'if false && !key_event.key_down && !is_alt_code_release {' -TestCommand 'cargo test -q --test e2e_unicode_input_test -- --test-threads=1'
```
Expected: exit 0.

- [ ] **Step 7: Docs, gate, commit**

`README.md`, under "Build and run", add: "`vendor/crossterm` is crossterm 0.28.1 with one fix for characters outside the BMP (see `vendor/crossterm/SHELL-PANEL-PATCH.md`)."

Run the gate. Then:

```bash
git add vendor/crossterm Cargo.toml Cargo.lock tests/e2e_unicode_input_test.rs README.md
git commit -m "fix(io): keep characters outside the BMP typed into shell-panel"
```

#### Traps already paid for that apply here
- `verify.ps1` runs `cargo fmt --check`: if it reports diffs under `vendor/`, the `[workspace] exclude` is missing or wrong — fix that, never run `cargo fmt` over the vendored code.
- The e2e harness types UTF-8 into the ConPTY; conhost turns it into key records for shell-panel, the same path as a real terminal. A unit test of the vendored function is not a substitute.

#### Execution rules / Report contract
As in Task 1. Report RED, GREEN, the mutation output, `cargo tree -i crossterm | Select-Object -First 3`, gate, `git status --porcelain`.

---

## After the critical tasks: the Important findings (next plan)

These are not part of this plan. They will get their own plan once Tasks 1–4 are merged, because several touch the same code (`aggregate.rs`, `lexer.rs`, `key_event.rs`) and their designs depend on the protocol settled here.

| ID | Finding (report section) | Intended approach |
|----|--------------------------|-------------------|
| I1 | Tab mid-word duplicates the rest of the word for spec/carapace/zoxide suggestions | `plan_replacement` deletes the rest of the active token after the cursor for external suggestions |
| I2 | Lexer ignores line breaks on continuation lines | treat `\n`/`\r` as whitespace and command separator in `lex_command_line` and `active_token_raw` |
| I3 | Carapace values inserted unquoted | quote with the same rules as zoxide paths |
| I4 | Alt+Enter runs the line | encode Alt+Enter as a win32-input-mode record like Shift/Ctrl+Enter |
| I5 | Non-ASCII hex color panics at start-up | check `is_ascii_hexdigit` before slicing in `color.rs` |
| I6 | Tooltips/descriptions drawn raw (escape sequences reach the terminal) | strip C0/C1/DEL from display text before rendering |
| I7, I9–I19 | Documented rules without a test that can fail | one test per rule, each with a mutation proof |
| M2 | e2e tests write to the user's real PSReadLine history | call `quiet_session()` in every existing e2e test |
