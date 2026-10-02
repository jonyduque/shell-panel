# Review Minor Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the Minor findings M1–M19 of the 2026-10-01 general review and every item deferred by the critical-fixes, release-installer and important-fixes plans, or record why one is closed without a code change.

**Architecture:** No new subsystem. Five tasks by area: engine, providers and quoting; UI; shell, io and config; test infrastructure; installer and release. Each item is a test that fails first (or a test that pins an existing rule, proven by mutation) plus the smallest change.

**Tech Stack:** Rust 2021, PowerShell 7 / 5.1, GitHub Actions.

**Spec:** `docs/reviews/2026-10-01-revisao-geral.md` section "Menores", plus the "Deferred from the execution of this plan" sections of `docs/superpowers/plans/2026-10-01-review-critical-fixes.md` and `2026-10-01-review-important-fixes.md`, plus the release-installer final-review deferrals listed in Task 5.

## Global Constraints

- Gate after every task: `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/verify.ps1` → `[OK] GATE VERDE`. Installer changes also run `pwsh -NoProfile -File scripts/test-installer.ps1 -Shell pwsh` and `-Shell powershell`.
- Every behaviour change starts with a failing test (paste RED). Every test that pins an existing rule gets a mutation proof: `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path <src> -Anchor '<exact>' -Replacement '<text>' -TestCommand '<one test>'`, exit 0. Report the script's own exit code.
- No new crates.io dependency. Source files never contain raw control bytes (write `\x1b`, `\t`).
- e2e tests: `-- --test-threads=1`, `quiet_session()` first; rerun once on failure and report both runs.
- Forbidden: `git stash/reset/checkout/restore/clean` on files you did not create.
- Commits: English, conventional, one per task, ending with the repository's two attribution lines.
- An item may close without a code change only with a written reason in the report ("Closed without change: <item> — <reason>").

## Review Focus

1. Replacing a word while the cursor is inside quotes that start mid-word (`C:\'My Do|cs'`) or that contain `''` or a backtick escape — the line stays valid. Task 1.
2. Replacing a word in the middle of a line before an existing space — no double space. Task 1.
3. A carapace flag value with a space (`--name=a b`) — one argument. Task 1.
4. A truncated dropdown row with an emoji plus variation selector (`🏷️`) — never wider than the terminal. Task 2.
5. Resizing the terminal while a session runs — the shell sees the new size. Task 3.

---

### Task 1: Engine, providers and quoting

**Files:** `src/engine/lexer.rs`, `src/engine/aggregate.rs`, `src/engine/replacement.rs`, `src/engine/quote.rs`, `src/engine/providers/carapace.rs`, `src/engine/providers/zoxide.rs`; tests `tests/lexer_test.rs`, `tests/aggregate_test.rs`, `tests/engine_test.rs`.

Items and decisions:

