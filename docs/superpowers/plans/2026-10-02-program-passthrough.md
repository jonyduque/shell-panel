# Program Passthrough Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** While PSReadLine is not reading a line, pass the host terminal's input bytes to ConPTY unchanged. This fixes the slow start with `[?65;4;6;18;22;52c` typed at the first prompt, and lets running programs receive mouse, focus, paste, query answers and every key.

**Architecture:** Two input modes that follow `CommandState::reading_line`. In prompt mode nothing changes. In program mode, which also covers start-up, the stdin console has `ENABLE_VIRTUAL_TERMINAL_INPUT` set, and every key event's char is written to the PTY raw. A second guard in the vendored crossterm stops ESC and other C0 controls from being dropped in that mode. The console mode is restored on exit and on panic.

**Tech Stack:** Rust 2021, crossterm 0.28.1 (vendored), crossterm_winapi 0.9 (already in `Cargo.lock`), portable-pty, PowerShell 7 / 5.1.

**Spec:** `docs/superpowers/specs/2026-10-02-program-passthrough-design.md` (approved by the user).

## Global Constraints

- Prompt mode behaviour is unchanged: every existing e2e test stays green (Tab, dropdown, Enter, withheld Tab, 3 s fallback, chord never reaches a program, resize, DA1-free paths).
- The user's console is left as it was found: the `ENABLE_VIRTUAL_TERMINAL_INPUT` bit (0x0200) is cleared on normal exit, on error exit and in the panic hook.
- `crossterm_winapi` becomes a direct dependency at the version already in `Cargo.lock` (0.9). No other dependency is added.
- No raw control bytes in source files (write `\x1b`, `\t`).
- Gate: `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/verify.ps1` → `[OK] GATE VERDE`.
- e2e tests: `-- --test-threads=1`, `term.quiet_session()` first, markers built by concatenation; rerun once on failure and report both runs.
- Never run install.ps1/uninstall.ps1. Never touch the user's install, PATH, PSReadLine history or Windows Terminal fragments.
- Forbidden: `git stash/reset/checkout/restore/clean` on files you did not create.

## Review Focus

1. A terminal that answers ConPTY's DA1 query at start-up: nothing must be typed at the first prompt. Pinned by `test_terminal_answers_at_start_up_are_not_typed`.
2. A key that today is dropped (F13) or arrives with an ESC prefix (arrows) while a program reads keys: the program must get it. Pinned by `test_program_receives_keys_shell_panel_cannot_encode` and `test_program_receives_escape_sequences_intact`.
3. Resizing while a program runs, with the console in VT input mode: the shell must see the new size. Pinned by `test_resize_reaches_a_running_program`.
4. Exiting shell-panel: the console must be left without the VT input bit. Pinned by `test_console_input_mode_is_restored_after_exit`.
5. Switching back to prompt mode: Tab completion and the dropdown must work again after a program ends. Pinned by the existing e2e suite, which runs commands before completing.

---

### Task 1: Two input modes

**Files:**
- Modify: `vendor/crossterm/src/event/sys/windows/parse.rs` (the `0x00..=0x1f` arm in `parse_key_event_record`), `vendor/crossterm/SHELL-PANEL-PATCH.md`
- Create: `src/io/console_mode.rs`; Modify: `src/io/mod.rs`
- Modify: `Cargo.toml` (`crossterm_winapi = "0.9"` under `[dependencies]`), `Cargo.lock` (root dependency edge only)
- Modify: `src/core/app.rs` (mode switching, program-mode key path), `src/io/raw_mode.rs` (restore), `src/main.rs` (panic hook)
- Modify: `README.md` (How it works; Known limitations)
- Test: `tests/io_test.rs` (pure flag helper), `tests/e2e_binary_test.rs` (five e2e tests)

**Interfaces:**
- Produces: `pub fn with_vt_input(mode: u32, enabled: bool) -> u32` (pure), `pub fn set_vt_input(enabled: bool) -> std::io::Result<()>` and `pub const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200` in `shell_panel::io::console_mode`.

