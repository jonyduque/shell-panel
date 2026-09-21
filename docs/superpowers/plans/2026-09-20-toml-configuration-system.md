# TOML Configuration System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable full customization of colors, icons, max suggestions, and shell preferences via a TOML configuration file defaulting to `~/.config/shell-panel.toml` (with `--config` CLI flag override).

**Architecture:**
1. **Config Data Models (`src/core/config.rs`):**
   - Struct `Config` holding `max_suggestions: usize`, `shell: Option<String>`, `colors: ColorConfig`, `icons: IconConfig`.
   - Struct `ColorConfig`: customizable selected background (`selected_bg`), selected foreground (`selected_fg`), unselected foreground (`unselected_fg`), description color (`description_fg`), and selected prefix (`selected_prefix`).
   - Struct `IconConfig`: customizable icon strings for `directory`, `file`, `command`, `subcommand`, `option`, `powershell_cmdlet`, `alias`, and `other`.
   - Helper `Config::load_or_default(custom_path: Option<&Path>) -> Self` that checks `--config` if supplied, falls back to `~/.config/shell-panel.toml`, and falls back to default if the file does not exist.
   - Helper `Config::default_toml() -> String` to generate a well-commented sample configuration.
2. **Color Parser (`src/ui/color.rs`):**
   - Converts color definitions (ANSI color names like `"cyan"`, `"black"`, `"gray"`, `"red"`, `"blue"`, `"green"`, `"yellow"`, `"magenta"`, `"white"`, 256-color numbers like `"244"`, or RGB hex codes like `"#3b82f6"`) into ANSI escape codes for foreground and background.
3. **Config-Aware Theme & UI Renderer (`src/ui/theme.rs`, `src/ui/renderer.rs`):**
   - Update `Theme` struct to hold resolved ANSI sequences and icon lookup table.
   - `Renderer::render_dropdown` and `clear_dropdown` use the active `Theme`.
4. **CLI Integration (`src/main.rs`, `src/core/app.rs`):**
   - Add `--config <PATH>` option to CLI parser.
   - Pass loaded `Config` to `App::new(config, cli.shell)`.

**Tech Stack:** Rust 1.80+, `toml = "0.8"`, `serde`, `clap`, `crossterm`.

---

### Task 1: Add `toml` Dependency and Config Data Structures

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/core/config.rs`
- Test: `tests/config_test.rs`

- [ ] **Step 1: Add `toml = "0.8"` to `Cargo.toml`**

Add `toml = "0.8"` under `[dependencies]`.

- [ ] **Step 2: Write failing unit test for TOML parsing and path resolution**

Create `tests/config_test.rs`:
```rust
use shell_panel::core::config::{Config, default_config_path};

#[test]
fn test_default_config_path_location() {
    let path = default_config_path().expect("Should resolve home dir");
    assert!(path.to_string_lossy().contains(".config"));
    assert!(path.to_string_lossy().ends_with("shell-panel.toml"));
}

#[test]
fn test_parse_custom_toml() {
    let toml_content = r#"
        max_suggestions = 10
        shell = "pwsh"

        [colors]
        selected_bg = "blue"
        selected_fg = "white"
        description_fg = "yellow"

        [icons]
        directory = "📁 "
        subcommand = "⚡ "
    "#;

    let config: Config = toml::from_str(toml_content).unwrap();
    assert_eq!(config.max_suggestions, 10);
    assert_eq!(config.shell, Some("pwsh".to_string()));
    assert_eq!(config.colors.selected_bg, "blue");
    assert_eq!(config.colors.selected_fg, "white");
    assert_eq!(config.icons.subcommand, "⚡ ");
    assert_eq!(config.icons.file, "📄 "); // default preserved
}
```

- [ ] **Step 3: Implement Config structs, defaults, and loader**

In `src/core/config.rs`:
- Define `ColorConfig` with default colors:
  - `selected_bg: "cyan"`
  - `selected_fg: "black"`
  - `description_fg: "gray"`
  - `selected_prefix: "> "`
- Define `IconConfig` with defaults matching `SuggestionKind::icon()`.
- Implement `default_config_path() -> Option<PathBuf>` using `dirs::home_dir()` or `std::env::var_os("USERPROFILE")`.
- Implement `Config::load_or_default(custom_path: Option<&Path>) -> Self`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test config_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add Cargo.toml src/core/config.rs tests/config_test.rs
git commit -m "feat(config): implement toml configuration models and path loader"
```

