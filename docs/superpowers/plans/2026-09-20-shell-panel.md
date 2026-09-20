# shell-panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `shell-panel`, an IDE-style command line autocompletion panel in Rust for Windows PowerShell using ConPTY, non-destructive virtual terminal patching, and modular completion providers.

**Architecture:** A lightweight ConPTY wrapper process that acts as a Man-in-the-Middle between the user's terminal and a child PowerShell process. An OSC 6973 integration script tracks prompt lifecycle and CWD. A headless VT100 emulator mirrors terminal state to generate non-destructive line patches for a floating suggestion dropdown. A modular completion engine aggregates suggestions from embedded JSON specs, the Carapace CLI, Zoxide, and the local filesystem.

**Tech Stack:** Rust 2021, `portable-pty` (ConPTY), `crossterm` (Raw mode & Key parsing), `vt100` (Headless emulator), `tokio` (Async runtime & channels), `serde` / `serde_json`, `unicode-width`, `clap`.

---

### Task 1: Shell Detection & ConPTY Process Spawner

**Files:**
- Create: `src/pty/shell.rs`
- Create: `src/pty/conpty.rs`
- Modify: `src/pty/mod.rs`
- Test: `tests/pty_test.rs`

- [x] **Step 1: Write test for shell detection and ConPTY initialization**

```rust
// tests/pty_test.rs
use shell_panel::pty::shell::{detect_shell, ShellType};

#[test]
fn test_detect_shell() {
    let shell = detect_shell(None);
    assert!(shell == ShellType::Pwsh || shell == ShellType::Powershell);
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --test pty_test`
Expected: FAIL with module/type not found.

- [x] **Step 3: Implement `shell.rs` and `conpty.rs`**

```rust
// src/pty/shell.rs
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Pwsh,
    Powershell,
}

impl ShellType {
    pub fn executable_name(&self) -> &'static str {
        match self {
            ShellType::Pwsh => "pwsh.exe",
            ShellType::Powershell => "powershell.exe",
        }
    }
}

pub fn detect_shell(override_shell: Option<&str>) -> ShellType {
    if let Some(s) = override_shell {
        if s.eq_ignore_ascii_case("powershell") {
            return ShellType::Powershell;
        }
        return ShellType::Pwsh;
    }
    // Check if pwsh.exe is in PATH, fallback to powershell.exe
    if which_in_path("pwsh.exe") {
        ShellType::Pwsh
    } else {
        ShellType::Powershell
    }
}

fn which_in_path(exe: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if dir.join(exe).is_file() {
                return true;
            }
        }
    }
    false
}
```

```rust
// src/pty/conpty.rs
use crate::pty::shell::ShellType;
use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtyPair, PtySize};
use std::path::Path;

pub struct ConPtySession {
    pub pair: PtyPair,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl ConPtySession {
    pub fn spawn(
        shell_type: ShellType,
        cols: u16,
        rows: u16,
        script_path: &Path,
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell_type.executable_name());
        cmd.env("ISTERM", "1");
        cmd.env("TERM", "xterm-256color");

        // PowerShell init arguments
        let script_arg = format!("try {{ . \"{}\" }} catch {{}}", script_path.display());
        cmd.args(["-noexit", "-command", &script_arg]);

        let child = pair.slave.spawn_command(cmd)
            .with_context(|| format!("Falha ao iniciar {}", shell_type.executable_name()))?;

        Ok(Self { pair, child })
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.pair.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }
}
```

```rust
// src/pty/mod.rs
pub mod conpty;
pub mod shell;
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --test pty_test`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/pty/ tests/pty_test.rs
git commit -m "feat(pty): implement shell detection and ConPTY session spawning"
```

---

### Task 2: PowerShell Shell Integration & OSC 6973 Parser

**Files:**
- Create: `src/shell/osc.rs`
- Create: `src/shell/command_state.rs`
- Modify: `src/shell/mod.rs`
- Test: `tests/osc_test.rs`

- [x] **Step 1: Write failing test for OSC 6973 sequence parsing**

```rust
// tests/osc_test.rs
use shell_panel::shell::osc::{parse_osc_sequence, OscEvent};