- **M7:** test `texts("cargo build && git sta") == ["git","sta"]`, plus `texts("a || git sta")`. Pins the existing rule; mutation: drop `'&'` from `COMMAND_SEPARATORS` and run the `&&` test.
- **M8:** test in aggregate_test: external `Suggestion::new("main","main",Some("carapace".into()),70)` and a shell suggestion `main` (priority 70, `with_shell_range`) merge to exactly one item with description `"carapace"` and `uses_shell_range == false`. Mutation: make the sort unstable on ties (anchor the `then_with(|| a.name.cmp(&b.name))` line and use `.then(std::cmp::Ordering::Greater)` or similar that still compiles) and run the test.
- **M9 / "carapace vs PowerShell dedupe":** test that shell `'.\My Dir\'` (ProviderContainer) and external `'My Dir/'` merge to one item. Mutation: remove `.trim_matches(['\'', '"'])` in `dedupe_key`.
- **M10:** test `calculate_replacement("\"My", "\".\\My Documents\\\"").insert_text` has no trailing space.
- **Open-quote detection** (deferred twice: `cd C:\'My |Do'` and `contains()` misreading `''` or `` `" ``): add `pub fn open_quote(raw: &str) -> Option<char>` to lexer.rs. It scans `raw` with PowerShell rules: outside quotes a backtick escapes the next char; inside `'…'` the pair `''` is a literal quote; inside `"…"` a backtick escapes the next char. It returns the quote that is still open at the end. `plan_replacement` uses it instead of the "starts with a quote and `!contains`" check. `quoted_tail(after, q)` honours the same escapes when it looks for the closing quote. Tests:
  - `open_quote("C:\\'My Do") == Some('\'')`
  - `open_quote("'it''s") == Some('\'')`
  - `open_quote("'done'") == None`
  - `open_quote("\"a`\"b") == Some('"')`
  - `quoted_tail("s x'", '\'') == "s x'"`
  - `quoted_tail("b`\"c\" d", '"') == "b`\"c\""`
  - aggregate cases for `cd C:\'My Do|cs'` and `cd 'it''s|x'` with exact actions.
- **Unterminated quote deletes the rest of the line:** closed without change. That text is inside the string in PowerShell's own grammar. Add one aggregate test that pins it (cursor in `'abc|def ghi` with no closing quote → the tail is `def ghi`).
- **Double space mid-line:** after computing any `ReplacementAction` in `plan_replacement` (both branches), if the text right after the replaced range starts with `' '` or `'\t'`, drop the trailing space of `insert_text`. Test: `cd 'My Do|cuments' | sort` with a quoted zoxide suggestion → `insert_text` ends with `'` and no space. Also cover the shell-range branch.
- **token_tail tests thin:** add a multibyte tail (`token_tail("ção x") == "ção"`, and an aggregate case asserting `delete_count` in UTF-16 units with `😀`), and a quoted-over-unquoted case (zoxide `'My Documents'` replacing `My Do|cuments`: `backspace 5, delete 7`).
- **`&`/`;` inside quotes and backtick-escaped `&`; escaped `` `>& ``:** tests `texts("echo 'a & b' x") == ["echo","a & b","x"]`, `texts("echo \"a;b\" x")`, `texts("echo a`&b x") == ["echo","a&b","x"]`. Fix the `>&` rule so a `>` that was itself backtick-escaped does not protect the following `&`. Track an `escaped` flag for the previous char in `split_segments` and `active_token_raw`. Test `texts("echo a`>& git sta") == ["git","sta"]`.
- **M18:** remove `CommandToken.width` and its computation. Update the tests that read it (`test_lex_command_line_unicode_width` asserts something else meaningful or is deleted; say which).
- **Carapace flag value with a space** (`--name=a b`): in `parse_carapace_json`, for a value that starts with `-` and contains `=`, quote only the part after the first `=` with `quote_for_powershell` (`--name='a b'`). Values without `=` stay as they are. Test with `--name=a b` → `--name='a b'` and `--force` unchanged. Also remove the duplicated `starts_with('-')` (one `let is_flag`).
- **M11 vs M17:** keep `with_binary`. M11's test needs it, so it now has a consumer. Test: a temp `.cmd` file that sleeps about 5 s (`@ping -n 6 127.0.0.1 >nul`); `CarapaceProvider::with_binary(<that .cmd>).complete("git sta", "")` returns empty in under 1 s. Same for `ZoxideProvider::with_binary(...)` with `"cd x"` (whatever its `can_handle`/complete need). If Rust cannot spawn a `.cmd` directly on this toolchain, use `cmd.exe` as the binary only if the provider API allows it; otherwise close M11 without change and give the reason. M17 is closed by M11.

Commit: `fix(engine): honour quote escapes and spacing in replacements; quote carapace flag values; close engine minors`.

---

### Task 2: UI

**Files:** `src/ui/theme.rs`, `src/ui/renderer.rs`, `src/ui/mod.rs`, `src/vt/emulator.rs` (only if needed); tests `tests/theme_test.rs`, `tests/renderer_test.rs`, `tests/vt_test.rs`.

- **M1:** `truncate_to_width` measures the prefix as a string (`UnicodeWidthStr::width(candidate)`), not char by char, so `🏷️` (U+1F3F7 U+FE0F) counts 2. Keep it O(n·k) on short strings; that is fine. Test: `truncate_to_width("\u{1f3f7}\u{fe0f}ab", 2) == "\u{1f3f7}\u{fe0f}"`, `... , 3)` keeps one more char. Also a formatted-line test with the default option icon `"🏷️  "` and `max_width` 10: selected and unselected rows drawn into a vt100 parser are no wider than 10. Make the unselected row exercise the truncation branch: its label must exceed `max_width` (closes the deferred "I19 unselected iteration" item).
- **M12:** renderer_test for `wanted == below`: 24 rows, cursor row 18, 5 items → `start_row == 19`, `row_count == 5`. Mutation: `wanted <= below` → `wanted < below`.
- **M14:** `impl Default for Theme` returns `Theme::from_config(&Config::default())`. Remove `SELECTED_PREFIX`/`UNSELECTED_PREFIX` constants if nothing in `src/` needs them after this. Tests that used them read `Config::default().colors.*_prefix`. Update tests that asserted the reverse-video default.
- **M15:** delete `Theme::format_selected`, `Theme::format_description`, `format_selected_line`, `format_description_text` and the asserts that use them (renderer_test `test_theme_constants_and_helpers`). Run `find_consumers.ps1` for each first and paste the output.
- **Theme test checks only specific sequences:** in `test_control_characters_in_suggestion_text_are_drawn_inert`, strip the theme's own SGR sequences (`\x1b[` digits/`;` `m`) and then assert no char of the line is a control char (`c.is_control()`).
- **Bidi overrides / U+2028:** extend `inert` to also replace U+200E, U+200F, U+202A–U+202E, U+2066–U+2069, U+2028, U+2029 with `'?'`. Test with `"a\u{202e}b\u{2028}c"` → `a?b?c`.
- **M19:** in `tests/vt_test.rs`, after `term.resize(100, 40)` assert `term.screen().size() == (40, 100)`. After processing `abc` at a known position, assert cell contents. Mutation: make `HeadlessTerminal::resize` a no-op.

Commit: `fix(ui): measure truncation by string width, neutralise bidi controls, derive the default theme from the config; close UI minors`.

---

### Task 3: Shell, io and config

**Files:** `src/shell/command_state.rs`, `src/shell/integration.rs`, `assets/shellIntegration.ps1`, `src/core/app.rs`, `src/core/config.rs` (tests only unless a fix is needed), `src/pty/conpty.rs`, `src/io/key_event.rs` (tests only), `vendor/crossterm/Cargo.toml` (lints only), `vendor/crossterm/SHELL-PANEL-PATCH.md`; tests `tests/osc_test.rs`, `tests/stream_test.rs`, `tests/shell_report_test.rs`, `tests/config_test.rs`, `tests/io_test.rs`, `tests/pty_test.rs`, `tests/e2e_binary_test.rs`, `tests/common/mod.rs`.

- **M3/M4:** config_test: `Config::load` on a temp file holding `default_sample_toml()` returns `config == Config::default()` and no warnings. Mutation: remove `"icons"` from `TOP_LEVEL_KEYS`.
- **M5:** stream_test: chunk 1 `"a" + "\x1b]6973;<TOKEN>;RE\x1b"`, chunk 2 `"\\b"` → outputs `a` then `b`, `reading_line == false` (it was set true by a preceding RS in the test).
- **M6:** delete `ConPtySession::resize` and `test_conpty_session_spawn_and_resize`. The reactor's inline `pair.master.resize` stays the one path. Add an e2e test of that path: give `tests/common::Terminal` a `resize(cols, rows)` that calls `_master.resize`. In a shell-panel session, resize to 100×40 and run `'SZ=' + $Host.UI.RawUI.WindowSize.Width + 'x' + $Host.UI.RawUI.WindowSize.Height`. Expect `SZ=100x40` as a screen line. Mutation: remove the `pair.master.resize(...)` call in app.rs.
- **M13:** io_test: Ctrl+Space → `[0x00]`, Ctrl+`[` → `[0x1b]`, Ctrl+`]` → `[0x1d]`.
- **Token redaction:** implement `Debug` for `CommandState` by hand and print `token: "<redacted>"`. Test: `format!("{:?}", CommandState::new(TOKEN))` does not contain `TOKEN`.
- **`script(token)` input check:** `script` asserts `token` is non-empty ASCII hex (`assert!`, with a message). Test with `#[should_panic]`.
- **Token literal instead of a global variable:** `assets/shellIntegration.ps1` uses the placeholder `__SP_TOKEN__` inside `__SP-Send`; `script(token)` replaces it (no `$Global:__SP_Token` line any more). Test: `script(TOKEN)` contains the token inside the `__SP-Send` body and does not contain `$Global:__SP_Token`. The existing pty tests keep passing. Add one shell_report test: after `Remove-Variable * -Scope Global -ErrorAction SilentlyContinue`, a report request still produces a CMP message with the token. Update the README sentence on the token if it mentions the variable.
- **Two sources of "a Tab waits":** add the comment at the `report_deadline.take().is_some()` guard in app.rs. It says `CommandState::awaiting_report` and `report_deadline` must change together, and that the guard makes `abandon_report` defensive. Closed with that comment, no further change.
- **`request_report` saturation:** `saturating_add(1)`.
- **Shell messages tests:** shell_report_test cases for a lone surrogate (built with `[char]0xD83D` in the session) and for `\`/`;` mixed with non-ASCII. Each report decodes and the line round-trips (the lone surrogate as U+FFFD).
- **In-time report after a stale one, end to end:** closed without change. It is unit-tested (`test_only_the_answer_to_the_last_request_is_kept`), and the slow-completer e2e tests already exercise the stale path. An e2e test that needs both a stale and a timely report would need two completers with tuned delays, which is flaky by construction.
- **Vendored crossterm build warnings:** add `[lints.rust] unexpected_cfgs = "allow"` and `dead_code = "allow"` to `vendor/crossterm/Cargo.toml` and note it in `SHELL-PANEL-PATCH.md`. Verify `cargo build` prints no crossterm warning (paste).
- **Alt-code release path untested:** closed without change. Exercising it needs crafted console input records, which only a console-injection harness can produce. The guard's condition is mirrored from crossterm's own `is_alt_code` check and was read against it.
- **Unterminated quote**: see Task 1.

Commit: `fix(shell): embed the session token, redact it in Debug, pin resize and message encoding; close shell and io minors`.

---

### Task 4: Test infrastructure

**Files:** `tests/common/mod.rs`, `tests/e2e_report_order_test.rs`, `tests/e2e_unicode_input_test.rs`, `tests/shell_report_test.rs`, `tests/e2e_binary_test.rs`, `tests/pty_test.rs`, `tests/engine_test.rs`, `tests/cli_test.rs`, `tests/osc_test.rs`.

- **`remove_dir_when_released` copied three times:** move one copy to `tests/common/mod.rs` as `pub fn remove_dir_when_released(dir: &Path)` and make the three files use it. `shell_report_test.rs` already has `mod common;`; add it where missing.
- **Temp dirs leak on panic; `temp_dir` never removed:** add `pub struct TempDir(PathBuf)` to `tests/common` with `new(tag)` (unique per process and tag) and a `Drop` that calls `remove_dir_when_released`. Use it in `e2e_binary_test.rs`, and in the I13 (pty_test) and I16 (engine_test) tests (both add `mod common;`). A session that holds the dir as cwd is dropped before the guard, so declare the guard first.
- **I12 config case asserts only the exit code:** also assert the stderr contains `unsupported shell "bash"`.
- **osc_test `ahead` case has no own mutation run:** run the I15 mutation against `cargo test -q --test osc_test test_report_ranges` and paste it (exit 0).
- **I11 `GOT-` line could match a wrapped echo:** match only a screen line that is entirely the answer: trimmed, it starts with `GOT-` and contains neither `'` nor `$`.
- **`Terminal::drain` has no overall deadline:** add `max: Duration` (drain stops at `max` even if output keeps coming). Use 5 s at the call site.
- **quiet_session on Windows PowerShell 5.1:** closed without change. PSReadLine 2.0 saves the accepted line before any option in it takes effect, and has no sensitive-line rule. The doc comment already says so.

Commit: `test: share temp-dir guards and cleanup, tighten e2e matching and draining; close test-infrastructure minors`.

---

### Task 5: Installer and release

**Files:** `install.ps1`, `uninstall.ps1` (only if needed), `scripts/test-installer.ps1`, `.github/workflows/release.yml`, `README.md` (M16).

- **M16:** replace the README's `shell-panel --print-default-config | Set-Content ...` with a form that writes UTF-8 on both shells and keeps the emoji intact: `[IO.File]::WriteAllText("$HOME\.config\shell-panel.toml", (shell-panel --print-default-config | Out-String), [Text.UTF8Encoding]::new($false))`. Check it against the wiki's instruction and align both.
  - Verify on both shells that the file holds `📁` byte-exact. Pipe the output into a file through `[Console]::OutputEncoding = [Text.Encoding]::UTF8` if needed, and state what was required.
  - If the native output is decoded through the OEM code page and the emoji cannot survive a pipe on 5.1, document `shell-panel --print-default-config > file` from pwsh only, like the wiki already does. Say which.
- **GitHub API rate limit:** when the `releases/latest` lookup fails, rethrow with a message that names the cause, when it is visible (HTTP 403 or "rate limit"), and suggests `-Version`. Hard to test end to end. Unit-test it by extracting the lookup into a function inside the script block that takes the URI, and calling the installer with `-ApiUri` (a testing-only parameter) pointing at a local unreachable address (`http://127.0.0.1:9/`). Assert the message mentions `-Version`.
- **Publish not idempotent:** in `release.yml`'s publish step, if `gh release view $tag` succeeds, use `gh release upload $tag <files> --clobber` instead of `create`. Validate with actionlint.
- **TLS 1.2 flag left in the caller's session:** save `[Net.ServicePointManager]::SecurityProtocol` before changing it and restore it in the `finally`.
- **Installer test gaps:** add cases to `scripts/test-installer.ps1`:
  - a user `Path` that is `REG_SZ` (it stays `REG_SZ`);
  - a missing `Path` value (it is created as `REG_EXPAND_SZ` holding only the install dir, and uninstall leaves it empty or absent; state which);
  - a `SHA256SUMS.txt` without the zip's line (install fails and nothing is installed).

  Restore the original value as the test already does.

Commit: `fix(installer): helpful errors on a failed release lookup, restore TLS settings, idempotent publish; close installer minors`.

---

## Closed elsewhere

- M2 (prediction and history in e2e): closed by `quiet_session` (commits 852378d, 1129890).
- "Cursor inside a quoted word containing a space leaves the rest": closed by `quoted_tail` (753c35f); Task 1 extends it to escapes.
- "token_tail tests thin": Task 1.

## Deferred from the execution of this plan

Recorded by the task and final reviews of commits 655bf9d..4ba0a0a; none blocks merge.

- M8 mutation relies on sort internals (indirect but either outcome fails)
- open_quote test uses C:'My Do (no backslash) — aggregate test covers the backslash form
- quoted_tail with cursor right after a backtick inside "..." and doubled "" inside double quotes
- trailing space kept before a separator (checkout ;x) — spec-conformant
- publish step treats any `gh release view` failure as "not found"; upload branch does not refresh title/prerelease
- TLS restore verified by hand only (child process state not observable from the test)
- wiki Configuration.md prose could mention the encoding is restored
- PSReadLine reads $? first; behind the wrapper it sees the wrapper's own $? (error-status indicator lost) — pre-existing
- tests/common resize doc wording; "icons" in TOP_LEVEL_KEYS unreachable
- remove_dir_when_released gives up silently; drain may overshoot max by one quiet interval; shell_report/e2e_unicode still use raw temp dirs