---

### Task 2: Color Parser and Config-Driven ANSI Styling

**Files:**
- Create: `src/ui/color.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/ui/theme.rs`
- Test: `tests/color_test.rs`

- [ ] **Step 1: Write unit test for color parsing**

Create `tests/color_test.rs`:
```rust
use shell_panel::ui::color::{parse_color_fg, parse_color_bg};

#[test]
fn test_parse_standard_colors() {
    assert_eq!(parse_color_fg("cyan"), Some("\x1b[36m".to_string()));
    assert_eq!(parse_color_bg("blue"), Some("\x1b[44m".to_string()));
    assert_eq!(parse_color_fg("gray"), Some("\x1b[90m".to_string()));
}

#[test]
fn test_parse_hex_colors() {
    assert_eq!(parse_color_fg("#ff0000"), Some("\x1b[38;2;255;0;0m".to_string()));
    assert_eq!(parse_color_bg("#00ff00"), Some("\x1b[48;2;0;255;0m".to_string()));
}
```

- [ ] **Step 2: Implement color parser in `src/ui/color.rs`**

Support:
- Standard names: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `gray` / `bright_black`.
- Hex RGB: `#RRGGBB` or `#RGB`.
- 256-color: numeric strings `0`..`255`.
- Invert / reverse: `"reverse"` or `"invert"` (`\x1b[7m`).

- [ ] **Step 3: Update `Theme` in `src/ui/theme.rs` to take `Config`**

- `Theme` struct holds:
  - `selected_start: String` (e.g. `\x1b[46m\x1b[30m` or `\x1b[7m`)
  - `selected_end: String` (`\x1b[0m`)
  - `desc_start: String` (e.g. `\x1b[90m`)
  - `desc_end: String` (`\x1b[0m`)
  - `selected_prefix: String`
  - `unselected_prefix: String`
  - `icons: IconConfig`
- Implement `Theme::from_config(config: &Config) -> Self`.
- Helper `theme.icon_for(kind: SuggestionKind) -> &str`.
- Update `format_suggestion_line_with_theme(...)` to style text according to the loaded theme.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test color_test` and `cargo test --test theme_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add src/ui/color.rs src/ui/mod.rs src/ui/theme.rs tests/color_test.rs tests/theme_test.rs
git commit -m "feat(ui): implement flexible ANSI color parser and config-driven theme"
```

---

### Task 3: Wire Config and Theme into UI Renderer & App Reactor Loop

**Files:**
- Modify: `src/ui/renderer.rs`
- Modify: `src/core/app.rs`
- Modify: `src/main.rs`
- Test: `tests/renderer_test.rs`
- Test: `tests/e2e_pty_test.rs`

- [x] **Step 1: Update `Renderer` in `src/ui/renderer.rs`**

- `Renderer::render_dropdown` accepts `&Theme`:
  `pub fn render_dropdown<W: Write>(state: &SuggestionState, term: &HeadlessTerminal, theme: &Theme, cursor_x: u16, cursor_y: u16, out: &mut W) -> io::Result<Option<DropdownLayout>>`
- Formats suggestion lines using `theme.format_line(...)`.

- [x] **Step 2: Update `App` in `src/core/app.rs`**

- Store `theme: Theme` initialized from `Theme::from_config(&self.config)`.
- Use `self.theme` when rendering dropdown.

- [x] **Step 3: Update `Cli` in `src/main.rs`**

- Add `--config <PATH>` argument.
- Load configuration via `Config::load_or_default(cli.config.as_deref())`.
- Pass to `App::new(config, cli.shell)`.

- [x] **Step 4: Run full test suite and verify**

Run: `cargo test`
Expected: All tests PASS.

- [x] **Step 5: Build release binary and commit**

Run: `cargo build --release`
```bash
git add src/ui/renderer.rs src/core/app.rs src/main.rs tests/renderer_test.rs
git commit -m "feat(core): connect toml configuration and custom theme to app and renderer"
```