#[test]
fn test_parse_osc_sequences() {
    assert_eq!(parse_osc_sequence("6973;PS"), Some(OscEvent::PromptStarted));
    assert_eq!(parse_osc_sequence("6973;PE"), Some(OscEvent::PromptEnded));
    assert_eq!(
        parse_osc_sequence("6973;CWD;C:\\Users\\test"),
        Some(OscEvent::Cwd("C:\\Users\\test".to_string()))
    );
    assert_eq!(parse_osc_sequence("1337;Other"), None);
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --test osc_test`
Expected: FAIL with module not found.

- [x] **Step 3: Implement `osc.rs` and `command_state.rs`**

```rust
// src/shell/osc.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscEvent {
    PromptStarted,
    PromptEnded,
    Cwd(String),
}

pub fn parse_osc_sequence(payload: &str) -> Option<OscEvent> {
    if !payload.starts_with("6973;") {
        return None;
    }
    let body = &payload[5..];
    if body == "PS" {
        return Some(OscEvent::PromptStarted);
    }
    if body == "PE" {
        return Some(OscEvent::PromptEnded);
    }
    if let Some(cwd) = body.strip_prefix("CWD;") {
        return Some(OscEvent::Cwd(unescape_cwd(cwd)));
    }
    None
}

fn unescape_cwd(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if chars.peek() == Some(&'\\') {
                chars.next();
                out.push('\\');
            } else if chars.peek() == Some(&'x') {
                chars.next(); // consume 'x'
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    out.push(byte as char);
                }
            } else {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }
    out
}
```

```rust
// src/shell/command_state.rs
use crate::shell::osc::OscEvent;

#[derive(Debug, Default, Clone)]
pub struct CommandState {
    pub prompt_line: Option<u16>,
    pub prompt_end_x: Option<u16>,
    pub cwd: String,
    pub command_text: String,
    pub in_prompt: bool,
    pub has_output: bool,
}

impl CommandState {
    pub fn handle_osc(&mut self, event: OscEvent, current_cursor_y: u16, current_cursor_x: u16) {
        match event {
            OscEvent::PromptStarted => {
                self.in_prompt = true;
                self.has_output = false;
                self.prompt_line = Some(current_cursor_y);
                self.prompt_end_x = None;
                self.command_text.clear();
            }
            OscEvent::PromptEnded => {
                self.in_prompt = false;
                self.prompt_end_x = Some(current_cursor_x);
            }
            OscEvent::Cwd(cwd) => {
                self.cwd = cwd;
            }
        }
    }
}
```

```rust
// src/shell/mod.rs
pub mod command_state;
pub mod osc;
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --test osc_test`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/shell/ tests/osc_test.rs
git commit -m "feat(shell): implement OSC 6973 parser and command state tracker"
```

---

### Task 3: Headless VT100 Emulator & Ghost Text Filtering

**Files:**
- Create: `src/vt/emulator.rs`
- Create: `src/vt/cpr.rs`
- Modify: `src/vt/mod.rs`
- Test: `tests/vt_test.rs`

- [x] **Step 1: Write failing test for headless terminal and CPR query detection**

```rust
// tests/vt_test.rs
use shell_panel::vt::cpr::has_cpr_query;
use shell_panel::vt::emulator::HeadlessTerminal;

#[test]
fn test_vt_headless_basic() {
    let mut vt = HeadlessTerminal::new(80, 24);
    vt.process(b"Hello from shell");
    assert_eq!(vt.cursor_position(), (16, 0));
}

#[test]
fn test_cpr_query() {
    assert!(has_cpr_query(b"\x1b[6n"));
    assert!(!has_cpr_query(b"normal text"));
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --test vt_test`
Expected: FAIL with module not found.

- [x] **Step 3: Implement `emulator.rs` and `cpr.rs`**

```rust
// src/vt/cpr.rs
pub fn has_cpr_query(data: &[u8]) -> bool {
    data.windows(4).any(|w| w == b"\x1b[6n" || w == b"\x1b[?6n")
}
```

