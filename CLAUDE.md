# shell-panel — instructions for agents

Rust (edition 2021) Windows-only CLI. It runs PowerShell (pwsh 7 or Windows PowerShell 5.1) inside a ConPTY, mirrors the screen in a headless vt100, and on Tab draws a completion dropdown fed by PowerShell's own completions (reported from inside the user's session), Fig-style JSON specs, carapace and zoxide. A PowerShell installer and GitHub release workflows ship it.

## Commands

| What | Command |
|---|---|
| Full gate (fmt, clippy `-D warnings`, all tests) | `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/verify.ps1` |
| One test binary | `cargo test -q --test <name>` |
| End-to-end / PTY tests | `cargo test -q --test e2e_binary_test -- --test-threads=1` (also `e2e_report_order_test`, `e2e_unicode_input_test`, `shell_report_test`, `e2e_pty_test`, `pty_test`) |
| Installer end-to-end test | `pwsh -NoProfile -File scripts/test-installer.ps1 -Shell pwsh` and `-Shell powershell` — **always launched from pwsh 7**; it needs `cargo build --release` first; one run per machine (it holds the mutex `Global\shell-panel-installer-test`) |
| Mutation proof | `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path <src> -Anchor '<exact text>' -Replacement '<text>' -TestCommand '<one test>'` |
| Snapshot / compare the user's machine state | `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/user_state.ps1 -Save <file>` then `-Compare <file>` |
| Workflow lint | `go run github.com/rhysd/actionlint/cmd/actionlint@latest .github/workflows/ci.yml .github/workflows/release.yml` |

## Safety — the user's machine is not a test fixture

These rules exist because each was broken once in this repository.

- **Never run `install.ps1` or `uninstall.ps1` with default locations**, as a file or through `irm | iex`.
  - Defaults are `%LOCALAPPDATA%\Programs\shell-panel` and the real Windows Terminal fragment. Running them with defaults reinstalled the user's real install once.
  - Run them only through `scripts/test-installer.ps1`, or with explicit temp `-InstallDir` and `-TerminalFragmentDir`.
- **Never write the user's `HKCU\Environment` `Path`** yourself.
  - Only `scripts/test-installer.ps1` may change it, and it restores the value.
  - The harness once leaked `;%SP_INSTALLER_TEST%\bin` into the user's PATH, after it was run under Windows PowerShell 5.1 and died before its restore. If you find the PATH changed, stop and report; do not "fix" the registry.
- **Never touch the user's PSReadLine history** (`(Get-PSReadLineOption).HistorySavePath`), Windows Terminal fragments, or `%USERPROFILE%\.config\shell-panel*`.
- Before and after any run that starts PowerShell sessions or the installer test, `user_state.ps1 -Save` / `-Compare` must show no difference.
- Never `git stash/reset/checkout/restore/clean` files you did not create. Never push or create tags without the user's explicit request for that step.

## Invariants (a change that breaks one is a defect)

- The user's terminal is the product: stdout carries only the shell's output and the dropdown; logs go to a file only with `--verbose`.
- Raw mode, the cursor and the console input mode are restored on every exit, including error and panic. The VT-input bit is restored to exactly the value found at start.
- PTY output is untrusted.
  - Every OSC 6973 message carries the per-session token, embedded as a literal in `__SP-Send`. Messages without it are ignored.
  - Text that is typed into the PTY never contains control characters.
  - Text drawn in the dropdown has C0/C1/DEL/bidi controls replaced by `?`.
- Report indices are UTF-16 code units.
- A Tab is never lost nor applied twice. Without a usable report within 3 s, Tab goes to PowerShell.
- Two input modes follow `CommandState::reading_line`:
  - **Prompt mode** (PSReadLine reading) decodes keys and handles Tab and the dropdown.
  - **Program mode** (everything else, including start-up) sets `ENABLE_VIRTUAL_TERMINAL_INPUT` and writes the host's bytes raw. Keys queued together are coalesced into one write, because ConPTY reads a lone trailing ESC as the Escape key.
- `assets/shellIntegration.ps1` and the installer scripts are **ASCII only**.
  - 5.1 reads BOM-less scripts as ANSI, so emoji and ESC are built at run time with `[char]` / `ConvertFromUtf32`.
  - The integration script never prints over the edited line and never changes `$?`/`$LASTEXITCODE` seen by the prompt.
  - It never uses `-ExecutionPolicy Bypass`.
  - It holds the original `PSConsoleHostReadLine` in a closure created in a **child scope** (`& { param($sp_original) { … }.GetNewClosure() } …`). `GetNewClosure()` at global scope snapshots `$PWD` and the preference variables.
