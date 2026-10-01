# Review Important Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the Important findings I1–I7 and I9–I19 of the 2026-10-01 general review, make the end-to-end suite stable on a cold CI runner, and stop it from reading or writing the user's real PSReadLine history.

**Architecture:** Six product fixes in the engine, io and ui layers: I2 is a single set of command separators shared by the lexer functions; I1 replaces the token on both sides of the cursor; I3 adds one quoting helper used by zoxide and carapace; I4 encodes Alt+Enter as a win32-input-mode record; I5 checks hex colours before slicing them; I6 removes control characters from the display text. Then two test-only tasks: unit tests for documented rules that no test could fail (I12, I13, I15–I19), and end-to-end tests for the reactor and session rules (I7, I9, I10, I11, I14), plus a completion warm-up and history isolation for every end-to-end test that types into a session.

**Tech Stack:** Rust 2021, crossterm 0.28 (vendored), portable-pty, vt100, PowerShell 7 / 5.1, PSReadLine 2.x.

**Spec:** `docs/reviews/2026-10-01-revisao-geral.md`, section "Importantes" (raw evidence per finding in `docs/reviews/2026-10-01-revisao-geral.json`). I8 was closed by commit ec1d284.

## Global Constraints

- Gate after every task: `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/verify.ps1` → `[OK] GATE VERDE`.
- Mutation proofs: `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path <src file> -Anchor '<exact text>' -Replacement '<text>' -TestCommand '<one test>'`; exit 0 = the test caught it. Evidence is taken on the final committed bytes (after `cargo fmt`).
- No new crates.io dependency. Do not edit `vendor/`.
- stdout belongs to the user's terminal: no `println!`/`eprintln!` after raw mode is entered.
- Report indices are UTF-16 code units; a Tab is never lost nor applied twice.
- Every end-to-end test that types into a session calls `term.quiet_session()` first (it exists in `tests/common/mod.rs`).
- End-to-end tests run with `-- --test-threads=1`; a failure is rerun once before concluding, and both runs are reported.
- Commit messages: English, conventional, ending with the two attribution lines used in this repository.
- Forbidden: `git stash/reset/checkout/restore/clean` on files you did not create.

## Review Focus

1. A command on a continuation line (`{` then Enter, or a backtick line continuation) — expected: the same completions as on a single line. Pinned in Task 1.
2. Tab with the cursor inside a word, accepting a suggestion that did not come from PowerShell — expected: the word is replaced, not duplicated. Pinned in Task 2.
3. A carapace value or zoxide path with PowerShell metacharacters (` `, `;`, `$`, `&`, `(`) — expected: one quoted argument. Pinned in Task 3.
4. Hostile text in a tooltip or description (ESC, BEL, C1) — expected: drawn as inert characters, the screen and clipboard untouched. Pinned in Task 6.
5. The first Tab of a session on a slow machine (cold PowerShell completion) — expected: the dropdown still opens in tests; in real use the documented 3 s fallback applies. Pinned in Task 8 (warm-up helper used by every dropdown test).

## Execution order and file overlap

1 → 2 (both edit `src/engine/lexer.rs`; Task 2 uses Task 1's separator constant), then 3, 4, 5, 6 in any order, then 7, then 8. Task 7 edits `src/pty/shell.rs` (I13); Task 8 edits `tests/common/mod.rs` and `tests/e2e_binary_test.rs`.

---

### Task 1: One set of command separators, line breaks included (I2)

#### What binds this task
- Decided: a single constant `COMMAND_SEPARATORS: &[char] = &['|', ';', '&', '(', '{', '\n', '\r']` in `src/engine/lexer.rs`, used by `split_segments` (each separator outside quotes ends a segment; `&&` and `||` are two separators with an empty segment between, which is harmless because only the last segment is lexed) and by `active_token_raw` (separators plus `' '` and `'\t'`). A single `&` becomes a separator: in PowerShell it is the call or background operator, both of which start a new command.
- In `lex_segment`, a backtick immediately followed by `\n` or `\r\n` is a line continuation: it ends the current word like whitespace, and the line break is not part of any token.
- The special case that drops a leading `&` token in `lex_command_line` is no longer reachable for `&`; keep it for `.` only.
- Update the doc comments of `lex_command_line` and `split_segments` to list the separators.

#### The measured evidence of the defect
`"if ($x) {\ngit sta"` lexes to `["\ngit", "sta"]`; `can_handle("\ngit")` is false and Tab gets no git completion. `"git status & git ch"` keeps `&` as a token and the spec provider returns nothing.

#### Files
- Modify: `src/engine/lexer.rs`
- Test: `tests/lexer_test.rs`

#### Interfaces
- Produces: `pub const COMMAND_SEPARATORS: &[char]` in `crate::engine::lexer` (Task 2 uses it).

- [ ] **Step 1: Failing tests** — append to `tests/lexer_test.rs`:

```rust
#[test]
fn test_line_breaks_separate_commands() {
    assert_eq!(texts("if ($x) {\n  git sta"), vec!["git", "sta"]);
    assert_eq!(texts("Get-Location\ngit sta"), vec!["git", "sta"]);
    assert_eq!(texts("Get-Location\r\ngit sta"), vec!["git", "sta"]);
    assert_eq!(texts("git status\n"), vec![""]);
}

#[test]
fn test_single_ampersand_starts_a_command() {
    assert_eq!(texts("git status & git ch"), vec!["git", "ch"]);
    assert_eq!(texts("git status && git ch"), vec!["git", "ch"]);
    assert_eq!(active_token_raw("git status & git ch"), "ch");
}

#[test]
fn test_backtick_line_continuation_is_whitespace() {
    assert_eq!(texts("git status `\n  --sh"), vec!["git", "status", "--sh"]);
    assert_eq!(texts("git status `\r\n  --sh"), vec!["git", "status", "--sh"]);
}