```rust
// src/vt/emulator.rs
use vt100::Parser;

pub struct HeadlessTerminal {
    parser: Parser,
    pub cols: u16,
    pub rows: u16,
}

impl HeadlessTerminal {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            parser: Parser::new(rows, cols, 0),
            cols,
            rows,
        }
    }

    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn cursor_position(&self) -> (u16, u16) {
        let screen = self.parser.screen();
        let (row, col) = screen.cursor_position();
        (col, row)
    }

    pub fn is_alternate_buffer(&self) -> bool {
        self.parser.screen().alternate_buffer()
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.parser.set_size(rows, cols);
    }

    /// Extracts text between prompt_end_x and current cursor, filtering PSReadLine ghost text
    pub fn extract_command_text(&self, prompt_row: u16, prompt_end_x: u16) -> String {
        let screen = self.parser.screen();
        let (cursor_col, cursor_row) = screen.cursor_position();
        if cursor_row < prompt_row {
            return String::new();
        }

        let mut result = String::new();
        for row in prompt_row..=cursor_row {
            let start_col = if row == prompt_row { prompt_end_x } else { 0 };
            let end_col = if row == cursor_row { cursor_col } else { self.cols };

            for col in start_col..end_col {
                if let Some(cell) = screen.cell(row, col) {
                    // Ignore PSReadLine ghost text (dim / italic or dull gray)
                    let is_ghost = cell.dim() || cell.italic();
                    if !is_ghost {
                        let ch = cell.contents();
                        if !ch.is_empty() {
                            result.push_str(ch);
                        } else {
                            result.push(' ');
                        }
                    }
                }
            }
        }
        result.trim_end().to_string()
    }
}
```

```rust
// src/vt/mod.rs
pub mod cpr;
pub mod emulator;
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --test vt_test`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/vt/ tests/vt_test.rs
git commit -m "feat(vt): implement headless VT100 emulator and CPR query filtering"
```

---

### Task 4: StdioProxy & Keyboard Event Router

**Files:**
- Create: `src/io/raw_mode.rs`
- Create: `src/io/key_event.rs`
- Create: `src/io/filter.rs`
- Modify: `src/io/mod.rs`
- Test: `tests/io_test.rs`

- [x] **Step 1: Write failing test for key classification**

```rust
// tests/io_test.rs
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use shell_panel::io::key_event::{classify_key, ActionKey};

#[test]
fn test_classify_keys() {
    let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(classify_key(&down), ActionKey::MenuDown);

    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(classify_key(&tab), ActionKey::AcceptSuggestion);

    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(classify_key(&esc), ActionKey::DismissMenu);

    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(classify_key(&enter), ActionKey::Passthrough);
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --test io_test`
Expected: FAIL with module not found.

- [x] **Step 3: Implement `raw_mode.rs`, `key_event.rs` and `filter.rs`**

```rust
// src/io/raw_mode.rs
use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

pub struct RawModeGuard {
    active: bool,
}

impl RawModeGuard {
    pub fn enter() -> Result<Self> {
        enable_raw_mode()?;
        Ok(Self { active: true })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
        }
    }
}
```

```rust
// src/io/key_event.rs
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum ActionKey {
    MenuUp,
    MenuDown,
    AcceptSuggestion,
    DismissMenu,
    Passthrough,
}

pub fn classify_key(event: &KeyEvent) -> ActionKey {
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        return ActionKey::Passthrough;
    }
    match event.code {
        KeyCode::Up => ActionKey::MenuUp,
        KeyCode::Down => ActionKey::MenuDown,
        KeyCode::Tab => ActionKey::AcceptSuggestion,
        KeyCode::Esc => ActionKey::DismissMenu,
        _ => ActionKey::Passthrough,
    }
}
```

```rust
// src/io/filter.rs
/// Strip Kitty keyboard protocol & Win32 input mode upgrades
pub fn sanitize_output_stream(data: &[u8]) -> Vec<u8> {
    // Pass-through clean bytes, strip \x1b[?9001h and \x1b[?u sequences
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i..].starts_with(b"\x1b[?9001h") || data[i..].starts_with(b"\x1b[?9001l") {
            i += 8;
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}
```

```rust
// src/io/mod.rs
pub mod filter;
pub mod key_event;
pub mod raw_mode;
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --test io_test`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/io/ tests/io_test.rs
git commit -m "feat(io): implement raw mode guard, key event classifier and protocol filter"
```

---

### Task 5: PowerShell-aware CLI Lexer