- `install.ps1`/`uninstall.ps1` must work both as files and through `irm | iex`.
  - Parameters are bound at script level and forwarded with `@PSBoundParameters` (`@args` turns `-Switch:$false` into true).
  - The body is a child-scope script block. It throws and never calls `exit`.
  - The uninstaller only removes a folder that carries the installer's `.shell-panel-install` marker.
- `vendor/crossterm` is crossterm 0.28.1 with two documented patches (`vendor/crossterm/SHELL-PANEL-PATCH.md`): surrogate pairs only from key-down records, and C0 controls kept for virtual-key code 0. Never format or lint-fix vendored code; the workspace excludes `vendor`.

## Facts measured here (do not re-derive, do not contradict without new measurement)

- ConPTY asks the host DA1 (`ESC[c`) at start-up and waits for the answer: the prompt appears in about 2 s when it is answered, about 4–6 s when it is not.
- ConPTY delivers F1–F12 only; `ESC[25~` (F13) never reaches a program, even without shell-panel.
- Windows timers have about 15.6 ms resolution. `tokio::time::timeout(5 ms)` and even a zero-duration timeout take about 16 ms. Poll without waiting when you mean "only what is already queued".
- `cmd.exe` resets the console mode after each command, so a test hosted by `cmd /c` cannot observe a missing console-mode restore. Host such probes in PowerShell.
- On the GitHub Windows runner, `%TEMP%` is an 8.3 short path (`RUNNER~1`) while PowerShell reports long paths. The console host there swallows some undecodable sequences that a Windows 11 host passes on as text.
- On a cold machine the first PowerShell completion can exceed 3 s. Tests that expect a dropdown call `warm_completion()` first.

## Tests

- TDD: write the failing test first and watch it fail for the stated reason. Every rule a test pins gets a mutation proof (`mutate.ps1` exit 0 = caught).
  - The proof runs on the final committed bytes, and the report gives the script's own exit code, not a pipe's.
  - A surviving mutation means either the test is weak or the mutation is equivalent. Find out which and say so.
- End-to-end tests (`tests/common/mod.rs`, `Terminal`):
  - call `term.quiet_session()` right after the first prompt; it keeps the session out of the real history and turns off inline predictions;
  - call `warm_completion()` before a timed Tab;
  - build every marker you wait for by concatenation (`'QUIET' + 'READY'`), so the echoed command line cannot satisfy the wait;
  - a wait must target text typed after anything stale on screen;
  - use `TempDir` for temporary directories and `drain(quiet, max)` after `wait_exit`;
  - run e2e tests with `--test-threads=1`, and rerun a failure once before concluding (report both runs). Load from parallel ConPTY sessions has overrun two timeouts: in an earlier CI run 19 sessions made the first command miss the 15 s `quiet_session` wait (once in 8 runs), and in the v0.2.0 release run the first completion report after `git ` Tab missed the 3 s report timeout. CI therefore runs the tests with two threads.
- No raw control bytes in source files: write `\x1b`, `\t`.
- The shell helper for `.cmd` processes: `kill_on_drop` kills `cmd.exe` but not its grandchildren.

## Workflow conventions

- Plans live in `docs/superpowers/plans/`, specs in `docs/superpowers/specs/`, review reports in `docs/reviews/`. Each plan ends with a "Deferred from the execution of this plan" list. Read the latest ones before planning new work.
- Execution is subagent-driven: an implementer per task, a task reviewer (the most capable model for anything touching the integration script, the reactor or the installer), and a final whole-branch review. The ledger lives in `.superpowers/sdd/<plan>/progress.md` and is git-ignored.
- Review the whole project with the `shell-panel-review` skill and `.claude/workflows/shell-panel-review.js`, configured by `.claude/review/review.json`.
- Commits: English, conventional (`fix(scope): …`), with the attribution lines the session provides. Work on a branch; merge and push only on the user's request.
- Release: set `version` in `Cargo.toml`, build so `Cargo.lock` follows, gate, commit, then `git tag v<version>` and `git push origin v<version>`. The Release workflow verifies the tag, gates, builds x64/ARM64, tests the installer and publishes. If a job fails intermittently, re-run the failed jobs before changing anything.