#[test]
fn test_lexer_and_active_token_agree_on_separators() {
    for input in [
        "if ($x) {\n  git sta",
        "Get-Location\r\ngit sta",
        "git status & git ch",
        "a | b; c && git ch",
        "$r = (git sta",
    ] {
        let last = texts(input).pop().unwrap();
        assert_eq!(active_token_raw(input), last, "input: {input:?}");
    }
}
```

- [ ] **Step 2:** `cargo test -q --test lexer_test` → the four new tests FAIL (e.g. `left: ["\ngit", "sta"]`).

- [ ] **Step 3: Implement.** Add the constant with a doc comment ("Characters that end a command outside quotes: pipeline, statement separators, call/background operator, sub-expression or script block, line breaks."). In `split_segments` replace the `'|'`, `';'`, `'&'` and `'('/'{'` branches by one branch `else if COMMAND_SEPARATORS.contains(&ch) { segments.push(&input[start..byte_pos]); i += 1; start = if i < len { chars[i].0 } else { input.len() }; }`. In `active_token_raw` replace the delimiter arm by `c if c == ' ' || c == '\t' || COMMAND_SEPARATORS.contains(&c) => start = next,`. In `lex_segment`, outside quotes, before the generic backtick branch:

```rust
                } else if c == '`' && matches!(chars.get(i + 1), Some('\n') | Some('\r')) {
                    // Line continuation: the backtick and the line break separate words.
                    i += 1;
                    if chars.get(i) == Some(&'\r') {
                        i += 1;
                    }
                    if chars.get(i) == Some(&'\n') {
                        i += 1;
                    }
                    break;
```

and treat `'\r'`/`'\n'` like `' '`/`'\t'` in the two whitespace checks of `lex_segment`. In `lex_command_line`, `ends_with_space` also accepts a final `'\n'`/`'\r'`, and the leading-operator removal checks only `raw_tokens[0].text == "."`.

- [ ] **Step 4:** `cargo test -q --test lexer_test --test engine_test --test aggregate_test` → all pass (existing script-block, call-operator and quote tests included).

- [ ] **Step 5: Mutation proofs** (each exit 0):
```
... -Path src/engine/lexer.rs -Anchor "'(', '{', '\n', '\r']" -Replacement "'(', '{']" -TestCommand 'cargo test -q --test lexer_test test_line_breaks_separate_commands'
... -Path src/engine/lexer.rs -Anchor "&['|', ';', '&'," -Replacement "&['|', ';'," -TestCommand 'cargo test -q --test lexer_test test_single_ampersand_starts_a_command'
```

- [ ] **Step 6:** gate; commit `fix(engine): treat line breaks and a single & as command separators everywhere`.

---

### Task 2: Replace the whole word around the cursor for every source (I1)

#### What binds this task
- Decided: for suggestions without `uses_shell_range` (specs, carapace, zoxide), `plan_replacement` erases the active token before the cursor **and** its tail after the cursor, then types the suggestion: `replace_range(active_token_raw(&line[..cursor]), token_tail(&line[cursor..]), &name)`. `replace_range` already falls back to `calculate_replacement` when the tail is empty, so end-of-line behaviour is unchanged.
- `token_tail(after)` in `src/engine/lexer.rs`: the prefix of `after` up to (not including) the first `' '`, `'\t'` or `COMMAND_SEPARATORS` char. Quotes are not interpreted (the cursor may be inside them; the tail stops at the first separator either way).

#### The measured evidence of the defect
`git checkout` with the cursor after `chec` + Tab → `ReplacementAction { backspace_count: 0, delete_count: 0, insert_text: "kout " }` → `git checkout kout`. Same for zoxide (`cd proj|etos`) and carapace.

#### Files
- Modify: `src/engine/lexer.rs`, `src/engine/aggregate.rs:177` (`plan_replacement`)
- Test: `tests/aggregate_test.rs`, `tests/lexer_test.rs`

- [ ] **Step 1: Failing tests.** `tests/lexer_test.rs` (add `token_tail` to the import):

```rust
#[test]
fn test_token_tail_stops_at_whitespace_and_separators() {
    assert_eq!(token_tail("kout"), "kout");
    assert_eq!(token_tail("kout --quiet"), "kout");
    assert_eq!(token_tail("etos|sort"), "etos");
    assert_eq!(token_tail(" next"), "");
    assert_eq!(token_tail(""), "");
}
```

`tests/aggregate_test.rs`:

```rust
#[test]
fn test_external_suggestion_mid_word_replaces_the_whole_word() {
    // `git chec|kout`: the spec suggests `checkout`.
    let r = report("git checkout", 8, 4, 8, vec![]);
    let spec = Suggestion::new("checkout", "checkout", None, 90);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 4,
            delete_count: 4,
            insert_text: "checkout ".into()
        }
    );

    // The tail ends at the next word: `git chec|kout --quiet`.
    let r = report("git checkout --quiet", 8, 4, 8, vec![]);
    assert_eq!(plan_replacement(&r, &spec).delete_count, 4);

    // At the end of the word nothing after the cursor is deleted.
    let r = report("git chec", 8, 4, 4, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "kout ".into()
        }
    );
}
```

- [ ] **Step 2:** `cargo test -q --test aggregate_test --test lexer_test` → compile error (`token_tail`), then, with a stub returning `""`, the aggregate test fails with `delete_count: 0`.

- [ ] **Step 3: Implement** `token_tail` (doc: "The rest of the word that continues after the cursor: what has to be deleted forwards to replace the whole word.") and in `plan_replacement`:

```rust
        _ => replace_range(
            active_token_raw(&report.line[..cursor]),
            token_tail(&report.line[cursor..]),
            &suggestion.name,
        ),