**Files:**
- Create: `src/engine/lexer.rs`
- Modify: `src/engine/mod.rs`
- Test: `tests/lexer_test.rs`

- [ ] **Step 1: Write failing test for PowerShell token lexing**

```rust
// tests/lexer_test.rs
use shell_panel::engine::lexer::{lex_command_line, CommandToken};

#[test]
fn test_lex_command_line() {
    let tokens = lex_command_line("git commit -m \"feat: test\"");
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0].text, "git");
    assert_eq!(tokens[1].text, "commit");
    assert_eq!(tokens[2].text, "-m");
    assert!(tokens[2].is_option);
    assert_eq!(tokens[3].text, "feat: test");
    assert!(tokens[3].complete);

    let incomplete = lex_command_line("git stat");
    assert_eq!(incomplete.len(), 2);
    assert_eq!(incomplete[1].text, "stat");
    assert!(!incomplete[1].complete);
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --test lexer_test`
Expected: FAIL with module not found.

- [x] **Step 3: Implement `lexer.rs`**

```rust
// src/engine/lexer.rs
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandToken {
    pub text: String,
    pub width: usize,
    pub complete: bool,
    pub is_option: bool,
}

pub fn lex_command_line(input: &str) -> Vec<CommandToken> {
    // Focus on the last command after pipeline or delimiters: |, &&, ||, ;
    let last_cmd = input
        .split(['|', ';'])
        .last()
        .unwrap_or("")
        .trim_start();

    if last_cmd.is_empty() {
        return Vec::new();
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;
    let mut chars = last_cmd.chars().peekable();

    while let Some(c) = chars.next() {
        match in_quote {
            Some(q) => {
                if c == q {
                    in_quote = None;
                } else if c == '`' && (q == '"') {
                    if let Some(next_c) = chars.next() {
                        current.push(next_c);
                    }
                } else {
                    current.push(c);
                }
            }
            None => {
                if c == '\'' || c == '"' {
                    in_quote = Some(c);
                } else if c.is_whitespace() {
                    if !current.is_empty() {
                        let is_option = current.starts_with('-');
                        let width = current.as_str().width();
                        tokens.push(CommandToken {
                            text: std::mem::take(&mut current),
                            width,
                            complete: true,
                            is_option,
                        });
                    }
                } else {
                    current.push(c);
                }
            }
        }
    }

    if !current.is_empty() {
        let is_option = current.starts_with('-');
        let width = current.as_str().width();
        let complete = last_cmd.ends_with(' ');
        tokens.push(CommandToken {
            text: current,
            width,
            complete,
            is_option,
        });
    } else if last_cmd.ends_with(' ') {
        tokens.push(CommandToken {
            text: String::new(),
            width: 0,
            complete: false,
            is_option: false,
        });
    }

    tokens
}
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --test lexer_test`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/engine/lexer.rs tests/lexer_test.rs
git commit -m "feat(engine): implement PowerShell-aware command line token lexer"
```

---

### Task 6: Completion Providers & Replacement Engine

**Files:**
- Create: `src/engine/provider.rs`
- Create: `src/engine/replacement.rs`
- Create: `src/engine/providers/files.rs`
- Create: `src/engine/providers/json_spec.rs`
- Create: `src/engine/providers/carapace.rs`
- Create: `src/engine/providers/zoxide.rs`
- Modify: `src/engine/mod.rs`
- Test: `tests/engine_test.rs`

- [ ] **Step 1: Write failing test for replacement calculations**

```rust
// tests/engine_test.rs
use shell_panel::engine::replacement::calculate_replacement;

#[test]
fn test_calculate_replacement() {
    let action = calculate_replacement("stat", "status");
    assert_eq!(action.backspace_count, 0);
    assert_eq!(action.insert_text, "us ");

    let action_fuzzy = calculate_replacement("cmit", "commit");
    assert_eq!(action_fuzzy.backspace_count, 4);
    assert_eq!(action_fuzzy.insert_text, "commit ");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test engine_test`
Expected: FAIL with module not found.

- [ ] **Step 3: Implement `replacement.rs`, `provider.rs` and modular providers**

