# Fix Shell-Panel UX, Tab-Only Completion & Key Handling Implementation Plan

> **Status:** executed — see the git history for the resulting commits. Checkbox state below was not maintained during execution. Parts of the design were later replaced: see `2026-09-21-review-fixes.md`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix real-world usability bugs in `shell-panel`: disable conflicting PSReadLine predictions, fix `\\?\` path blocking of `shellIntegration.ps1`, fix Backspace deleting whole words, implement full xterm modifiers for Ctrl+Shift text selection, switch autocomplete triggering to Tab-only (eliminating typing lag), add icons to suggestion items, and ensure subcommands take precedence over file listings.

**Architecture:**
1. **Shell Integration & Execution Policy:** Strip `\\?\` UNC prefix when canonicalizing paths, add `-ExecutionPolicy Bypass` in ConPTY launcher, and disable PSReadLine predictions (`Set-PSReadLineOption -PredictionSource None`) inside `assets/shellIntegration.ps1`.
2. **Terminal Key Event Transcoding:** Map `KeyCode::Backspace` to `b"\x7f"` (ASCII DEL for PowerShell `BackwardDeleteChar`), and implement compound xterm modifier calculation (`1 + shift + alt*2 + ctrl*4`) for arrows, Home, End, PageUp, PageDown.
3. **Tab-Only Reactor Loop:** Remove background debounced query loop on keystrokes/output. Query providers and render dropdown ONLY when <kbd>Tab</kbd> is pressed (or accept active item if dropdown is already visible). Normal typing passes through with zero latency.
4. **Icons & Category Badges:** Add `icon: Option<&'static str>` or category enum to `Suggestion`, and format items in `theme.rs` with `📁 `, `📄 `, `⚡ `, `🔹 `, `🏷️ `, `>_ `, `🔗 `.
5. **Completion Priority & Provider Filtering:** Gate `FileProvider` so it doesn't pollute subcommands, and sort suggestions strictly by priority (subcommands 80 > flags 75 > directories 60 > files 50).

**Tech Stack:** Rust 1.80+, Tokio, Crossterm, VT100 headless, ConPTY, PowerShell 7 / Windows PowerShell.

---

### Task 1: Fix Path Canonicalization & PSReadLine Prediction Conflict

**Files:**
- Modify: `assets/shellIntegration.ps1`
- Modify: `src/core/app.rs:35-50`
- Modify: `src/pty/conpty.rs:25-40`
- Test: `tests/integration_script_test.rs`

- [ ] **Step 1: Write integration test for canonical path without `\\?\` and script execution**

Create `tests/integration_script_test.rs`:
```rust
use shell_panel::core::app::get_shell_integration_path;

