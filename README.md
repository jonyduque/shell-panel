# shell-panel

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

IDE-style Tab completion dropdown for PowerShell on Windows, written in Rust.
Inspired by [`@microsoft/inshellisense`](https://github.com/microsoft/inshellisense), without a Node.js runtime.

## How it works

shell-panel starts PowerShell inside a ConPTY pseudo-terminal and sits between it and your terminal.

- A small integration script is embedded in the binary and passed to PowerShell with `-EncodedCommand`: nothing is written to disk and your execution policy is not touched. It wraps `PSConsoleHostReadLine` to tell shell-panel when PSReadLine is reading a line (and in which directory), and binds **Ctrl+Alt+Shift+F12** to a handler that reports the current line, the cursor and PowerShell's own completions for it. Every message the script sends carries a secret generated for the session, so text printed by a program cannot pose as a report.
- When you press **Tab**, shell-panel sends that chord, receives the report from *your* session — so variables, functions, registered argument completers and the current location are all known — merges it with its other sources and draws a dropdown over the terminal. A headless VT100 emulator mirrors the screen so the rows under the dropdown are restored exactly, colors included.
- While a program runs, shell-panel passes the terminal's input through unchanged (mouse, focus, paste, query answers, key sequences). Only while PowerShell reads a line does it interpret keys.
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

## Install

```powershell
irm https://github.com/jonyduque/shell-panel/releases/latest/download/install.ps1 | iex
```

The installer picks the x64 or ARM64 build, checks it against the release's `SHA256SUMS.txt`, installs `shell-panel.exe` into `%LOCALAPPDATA%\Programs\shell-panel`, adds that folder to your user `PATH` and adds a **PowerShell (shell-panel)** profile to Windows Terminal. Run it again to update (also from inside a shell-panel session). A specific version, or another folder (it must be new or empty):

```powershell
& ([scriptblock]::Create((irm https://github.com/jonyduque/shell-panel/releases/latest/download/install.ps1))) -Version 0.2.0
```

A downloaded `install.ps1` run as a file is blocked by the default execution policy; use the line above instead.

To uninstall (your `~\.config\shell-panel.toml` and custom specs are kept unless you add `-Purge`):

```powershell
irm https://github.com/jonyduque/shell-panel/releases/latest/download/uninstall.ps1 | iex
```

## Build and run

```powershell
cargo build --release
.\target\release\shell-panel.exe
```

`vendor/crossterm` is crossterm 0.28.1 with one fix for characters outside the BMP (see `vendor/crossterm/SHELL-PANEL-PATCH.md`).

The shell starts in the directory you start shell-panel from. Starting shell-panel inside a shell-panel session is refused (`SHELL_PANEL_SESSION=1` is set inside a session; `shell-panel --check` tests for it).

## Keys

| Key | Dropdown closed | Dropdown open |
|-----|-----------------|---------------|
| Tab | Complete: one match is inserted directly, several open the dropdown, none falls back to PowerShell's Tab | Insert the highlighted suggestion |
| Enter | Passed to PowerShell (runs the line) | Insert the highlighted suggestion; the line is not run |
| Shift+Enter, Ctrl+Enter, Alt+Enter | Passed to PowerShell | Passed to PowerShell |
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
# Decode shell-panel's output as UTF-8 (the default is the OEM code page, which turns the emoji into "?"),
# and write the file as UTF-8 without a BOM. Works in PowerShell 7 and Windows PowerShell 5.1.
$enc = [Console]::OutputEncoding; [Console]::OutputEncoding = [Text.Encoding]::UTF8
try { [IO.File]::WriteAllText("$HOME\.config\shell-panel.toml", (shell-panel --print-default-config | Out-String), [Text.UTF8Encoding]::new($false)) }
finally { [Console]::OutputEncoding = $enc }
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

`scripts/test-installer.ps1` tests `install.ps1` and `uninstall.ps1` end to end against `target\release` (run `cargo build --release` first), in temporary folders; add `-Shell powershell` to run them under Windows PowerShell 5.1. It restores your `PATH` afterwards.

### Releasing

1. Set `version` in `Cargo.toml` (and run `cargo build` so `Cargo.lock` follows), commit.
2. `git tag v<version>` and `git push origin v<version>`.

The Release workflow checks that the tag matches `Cargo.toml`, runs the full gate, builds x64 and ARM64, tests the installer on the x64 build and publishes the release with both zips, `SHA256SUMS.txt`, `install.ps1` and `uninstall.ps1`. A tag with a `-` (e.g. `v0.2.0-rc.1`) is published as a pre-release.

## Known limitations

- Windows and PowerShell only.
- Under execution policy `Restricted`, PSReadLine cannot load; shell-panel then gets no ReadLine markers and no completion report, and behaves as a plain pass-through terminal. shell-panel deliberately does not pass `-ExecutionPolicy Bypass`.
- PowerShell's completions are computed on the shell's thread, like native Tab: a slow completer delays the dropdown (after 3 seconds Tab falls back to PowerShell).
- The report travels through the terminal stream as an OSC sequence. Shell messages longer than 1 MiB, or containing raw control bytes, are treated as ordinary output. Messages without the session's secret are ignored. This is verified on Windows 11; very old Windows 10 console hosts may truncate long sequences.
- A host input sequence that arrives in the instant a program starts or ends may be read in the wrong mode.
- A Tab character inside pasted text triggers completion instead of being inserted.
- The child shell inherits `SHELL_PANEL_SESSION=1`, so shell-panel refuses to start in a new window opened from inside a session (for example with `Start-Process`) until that variable is removed: `$env:SHELL_PANEL_SESSION = $null`.

## License

MIT — see [LICENSE](LICENSE).