```

(remove the now-unused `calculate_replacement` import if clippy says so).

- [ ] **Step 4:** `cargo test -q --test aggregate_test --test lexer_test --test engine_test` → pass.

- [ ] **Step 5: Mutation proof:** `-Path src/engine/aggregate.rs -Anchor 'token_tail(&report.line[cursor..]),' -Replacement '"",' -TestCommand 'cargo test -q --test aggregate_test test_external_suggestion_mid_word'` → exit 0.

- [ ] **Step 6:** gate; commit `fix(engine): replace the whole word around the cursor for spec, carapace and zoxide suggestions`.

---

### Task 3: Quote carapace values like zoxide paths (I3, I17)

#### What binds this task
- Decided: move `quote_for_powershell` unchanged into a new module `src/engine/quote.rs` (`pub mod quote;` in `src/engine/mod.rs`); `zoxide.rs` keeps `pub use crate::engine::quote::quote_for_powershell;` so existing imports keep working.
- In `parse_carapace_json`, the suggestion `name` is `quote_for_powershell(&item.value)` unless the value starts with `-` (a flag); `display` stays the unquoted value (or carapace's `display`).
- `trailing_space` already strips quotes before looking for a trailing `/`, so `'My Dir/'` gets no space.

#### The measured evidence of the defect
carapace returned `"value":"My Dir/"`; after the merge the line became `git -C My Dir/` (two arguments). For I17: removing `;` from the metacharacter list survives every engine test.

#### Files
- Create: `src/engine/quote.rs`
- Modify: `src/engine/mod.rs`, `src/engine/providers/zoxide.rs`, `src/engine/providers/carapace.rs`
- Test: `tests/engine_test.rs`

- [ ] **Step 1: Failing tests** — in `tests/engine_test.rs` (import `parse_carapace_json` from `shell_panel::engine::providers::carapace` if not imported):

```rust
#[test]
fn test_carapace_values_are_quoted_for_powershell() {
    let sugs = parse_carapace_json(
        r#"{"values":[{"value":"My Dir/","display":"My Dir/"},{"value":"--force"},{"value":"main"}]}"#,
    );
    let names: Vec<&str> = sugs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["'My Dir/'", "--force", "main"]);
    assert_eq!(sugs[0].display, "My Dir/");
}