#[test]
fn test_shell_integration_path_has_no_verbatim_prefix() {
    let path = get_shell_integration_path().expect("Should resolve path");
    let path_str = path.to_string_lossy();
    assert!(!path_str.starts_with(r"\\?\"), "Path must not contain \\?\\ prefix: {}", path_str);
    assert!(path.is_file(), "File must exist: {:?}", path);
}
```

- [ ] **Step 2: Run test to verify it fails with `\\?\` prefix on Windows**

Run: `cargo test --test integration_script_test`
Expected: FAIL if `canonicalize()` returned `\\?\...`.

- [ ] **Step 3: Update `get_shell_integration_path`, `conpty.rs` and `shellIntegration.ps1`**

1. In `src/core/app.rs`:
```rust
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
```

2. In `src/pty/conpty.rs`:
Add `-ExecutionPolicy Bypass`:
```rust
let mut cmd = CommandBuilder::new(shell_type.executable_name());
cmd.env("ISTERM", "1");
cmd.env("TERM", "xterm-256color");
cmd.arg("-NoLogo");
cmd.arg("-ExecutionPolicy");
cmd.arg("Bypass");
cmd.arg("-NoExit");
cmd.arg("-Command");
```

3. In `assets/shellIntegration.ps1`:
Add PSReadLine prediction disable:
```powershell
try {
    Set-PSReadLineOption -PredictionSource None
} catch {}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test integration_script_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add assets/shellIntegration.ps1 src/core/app.rs src/pty/conpty.rs tests/integration_script_test.rs
git commit -m "fix(pty): strip verbatim path prefix, bypass execution policy and disable psreadline predictions"
```

---

### Task 2: Fix Backspace and Full Xterm Modifiers for Ctrl/Shift Selection

**Files:**
- Modify: `src/io/key_event.rs:65-115`
- Test: `tests/io_test.rs`

- [ ] **Step 1: Add failing unit tests for Backspace (0x7f) and compound modifiers (Ctrl+Shift)**

In `tests/io_test.rs`:
```rust
#[test]
fn test_backspace_emits_del_0x7f() {
    let ev = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(encode_key_event(&ev), vec![0x7f]);
}

#[test]
fn test_ctrl_shift_arrows_emit_xterm_code_6() {
    let right_ctrl_shift = KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(encode_key_event(&right_ctrl_shift), b"\x1b[1;6C".to_vec());

    let left_ctrl_shift = KeyEvent::new(
        KeyCode::Left,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(encode_key_event(&left_ctrl_shift), b"\x1b[1;6D".to_vec());

    let home_shift = KeyEvent::new(KeyCode::Home, KeyModifiers::SHIFT);
    assert_eq!(encode_key_event(&home_shift), b"\x1b[1;2H".to_vec());

    let end_ctrl_shift = KeyEvent::new(
        KeyCode::End,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(encode_key_event(&end_ctrl_shift), b"\x1b[1;6F".to_vec());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test io_test`
Expected: FAIL (backspace returned 0x08, Ctrl+Shift returned code 5 instead of 6)

- [ ] **Step 3: Implement xterm modifier helper and update key encoding**

In `src/io/key_event.rs`:
```rust
fn xterm_modifier_code(modifiers: KeyModifiers) -> u8 {
    let mut code = 1u8;
    if modifiers.contains(KeyModifiers::SHIFT) {
        code += 1;
    }
    if modifiers.contains(KeyModifiers::ALT) {
        code += 2;
    }
    if modifiers.contains(KeyModifiers::CONTROL) {
        code += 4;
    }
    code
}
```
Update `encode_key_event`:
- `KeyCode::Backspace => vec![0x7f],`
- For `Up`, `Down`, `Right`, `Left`, `Home`, `End`:
  If `mod_code > 1`: format `\x1b[1;{mod_code}{A|B|C|D|H|F}`.
  If `mod_code == 1`: emit standard `\x1b[{A|B|C|D|H|F}`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test io_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add src/io/key_event.rs tests/io_test.rs
git commit -m "fix(io): map backspace to 0x7f and implement compound xterm modifiers for selection"
```

---

### Task 3: Add Icons and Category Formatting to Suggestions

**Files:**
- Modify: `src/engine/provider.rs`
- Modify: `src/ui/theme.rs`
- Modify: `src/engine/providers/files.rs`
- Modify: `src/engine/providers/json_spec.rs`
- Modify: `src/engine/providers/carapace.rs`
- Modify: `src/engine/providers/zoxide.rs`
- Test: `tests/theme_test.rs`

- [ ] **Step 1: Write test for icon formatting in suggestion labels**

Create `tests/theme_test.rs`:
```rust
use shell_panel::engine::provider::{Suggestion, SuggestionKind};
use shell_panel::ui::theme::format_suggestion_line;

#[test]
fn test_format_suggestion_line_with_icons() {
    let dir_sug = Suggestion::new("src/", "src/", Some("Directory".into()), 60)
        .with_kind(SuggestionKind::Directory);
    let formatted = format_suggestion_line(&dir_sug, false, 40);
    assert!(formatted.contains("📁"), "Must contain folder icon: {}", formatted);

    let cmd_sug = Suggestion::new("commit", "commit", Some("Commit changes".into()), 80)
        .with_kind(SuggestionKind::Subcommand);
    let formatted_cmd = format_suggestion_line(&cmd_sug, true, 40);
    assert!(formatted_cmd.contains("🔹"), "Must contain subcommand icon: {}", formatted_cmd);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test theme_test`
Expected: FAIL (types/methods not yet present)

- [ ] **Step 3: Implement `SuggestionKind` and update `theme.rs`**

1. In `src/engine/provider.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionKind {
    Directory,
    File,
    Command,
    Subcommand,
    Option,
    PowerShellCmdlet,
    Alias,
    Other,
}

impl SuggestionKind {
    pub fn icon(&self) -> &'static str {
        match self {
            SuggestionKind::Directory => "📁 ",
            SuggestionKind::File => "📄 ",
            SuggestionKind::Command => "⚡ ",
            SuggestionKind::Subcommand => "🔹 ",
            SuggestionKind::Option => "🏷️  ",
            SuggestionKind::PowerShellCmdlet => ">_ ",
            SuggestionKind::Alias => "🔗 ",
            SuggestionKind::Other => "  ",
        }
    }
}
```
Add `pub kind: SuggestionKind` to `Suggestion` with default `SuggestionKind::Other`.
2. Set appropriate `kind` in `FileProvider` (Directory vs File), `JsonSpecProvider` (Subcommand vs Option), `ZoxideProvider` (Directory).
3. In `src/ui/theme.rs`: update `format_suggestion_line` to prepend `sug.kind.icon()`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test theme_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add src/engine/provider.rs src/ui/theme.rs src/engine/providers/ tests/theme_test.rs
git commit -m "feat(ui): add icons and category badges to autocompletion suggestions"
```

---

### Task 4: Switch Autocomplete Trigger to Tab-Only & Polish Replacement

**Files:**
- Modify: `src/core/app.rs:635-860`
- Test: `tests/tab_trigger_test.rs`

- [ ] **Step 1: Write integration test for Tab-only trigger and 0x7f replacement backspaces**

Create `tests/tab_trigger_test.rs`:
```rust
use shell_panel::engine::replacement::calculate_replacement;

#[test]
fn test_replacement_action_calculation() {
    let rep = calculate_replacement("c", "commit");
    assert_eq!(rep.backspace_count, 0);
    assert_eq!(rep.insert_text, "ommit ");
}
```

- [ ] **Step 2: Update `App::run` in `src/core/app.rs`**

1. Remove automatic `debounce_deadline` on PTY output.
2. In `ActionKey::AcceptSuggestion` (<kbd>Tab</kbd>):
   - If `!suggestion_state.visible`:
     - Extract `command_text` via `term.extract_command_text(p_row, p_col)`.
     - Query providers (`json_spec`, `zoxide`, `carapace`, `files`).
     - If results empty: pass `\t` to PTY master.
     - If results non-empty:
       - If 1 result: auto-complete immediately!
       - If >1 results: `suggestion_state.set_suggestions(results)`, render dropdown!
   - If `suggestion_state.visible`:
     - Accept current selection, calculate replacement.
     - Send `0x7f` backspaces and `insert_text` to PTY master!
     - Dismiss menu and clear virtual patch.
3. In `ActionKey::Passthrough` (typing keys while menu is open):
   - Dismiss menu, clear dropdown, and forward key to PTY master.

- [ ] **Step 3: Run all unit and integration tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 4: Test interactive `debug_git_completion.rs`**

Run: `cargo test --test debug_git_completion -- --nocapture`
Verify that `prompt_line` and `prompt_end_x` are accurately detected, and `git c` yields `commit`, `clone`, `checkout` with priority 80.

- [ ] **Step 5: Build release binary and commit**

Run: `cargo build --release`
```bash
git add src/core/app.rs tests/tab_trigger_test.rs tests/debug_git_completion.rs
git commit -m "feat(core): trigger suggestions only on tab and use 0x7f backspaces"
```