#### The measured evidence (from the spec)
In classic mode the DA1 answer `ESC[?65;4;6c` arrives as the keys `[ ? 6 5 ; 4 ; 6 c` (ESC dropped), and F13 is dropped. With VT input on, every byte arrives in order except ESC, which crossterm drops because the record's virtual-key code is 0 and `get_char_for_key` finds nothing for it.

#### Decisions
- **crossterm patch #2:** in the `0x00..=0x1f` arm, when `key_event.virtual_key_code == 0`, return `Some(KeyCode::Char(char::from(utf16 as u8)))` (the control char itself); otherwise keep `get_char_for_key`. Comment: `// shell-panel patch #2: with ENABLE_VIRTUAL_TERMINAL_INPUT the console delivers the host's bytes as key records with virtual-key code 0; a C0 control (ESC, ^A...) among them is text to pass on, not a key to look up.` Document it in SHELL-PANEL-PATCH.md under patch #1.
- **Mode = `!command_state.reading_line`.** Start in program mode: call `set_vt_input(true)` right after `RawModeGuard::enter()`. After every `ingest_pty_chunk` in the chunk branch, recompute. If it changed, call `set_vt_input(program_mode)` and log errors with `tracing::debug!`; never print.
- **Program-mode key path:** in the `Event::Key(..)` branch, keep the existing preamble (`generation += 1`, `report_deadline = None`, `command_state.abandon_report()`, withheld-Tab replay). Then, if `program_mode`:
  - a `KeyCode::Char(c)` writes `c.encode_utf8` to the PTY, whatever its modifiers;
  - any other key code goes through `encode_key_event`, as before;
  - `handle_key` is not called;
  - `tab_pending` becomes false.
- **Restore:** `RawModeGuard::drop` calls `let _ = set_vt_input(false);` before `disable_raw_mode()`. The panic hook in main.rs does the same before its `disable_raw_mode`.
- **`set_vt_input`:** opens `Handle::current_in_handle()` → `ConsoleMode::from(handle)`, reads `mode()`, computes `with_vt_input`, and calls `set_mode` only if the value changed. Errors propagate as `std::io::Error`.

- [ ] **Step 1: Pure helper test** (io_test.rs):

```rust
#[test]
fn test_vt_input_flag_is_set_and_cleared_without_touching_other_bits() {
    use shell_panel::io::console_mode::{with_vt_input, ENABLE_VIRTUAL_TERMINAL_INPUT};
    assert_eq!(ENABLE_VIRTUAL_TERMINAL_INPUT, 0x0200);
    assert_eq!(with_vt_input(0x01f0, true), 0x03f0);
    assert_eq!(with_vt_input(0x03f0, false), 0x01f0);
    assert_eq!(with_vt_input(0x03f0, true), 0x03f0);
    assert_eq!(with_vt_input(0x0000, false), 0x0000);
}
```