#[test]
fn test_every_powershell_metacharacter_is_quoted() {
    for path in [r"C:\a;b", r"C:\x$y", r"C:\a&b", r"C:\p(1)", r"C:\a b", r"C:\a,b", r"C:\a|b"] {
        let quoted = quote_for_powershell(path);
        assert!(
            quoted.starts_with('\'') && quoted.ends_with('\''),
            "{path} -> {quoted}"
        );
    }
}
```

- [ ] **Step 2:** `cargo test -q --test engine_test carapace_values` → FAIL (`'My Dir/'` expected, `My Dir/` found). `test_every_powershell_metacharacter_is_quoted` passes already (it pins the existing list).

- [ ] **Step 3: Implement** as decided.

- [ ] **Step 4:** `cargo test -q --test engine_test --test aggregate_test` → pass.

- [ ] **Step 5: Mutation proofs** (each exit 0):
```
... -Path src/engine/quote.rs -Anchor '"''\"`$(){};,&@#|<>"' -Replacement '"''\"`$(){},&@#|<>"' -TestCommand 'cargo test -q --test engine_test test_every_powershell_metacharacter_is_quoted'
... -Path src/engine/providers/carapace.rs -Anchor 'quote_for_powershell(&item.value)' -Replacement 'item.value.clone()' -TestCommand 'cargo test -q --test engine_test test_carapace_values_are_quoted'
```
(Copy the first anchor from the file if the escaping differs; it must be the metacharacter string minus `;`.)

- [ ] **Step 6:** gate; commit `fix(engine): quote carapace values that PowerShell would split or interpret`.

---

### Task 4: Alt+Enter does not run the line (I4)

#### What binds this task
- Decided: `encode_key_event` for `KeyCode::Enter` matches `(shift, ctrl, alt)`; only `(false, false, false)` is `\r`; every other combination is the win32-input-mode record `\x1b[13;28;13;1;<state>;1_` with state = `0x10` (SHIFT) | `0x08` (LEFT_CTRL) | `0x02` (LEFT_ALT). Shift+Enter and Ctrl+Enter keep their current bytes.

#### The measured evidence of the defect
With the record `\x1b[13;28;13;1;2;1_`, plain pwsh did not execute the line; through shell-panel, which sent `\r`, it did.

#### Files
- Modify: `src/io/key_event.rs:110-121`
- Test: `tests/io_test.rs`

- [ ] **Step 1: Failing test** — in `tests/io_test.rs`:

```rust
#[test]
fn test_alt_enter_is_a_record_not_a_carriage_return() {
    let enter = |m| encode_key_event(&make_key_event(KeyCode::Enter, m, KeyEventKind::Press));
    assert_eq!(enter(KeyModifiers::ALT), b"\x1b[13;28;13;1;2;1_".to_vec());
    assert_eq!(enter(KeyModifiers::ALT | KeyModifiers::SHIFT), b"\x1b[13;28;13;1;18;1_".to_vec());
    // Unchanged:
    assert_eq!(enter(KeyModifiers::NONE), b"\r".to_vec());
    assert_eq!(enter(KeyModifiers::SHIFT), b"\x1b[13;28;13;1;16;1_".to_vec());
    assert_eq!(enter(KeyModifiers::CONTROL), b"\x1b[13;28;13;1;8;1_".to_vec());
}
```

- [ ] **Step 2:** `cargo test -q --test io_test alt_enter` → FAIL (`\r` for ALT).
- [ ] **Step 3: Implement** as decided.
- [ ] **Step 4:** `cargo test -q --test io_test` → pass.
- [ ] **Step 5: Mutation proof:** anchor the ALT bit (`if alt { 0x02 } else { 0 }`) → replace with `0` → `cargo test -q --test io_test alt_enter` exit 0.
- [ ] **Step 6:** gate; commit `fix(io): send Alt+Enter as a key record so it does not run the line`.

---

### Task 5: A non-ASCII hex colour is ignored, not a panic (I5)

#### What binds this task
- Decided: in `parse_color_spec`, after `strip_prefix('#')`, return `None` unless every byte of `hex_part` is an ASCII hex digit; the length checks stay byte-based (now equal to char counts). Matches the wiki: an unreadable colour falls back silently.

#### The measured evidence of the defect
`selected_bg = "#aé"` → `panicked at src\ui\color.rs:44:49: end byte index 1 is not a char boundary`, exit 101.

#### Files
- Modify: `src/ui/color.rs:36-50`
- Test: `tests/color_test.rs`

- [ ] **Step 1: Failing test:**

```rust
#[test]
fn test_non_ascii_hex_colours_are_ignored_without_panicking() {
    for value in ["#a\u{e9}", "#\u{e9}a", "#aaa\u{e9}a", "#\u{1f600}", "#ab\u{e9}"] {
        assert_eq!(parse_color_fg(value), None, "{value:?}");
        assert_eq!(parse_color_bg(value), None, "{value:?}");
    }
    assert!(parse_color_fg("#3b82f6").is_some());
}
```

- [ ] **Step 2:** `cargo test -q --test color_test non_ascii` → FAIL by panic.
- [ ] **Step 3: Implement** (`if !hex_part.bytes().all(|b| b.is_ascii_hexdigit()) { return None; }`).
- [ ] **Step 4:** `cargo test -q --test color_test --test theme_test` → pass.
- [ ] **Step 5: Mutation proof:** anchor `.all(|b| b.is_ascii_hexdigit())` → `.all(|_| true)` → exit 0.
- [ ] **Step 6:** gate; commit `fix(ui): ignore a hex colour with non-ASCII characters instead of panicking`.

---

### Task 6: Draw suggestion text without control characters (I6)

#### What binds this task
- Decided: in `src/ui/theme.rs`, a private `fn inert(text: &str) -> Cow<str>` replaces every char `c` with `c < ' ' || c == '\u{7f}' || ('\u{80}'..='\u{9f}').contains(&c)` by `'?'`; `format_suggestion_line_with_theme_and_min_width` uses `inert(&sug.display)` and `inert(desc)` everywhere it used `sug.display` / `desc`. Icons and prefixes come from the user's config and are not filtered. The inserted text (`name`) is already filtered by `is_insertable`.

#### The measured evidence of the defect
A JSON property value `ESC[1;1HPWNED` completed with `$j.a<Tab>` wrote `PWNED` at row 0, still there after Esc; a carapace description with `ESC]52;c;...BEL` reached the terminal verbatim.

#### Files
- Modify: `src/ui/theme.rs`
- Test: `tests/theme_test.rs`

- [ ] **Step 1: Failing test:**

```rust
#[test]
fn test_control_characters_in_suggestion_text_are_drawn_inert() {
    let theme = Theme::default();
    let sug = Suggestion::new(
        "a",
        "a\u{1b}[1;1HPWNED",
        Some("x\u{1b}]52;c;aWV4\u{7}y\u{9b}2Jz".to_string()),
        50,
    );
    for selected in [false, true] {
        let line = format_suggestion_line_with_theme(&sug, selected, 80, &theme);
        assert!(!line.contains("\u{1b}[1;1H"), "{line:?}");
        assert!(!line.contains("\u{1b}]52"), "{line:?}");
        assert!(!line.contains('\u{7}'), "{line:?}");
        assert!(!line.contains('\u{9b}'), "{line:?}");
        assert!(line.contains("PWNED"), "{line:?}");
    }
}
```

- [ ] **Step 2:** `cargo test -q --test theme_test control_characters` → FAIL.
- [ ] **Step 3: Implement** as decided.
- [ ] **Step 4:** `cargo test -q --test theme_test --test renderer_test` → pass.
- [ ] **Step 5: Mutation proof:** in `inert`, anchor the C1 range `('\u{80}'..='\u{9f}').contains(&c)` → `false` → `cargo test -q --test theme_test control_characters` exit 0.
- [ ] **Step 6:** gate; commit `fix(ui): draw control characters in suggestion text as '?'`.

---

### Task 7: Tests for documented rules no test could fail (I12, I13, I15, I16, I18, I19)

Test-only except I13, which needs a seam. Each rule gets one test and one mutation proof; all six are independent.

- [ ] **I12 — unsupported shell exits 2.** `tests/cli_test.rs`:

```rust
#[test]
fn test_unsupported_shell_exits_with_code_2() {
    let out = run_with_session_env(&["--shell", "bash"], false);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unsupported shell \"bash\""));

    let config = std::env::temp_dir().join(format!("sp_bash_{}.toml", std::process::id()));
    std::fs::write(&config, "shell = \"bash\"\n").unwrap();
    let out = run_with_session_env(&["--config", config.to_str().unwrap()], false);
    let _ = std::fs::remove_file(&config);
    assert_eq!(out.status.code(), Some(2));
}
```
Mutation: `src/main.rs` anchor `std::process::exit(2);` → `std::process::exit(1);` → `cargo test -q --test cli_test unsupported_shell` exit 0.

- [ ] **I13 — pwsh.exe on PATH wins.** In `src/pty/shell.rs` split the PATH search into `pub fn detect_shell_in(override_shell: Option<&str>, path_var: Option<&std::ffi::OsStr>) -> ShellType` (same body, using `path_var` instead of `env::var_os("PATH")`); `detect_shell(o)` becomes `detect_shell_in(o, env::var_os("PATH").as_deref())`. Replace the tautological `test_detect_shell_auto` in `tests/pty_test.rs` with:

```rust
#[test]
fn test_detect_shell_prefers_pwsh_on_path() {
    let with = std::env::temp_dir().join(format!("sp_pwsh_{}", std::process::id()));
    let without = std::env::temp_dir().join(format!("sp_nopwsh_{}", std::process::id()));
    std::fs::create_dir_all(&with).unwrap();
    std::fs::create_dir_all(&without).unwrap();
    std::fs::write(with.join("pwsh.exe"), b"").unwrap();

    let path = std::env::join_paths([&without, &with]).unwrap();
    assert_eq!(detect_shell_in(None, Some(&path)), ShellType::Pwsh);
    let path = std::env::join_paths([&without]).unwrap();
    assert_eq!(detect_shell_in(None, Some(&path)), ShellType::Powershell);
    assert_eq!(detect_shell_in(None, None), ShellType::Powershell);
    assert_eq!(detect_shell_in(Some("powershell"), Some(&path)), ShellType::Powershell);

    let _ = std::fs::remove_dir_all(&with);
    let _ = std::fs::remove_dir_all(&without);
}
```
Mutation: anchor `dir.join("pwsh.exe")` → `dir.join("pwsh.com")` → `cargo test -q --test pty_test detect_shell_prefers` exit 0.

- [ ] **I15 — a range after the cursor is rejected.** Append to `test_report_ranges` in `tests/osc_test.rs`:

```rust
    // A range that starts after the cursor cannot be applied either; slicing it would panic.
    let ahead = ShellReport {
        line: "abcdef".into(),
        cursor: 2,
        replacement_index: 3,
        replacement_length: 2,
        matches: vec![],
    };
    assert_eq!(ahead.replacement_range(), None);