```rust
// src/engine/replacement.rs
#[derive(Debug, PartialEq, Eq)]
pub struct ReplacementAction {
    pub backspace_count: usize,
    pub insert_text: String,
}

pub fn calculate_replacement(typed: &str, suggestion: &str) -> ReplacementAction {
    let target = format!("{} ", suggestion);
    if target.starts_with(typed) {
        let remaining = &target[typed.len()..];
        ReplacementAction {
            backspace_count: 0,
            insert_text: remaining.to_string(),
        }
    } else {
        ReplacementAction {
            backspace_count: typed.chars().count(),
            insert_text: target,
        }
    }
}
```

```rust
// src/engine/provider.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub name: String,
    pub display: String,
    pub description: Option<String>,
    pub priority: u32,
}

#[async_trait]
pub trait CompletionProvider: Send + Sync {
    fn can_handle(&self, cmd: &str) -> bool;
    async fn complete(&self, cmd_line: &str, cwd: &str) -> Vec<Suggestion>;
}
```

```rust
// src/engine/providers/files.rs
use crate::engine::provider::{CompletionProvider, Suggestion};
use async_trait::async_trait;
use std::path::Path;

pub struct FileProvider;

#[async_trait]
impl CompletionProvider for FileProvider {
    fn can_handle(&self, _cmd: &str) -> bool {
        true
    }

    async fn complete(&self, cmd_line: &str, cwd: &str) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        let target_dir = Path::new(cwd);
        if let Ok(entries) = std::fs::read_dir(target_dir) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    let display = if is_dir { format!("{}/", name) } else { name.clone() };
                    suggestions.push(Suggestion {
                        name: display.clone(),
                        display,
                        description: if is_dir { Some("Directory".to_string()) } else { None },
                        priority: 50,
                    });
                }
            }
        }
        suggestions
    }
}
```

```rust
// src/engine/mod.rs
pub mod lexer;
pub mod provider;
pub mod providers;
pub mod replacement;
```

```rust
// src/engine/providers/mod.rs
pub mod files;
pub mod json_spec;
pub mod carapace;
pub mod zoxide;
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --test engine_test`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/engine/ tests/engine_test.rs
git commit -m "feat(engine): implement completion provider trait, file completer and replacement calculator"
```

---

### Task 7: Non-Destructive Virtual Patching TUI Renderer

**Files:**
- Create: `src/ui/suggestion_state.rs`
- Create: `src/ui/theme.rs`
- Create: `src/ui/patch.rs`
- Create: `src/ui/renderer.rs`
- Modify: `src/ui/mod.rs`
- Test: `tests/renderer_test.rs`

- [x] **Step 1: Write failing test for suggestion state pagination and layout calculation**

```rust
// tests/renderer_test.rs
use shell_panel::engine::provider::Suggestion;
use shell_panel::ui::suggestion_state::SuggestionState;