- [ ] **Step 2: e2e tests** (append to tests/e2e_binary_test.rs; reuse the file's `temp_dir`, `START`, `STEP`, and `answer_line`-style matching from the I11 test):

```rust
const DA1_ANSWER: &[u8] = b"\x1b[?65;4;6;18;22;52c";

/// Answers every DA1 query (`ESC[c`) the terminal sees, as a real terminal would, until `done`.
fn answer_queries_until(term: &mut Terminal, timeout: Duration, done: impl Fn(&Terminal) -> bool) -> bool {
    let start = std::time::Instant::now();
    let mut answered = 0;
    while start.elapsed() < timeout {
        let seen = term.raw.windows(3).filter(|w| *w == b"\x1b[c").count();
        while answered < seen {
            term.send(DA1_ANSWER);
            answered += 1;
        }
        if done(term) {
            return true;
        }
        term.wait_until(Duration::from_millis(100), |_| false);
    }
    false
}

#[test]
fn test_terminal_answers_at_start_up_are_not_typed() {
    // The user's report: a slow start and `[?65;4;6;18;22;52c` typed at the first prompt.
    let dir = temp_dir("da1");
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        answer_queries_until(&mut term, START, |t| t.screen().contains("PS ")),
        "screen: {}",
        term.screen()
    );
    // Give a late answer time to land on the line.
    answer_queries_until(&mut term, Duration::from_secs(3), |_| false);
    let screen = term.screen();
    assert!(!screen.contains("65;4;6"), "a terminal answer was typed: {screen}");
    assert!(!screen.contains("[?6"), "a terminal answer was typed: {screen}");
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

/// Starts `[Console]::ReadKey` in the session and returns once it is waiting.
fn read_one_key(term: &mut Terminal) {
    term.send(b"'READY' + 'KEY'; $k = [Console]::ReadKey($true); 'GOT-' + $k.Key + '-' + $k.Modifiers\r");
    assert!(term.wait_for_text("READYKEY", STEP), "screen: {}", term.screen());
}

/// The answer line printed by `read_one_key`, not a wrapped echo of the command.
fn got_line(term: &Terminal) -> Option<String> {
    term.screen()
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("GOT-") && !l.contains('\'') && !l.contains('$'))
        .map(str::to_string)
}

#[test]
fn test_program_receives_keys_shell_panel_cannot_encode() {
    let dir = temp_dir("f13");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();
    read_one_key(&mut term);
    term.send(b"\x1b[25~"); // F13
    assert!(term.wait_until(STEP, |t| got_line(t).is_some()), "screen: {}", term.screen());
    assert_eq!(got_line(&term).as_deref(), Some("GOT-F13-None"), "screen: {}", term.screen());
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_program_receives_escape_sequences_intact() {
    let dir = temp_dir("arrow");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();
    read_one_key(&mut term);
    term.send(b"\x1b[A");
    assert!(term.wait_until(STEP, |t| got_line(t).is_some()), "screen: {}", term.screen());
    assert_eq!(got_line(&term).as_deref(), Some("GOT-UpArrow-None"), "screen: {}", term.screen());
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_resize_reaches_a_running_program() {
    let dir = temp_dir("resizeprog");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();
    term.send(b"Start-Sleep -Seconds 3; 'SZ=' + $Host.UI.RawUI.WindowSize.Width + 'x' + $Host.UI.RawUI.WindowSize.Height\r");
    term.wait_until(Duration::from_millis(800), |_| false); // the program is running
    term.resize(100, 40);
    assert!(
        term.wait_until(Duration::from_secs(15), |t| t.screen().lines().any(|l| l.trim() == "SZ=100x40")),
        "screen: {}",
        term.screen()
    );
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_console_input_mode_is_restored_after_exit() {
    // shell-panel and a probe share one console: cmd runs shell-panel, then prints the mode.
    let dir = temp_dir("restore");
    let probe = "$s='[DllImport(\"kernel32.dll\")] public static extern IntPtr GetStdHandle(int n); \
[DllImport(\"kernel32.dll\")] public static extern bool GetConsoleMode(IntPtr h, out uint m);'; \
$t=Add-Type -MemberDefinition $s -Name K -Namespace SpProbe -PassThru; $m=[uint32]0; \
[void]$t::GetConsoleMode($t::GetStdHandle(-10),[ref]$m); 'VTIN=' + ($m -band 0x200)";
    let mut cmd = portable_pty::CommandBuilder::new("cmd.exe");
    cmd.arg("/c");
    cmd.arg(format!(
        "\"{}\" --no-profile & powershell.exe -NoProfile -Command \"{}\"",
        env!("CARGO_BIN_EXE_shell-panel"),
        probe.replace('"', "\\\"")
    ));
    cmd.cwd(&*dir);
    cmd.env_remove("SHELL_PANEL_SESSION");
    let mut term = Terminal::spawn(cmd);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.send(b"exit\r");
    assert!(
        term.wait_until(Duration::from_secs(30), |t| t.screen().contains("VTIN=")),
        "screen: {}",
        term.screen()
    );
    assert!(term.screen().lines().any(|l| l.trim() == "VTIN=0"), "screen: {}", term.screen());
}
```

  Adapt the `TempDir`/`temp_dir` usage to the file's current helper. Quoting the probe through `cmd /c` may need adjusting: if `powershell.exe` misparses it, write the probe to a `.ps1` in the temp dir and call `powershell.exe -NoProfile -File <it>`. Before relying on this test, run the probe alone in a fresh console and confirm that 5.1 does not set 0x0200 itself.

- [ ] **Step 3: Run them on the current code (RED).** `cargo test -q --test io_test vt_input` fails to compile, because the module does not exist. `cargo test -q --test e2e_binary_test test_terminal_answers test_program_receives -- --test-threads=1` fails:
  - DA1: the answer is typed (paste the screen);
  - F13: times out, because the key was dropped;
  - arrow: GOT-UpArrow passes today, since classic mode decodes arrows. It is the guard for patch #2 after the switch, and the mutation in Step 6 proves it.

  `test_resize_reaches_a_running_program` and `test_console_input_mode_is_restored_after_exit` pass today; both are guards. Paste all of it.

- [ ] **Step 4: Implement** as decided: patch #2, console_mode.rs, Cargo dependency, reactor, guard, panic hook.

- [ ] **Step 5: GREEN.** Run `cargo test -q --test io_test`, then the five new e2e tests, then the whole e2e suite with `cargo test -q --test e2e_binary_test --test e2e_report_order_test --test e2e_unicode_input_test --test shell_report_test -- --test-threads=1`. All must pass.

- [ ] **Step 6: Mutation proofs** (each exit 0, on final bytes, reporting mutate.ps1's own exit code):
  - remove patch #2 (in `vendor/crossterm/src/event/sys/windows/parse.rs` make the new `if` condition `false`) → `test_program_receives_escape_sequences_intact` fails;
  - force prompt mode (in app.rs make the computed mode always `false`, e.g. `let want = false;`) → `test_terminal_answers_at_start_up_are_not_typed` and `test_program_receives_keys_shell_panel_cannot_encode` fail; run each;
  - drop the restore in `RawModeGuard::drop` → `test_console_input_mode_is_restored_after_exit` fails;
  - `with_vt_input` returning `mode` unchanged → the io_test fails.

- [ ] **Step 7: Docs.**
  - README "How it works": add one sentence. While a program runs, shell-panel passes the terminal's input through unchanged (mouse, focus, paste, every key). Only while PowerShell reads a line does it interpret keys.
  - README Known limitations: a host input sequence that arrives in the instant a program starts or ends may be read in the wrong mode.
  - SHELL-PANEL-PATCH.md: patch #2.

- [ ] **Step 8: Gate and commit.** Run the gate. Commit:

```
fix: pass terminal input through unchanged while a program runs

The host's answer to ConPTY's start-up DA1 query reached PowerShell as
typed text and ConPTY waited for its timeout; F13-F24, mouse, focus
and paste never reached programs. Outside PSReadLine the console now
has ENABLE_VIRTUAL_TERMINAL_INPUT set and its bytes go to the PTY
unchanged; a second vendored crossterm guard keeps ESC and other C0
controls; the console mode is restored on exit and on panic.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_018M2Bm1i7LaNF65teCQqU4y
```

## Deferred from the execution of this plan

Recorded by the reviews of commits 8e264a7..5c1837e; none blocks merge.

- panic hook clears VT input for panics inside tokio tasks while the session continues (same class as disable_raw_mode there)
- if set_vt_input fails, program_mode flips without the console mode (logged only)
- no test for mouse/focus/paste passthrough claims
- quiet_session 15 s wait is tight with 19 parallel sessions; raise it if CI flakes
- the 20 ms deadline or 64 KiB cap can end a write inside an escape sequence (e.g. a paste's closing ESC[201~); write up to the last ESC and carry the tail
- unfinished-escape helper does not cover OSC/DCS strings split across reads
- a resize during the wait flushes a partial sequence; an Esc/arrow typed across the switch back to prompt mode is dropped / leaves `[A` text