```
and in `tests/aggregate_test.rs`:

```rust
#[test]
fn test_plan_replacement_survives_a_range_after_the_cursor() {
    let r = report("abcdef", 2, 3, 2, vec![]);
    let shell = Suggestion::new("xyz", "xyz", None, 70).with_shell_range();
    let _ = plan_replacement(&r, &shell); // must not panic
}
```
Mutation: `src/shell/report.rs` anchor `(start <= cursor && cursor <= end)` → `(cursor <= end)` → `cargo test -q --test aggregate_test survives_a_range` exit 0.

- [ ] **I16 — a user spec replaces the built-in one.** In `test_load_dir_adds_user_specs_and_reports_bad_files` (`tests/engine_test.rs`), before `load_dir`, also write `dir.join("git.json")` with `{"name":"git","subcommands":[{"name":"onlymine"}]}`, and after the existing asserts:

```rust
    let names: Vec<String> = provider.complete("git ", "").await.into_iter().map(|s| s.name).collect();
    assert_eq!(names, vec!["onlymine".to_string()]);
```
Mutation: `src/engine/providers/json_spec.rs` anchor `self.specs.insert(spec.name.clone(), spec);` → `self.specs.entry(spec.name.clone()).or_insert(spec);` → `cargo test -q --test engine_test test_load_dir_adds_user_specs` exit 0.

- [ ] **I18 — `max_suggestions = 0` means 5.** `tests/renderer_test.rs`:

```rust
#[test]
fn test_zero_max_suggestions_means_five_and_still_renders() {
    assert_eq!(SuggestionState::new(0).max_rows, 5);
    let mut state = SuggestionState::new(0);
    state.set_suggestions(many(3));
    let term = HeadlessTerminal::new(80, 24);
    let mut out = Vec::new();
    let layout = Renderer::render_dropdown(&state, &term, &Theme::default(), 0, 0, &mut out).unwrap();
    assert_eq!(layout.map(|l| l.row_count), Some(3));
}
```
Mutation: `src/ui/suggestion_state.rs` anchor `if max_rows == 0 { 5 } else { max_rows }` → `if max_rows == 0 { 0 } else { max_rows }` → `cargo test -q --test renderer_test zero_max_suggestions` exit 0.

- [ ] **I19 — emoji icons are two columns wide.** `tests/theme_test.rs` (import `truncate_to_width`, `SuggestionKind`):

```rust
#[test]
fn test_truncation_counts_emoji_as_two_columns() {
    assert_eq!(truncate_to_width("\u{1f4c1}ab", 3), "\u{1f4c1}a");
    let theme = Theme::default();
    let sug = Suggestion::new("src", "src", Some("a long description".into()), 50)
        .with_kind(SuggestionKind::Directory);
    for selected in [false, true] {
        let line = format_suggestion_line_with_theme(&sug, selected, 10, &theme);
        let mut screen = vt100::Parser::new(1, 40, 0);
        screen.process(line.as_bytes());
        let drawn = screen.screen().contents();
        assert!(
            unicode_width::UnicodeWidthStr::width(drawn.trim_end()) <= 10,
            "{drawn:?}"
        );
    }
}
```
Mutation: `src/ui/theme.rs` anchor `let w = c.width().unwrap_or(0);` → `let w = c.width().unwrap_or(0).min(1);` → `cargo test -q --test theme_test truncation_counts_emoji` exit 0.

- [ ] **Finish:** gate; one commit `test: pin documented rules that no test could fail (I12, I13, I15, I16, I18, I19)`. The report lists each mutation with its exit code.

---

### Task 8: End-to-end tests for the reactor and the session; a stable, history-free e2e suite (I7, I9, I10, I11, I14, flaky dropdown test, M2)

#### What binds this task
- Decided: add `Terminal::warm_completion(&mut self)` to `tests/common/mod.rs`. It runs, inside the session, `$null = [System.Management.Automation.CommandCompletion]::CompleteInput('git ', 4, $null); 'WARM' + 'ED'` and waits for `WARMED`. It loads PowerShell's completion machinery before the first timed Tab. This is not a workaround: shell-panel's 3 s fallback is documented, and a cold runner sometimes needs longer for the first completion. Measured: the release run of v0.1.0 failed at `e2e_binary_test.rs:65` with the screen `> git ` and no dropdown. The same commit had passed minutes earlier, and a rerun passed.
- Every test in `tests/e2e_binary_test.rs` calls `term.quiet_session()` right after the first prompt. Every test that waits for a dropdown or a completion also calls `term.warm_completion()`. That covers M2: those tests no longer write to the user's history, and inline predictions no longer satisfy `git status`.
- New tests go in `tests/e2e_binary_test.rs`, follow its helpers (`temp_dir`, `START`, `STEP`), and build every marker they wait for by concatenation.

- [ ] **Step 1: Harness helper.** In `tests/common/mod.rs`:

```rust
    /// Loads PowerShell's completion machinery before a timed Tab: the first completion in a
    /// fresh session can take longer than shell-panel's 3 s report timeout on a cold machine.
    pub fn warm_completion(&mut self) {
        self.send(
            b"$null = [System.Management.Automation.CommandCompletion]::CompleteInput('git ', 4, $null); 'WARM' + 'ED'\r",
        );
        assert!(
            self.wait_for_text("WARMED", Duration::from_secs(30)),
            "warm_completion did not finish: {}",
            self.screen()
        );
    }