#[test]
fn test_suggestion_state_navigation() {
    let mut state = SuggestionState::default();
    let suggestions = (0..10)
        .map(|i| Suggestion {
            name: format!("item{}", i),
            display: format!("item{}", i),
            description: None,
            priority: 50,
        })
        .collect();

    state.set_suggestions(suggestions);
    assert_eq!(state.active_index(), 0);
    state.move_down();
    assert_eq!(state.active_index(), 1);
    state.move_up();
    assert_eq!(state.active_index(), 0);
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --test renderer_test`
Expected: FAIL with module not found.

- [x] **Step 3: Implement `suggestion_state.rs`, `theme.rs`, `patch.rs` and `renderer.rs`**

```rust
// src/ui/suggestion_state.rs
use crate::engine::provider::Suggestion;

#[derive(Default, Debug, Clone)]
pub struct SuggestionState {
    suggestions: Vec<Suggestion>,
    active_idx: usize,
    pub visible: bool,
}

impl SuggestionState {
    pub fn set_suggestions(&mut self, list: Vec<Suggestion>) {
        self.suggestions = list;
        self.active_idx = 0;
        self.visible = !self.suggestions.is_empty();
    }

    pub fn active_index(&self) -> usize {
        self.active_idx
    }

    pub fn move_down(&mut self) {
        if !self.suggestions.is_empty() {
            self.active_idx = (self.active_idx + 1) % self.suggestions.len();
        }
    }

    pub fn move_up(&mut self) {
        if !self.suggestions.is_empty() {
            if self.active_idx == 0 {
                self.active_idx = self.suggestions.len() - 1;
            } else {
                self.active_idx -= 1;
            }
        }
    }

    pub fn active_item(&self) -> Option<&Suggestion> {
        self.suggestions.get(self.active_idx)
    }

    pub fn visible_page(&self, max_rows: usize) -> Vec<(&Suggestion, bool)> {
        if self.suggestions.is_empty() {
            return Vec::new();
        }
        let page = self.active_idx / max_rows;
        let start = page * max_rows;
        let end = (start + max_rows).min(self.suggestions.len());

        self.suggestions[start..end]
            .iter()
            .enumerate()
            .map(|(idx, s)| (s, (start + idx) == self.active_idx))
            .collect()
    }
}
```

```rust
// src/ui/patch.rs
pub struct LinePatch {
    pub start_x: u16,
    pub length: u16,
    pub content: String,
}
```

```rust
// src/ui/renderer.rs
use crate::ui::patch::LinePatch;
use crate::ui::suggestion_state::SuggestionState;
use crate::vt::emulator::HeadlessTerminal;
use std::io::{stdout, Write};

pub struct Renderer;

impl Renderer {
    pub fn render_dropdown(
        state: &SuggestionState,
        term: &HeadlessTerminal,
        cursor_x: u16,
        cursor_y: u16,
    ) {
        if !state.visible {
            return;
        }

        let mut out = stdout().lock();
        // Save cursor position
        let _ = write!(out, "\x1b[?25l\x1b[s");

        let page = state.visible_page(5);
        for (i, (sug, selected)) in page.iter().enumerate() {
            let row_offset = (i as u16) + 1;
            let target_y = cursor_y + row_offset;
            if target_y >= term.rows {
                break;
            }

            // Move to target row and column
            let _ = write!(out, "\x1b[{};{}H", target_y + 1, cursor_x + 1);
            if *selected {
                let _ = write!(out, "\x1b[7m> {:<30}\x1b[0m", sug.display);
            } else {
                let _ = write!(out, "  {:<30}", sug.display);
            }
        }

        // Restore cursor position
        let _ = write!(out, "\x1b[u\x1b[?25h");
        let _ = out.flush();
    }

    pub fn clear_dropdown(lines_count: u16, cursor_y: u16, term: &HeadlessTerminal) {
        let mut out = stdout().lock();
        let _ = write!(out, "\x1b[?25l\x1b[s");
        for i in 1..=lines_count {
            let target_y = cursor_y + i;
            if target_y < term.rows {
                let _ = write!(out, "\x1b[{};1H\x1b[2K", target_y + 1);
            }
        }
        let _ = write!(out, "\x1b[u\x1b[?25h");
        let _ = out.flush();
    }
}
```

```rust
// src/ui/mod.rs
pub mod patch;
pub mod renderer;
pub mod suggestion_state;
pub mod theme;
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --test renderer_test`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/ui/ tests/renderer_test.rs
git commit -m "feat(ui): implement suggestion state, pagination and virtual patch renderer"
```

---

### Task 8: App Reactor Loop & End-to-End Orchestration

**Files:**
- Modify: `src/core/app.rs`
- Modify: `src/main.rs`
- Test: `tests/e2e_pty_test.rs`

- [x] **Step 1: Wire App loop multiplexing Stdio, PTY output and Suggestions**

Integrate:
1. ConPTY spawn of PowerShell with `assets/shellIntegration.ps1`.
2. Raw mode on host `stdin`.
3. Read PTY bytes $\to$ Feed into `HeadlessTerminal` $\to$ Print to host `stdout`.
4. Monitor keystrokes:
   - When <kbd>Down</kbd>/<kbd>Up</kbd>: navigate suggestions.
   - When <kbd>Tab</kbd>: calculate replacement, send backspaces + suggestion to PTY.
   - When <kbd>Esc</kbd>: dismiss menu.
   - Other keys: forward directly to PTY.

- [x] **Step 2: Run `cargo check` and `cargo build`**

Run: `cargo build`
Expected: PASS without errors.

- [x] **Step 3: Commit**

```bash
git add src/core/app.rs src/main.rs
git commit -m "feat(core): orchestrate reactor loop connecting PTY, headless VT and suggestion UI"
```