```

- [ ] **Step 2: Existing tests.** Insert `term.quiet_session();` after the first `wait_for_text("PS ", START)` assertion of every test in `tests/e2e_binary_test.rs`. Insert `term.warm_completion();` right after it in `test_tab_inserts_single_match_opens_dropdown_and_exit_code_propagates`, `test_completion_uses_the_real_line_and_the_real_session` and `test_enter_accepts_the_highlighted_suggestion_without_running_the_line`. Some later waits may then match stale text that `quiet_session` or `warm_completion` left on screen. Check each wait still waits for something typed after them (for example `"> git"`). Report any wait you had to change.

- [ ] **Step 3: New end-to-end tests** (append to `tests/e2e_binary_test.rs`):

```rust
#[test]
fn test_cursor_is_visible_after_exit() {
    // I7: the raw-mode guard must show the cursor again on the way out.
    let dir = temp_dir("cursor");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
    let last_show_or_hide = term
        .raw
        .windows(6)
        .rposition(|w| w == b"\x1b[?25h" || w == b"\x1b[?25l")
        .map(|i| &term.raw[i..i + 6]);
    assert_eq!(last_show_or_hide, Some(&b"\x1b[?25h"[..]));
}

#[test]
fn test_tab_falls_back_to_powershell_when_no_report_arrives() {
    // I9: without the chord handler no report comes; after 3 s the Tab goes to PowerShell.
    let dir = temp_dir("fallback");
    std::fs::write(dir.join("zz_unique_file.txt"), "x").unwrap();
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();
    term.send(b"Remove-PSReadLineKeyHandler -Chord 'Ctrl+Alt+Shift+F12'; 'UNBO' + 'UND'\r");
    assert!(term.wait_for_text("UNBOUND", STEP), "screen: {}", term.screen());
    term.send(b"echo zz_uni");
    assert!(term.wait_for_text("echo zz_uni", STEP));
    term.send(b"\t");
    assert!(
        term.wait_for_text("zz_unique_file.txt", Duration::from_secs(20)),
        "screen: {}",
        term.screen()
    );
    term.send(b"\x1b");
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_key_typed_before_the_report_keeps_the_tab() {
    // I10: Tab then Q in one write; the withheld Tab is replayed before the Q.
    let dir = temp_dir("withheld");
    std::fs::write(dir.join("zz_unique_file.txt"), "x").unwrap();
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();
    term.send(b"echo zz_uni");
    assert!(term.wait_for_text("echo zz_uni", STEP));
    term.send(b"\tQ");
    assert!(
        term.wait_for_text("zz_unique_file.txtQ", Duration::from_secs(20)),
        "screen: {}",
        term.screen()
    );
    term.send(b"\x1b");
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_reserved_chord_never_reaches_a_running_program() {
    // I11: while a program reads keys, Tab is a plain Tab, never Ctrl+Alt+Shift+F12.
    let dir = temp_dir("readkey");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();
    term.send(b"'READY' + 'KEY'; $k = [Console]::ReadKey($true); 'GOT-' + $k.Key + '-' + $k.Modifiers\r");
    assert!(term.wait_for_text("READYKEY", STEP), "screen: {}", term.screen());
    term.send(b"\t");
    assert!(
        term.wait_until(STEP, |t| t.screen().contains("GOT-")
            && t.screen().lines().any(|l| l.trim_start().starts_with("GOT-"))),
        "screen: {}",
        term.screen()
    );
    let got = term
        .screen()
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("GOT-"))
        .unwrap()
        .to_string();
    assert_eq!(got, "GOT-Tab-0", "screen: {}", term.screen());
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_session_marks_itself_for_nested_start_detection() {
    // I14: the child shell sees SHELL_PANEL_SESSION=1, which --check and nested starts rely on.
    let dir = temp_dir("sessionenv");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START), "screen: {}", term.screen());
    term.quiet_session();
    term.send(b"'SPS=' + $env:SHELL_PANEL_SESSION\r");
    assert!(
        term.wait_until(STEP, |t| t.screen().lines().any(|l| l.trim() == "SPS=1")),
        "screen: {}",
        term.screen()
    );
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}
```

`[ConsoleModifiers]` prints as `0` when no modifier is set: PowerShell's `+` on a string converts the enum to its numeric value. If the base run prints `GOT-Tab-None` instead, use that literal and report it. The test fails either way when the chord leaks (`GOT-F12-...`).

- [ ] **Step 4: Run** `cargo test -q --test e2e_binary_test -- --test-threads=1`. Expected: all pass. If one fails, rerun it once and report both runs.

- [ ] **Step 5: Mutation proofs.** Each must exit 0 and is run on the final bytes:
```
... -Path src/io/raw_mode.rs -Anchor 'b"\x1b[?25h"' -Replacement 'b"\x1b[?25l"' -TestCommand 'cargo test -q --test e2e_binary_test test_cursor_is_visible_after_exit -- --test-threads=1'
... -Path src/core/app.rs -Anchor '                    command_state.abandon_report();
                    write_to_pty(&mut pty_writer, b"\t");' -Replacement '                    command_state.abandon_report();' -TestCommand 'cargo test -q --test e2e_binary_test test_tab_falls_back -- --test-threads=1'
... -Path src/core/app.rs -Anchor 'withheld_tab_bytes(tab_pending, key_event.code)' -Replacement 'withheld_tab_bytes(false, key_event.code)' -TestCommand 'cargo test -q --test e2e_binary_test test_key_typed_before_the_report -- --test-threads=1'
... -Path src/core/app.rs -Anchor '            && command_state.reading_line' -Replacement '            && (command_state.reading_line || true)' -TestCommand 'cargo test -q --test e2e_binary_test test_reserved_chord_never_reaches -- --test-threads=1'
... -Path src/pty/conpty.rs -Anchor 'cmd.env(SESSION_ENV, "1");' -Replacement 'cmd.env(SESSION_ENV, "0");' -TestCommand 'cargo test -q --test e2e_binary_test test_session_marks_itself -- --test-threads=1'
```
Copy anchors from the files if whitespace differs. Read the timeout-branch lines of `app.rs` first, because Task 2 of the previous plan reordered them.

- [ ] **Step 6:** run the gate. Make one commit: `test: cover the reactor and session rules end to end; warm completion and isolate history in e2e tests`.

---

## After the tasks

- Push `main` after the final review. CI runs the whole suite; the next release tag no longer depends on a warm runner.
- Still deferred: the Minor findings of the review and the items listed under "Deferred from the execution of this plan" in `2026-10-01-review-critical-fixes.md` and in the release-installer plan's final review.
