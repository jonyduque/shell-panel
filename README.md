# shell-panel 🚀

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](#)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20PowerShell-blue)](https://github.com/)
[![Binary Size](https://img.shields.io/badge/binary%20size-1.5%20MB-success)](https://github.com/)

> **Fast, native, IDE-style command line autocompletion panel for Windows PowerShell built in Rust.**  
> A complete, zero-dependency native rewrite inspired by `@microsoft/inshellisense`. No Node.js runtime required.

---

## Overview

`shell-panel` acts as a transparent, high-performance **ConPTY** wrapper between your terminal and a child PowerShell process (`pwsh.exe` or `powershell.exe`). 

It tracks terminal state using an internal headless VT100 emulator, intercepts keystrokes to present a non-destructive floating completion dropdown, and aggregates suggestions across multiple providers (Fig JSON CLI specs, the Carapace CLI, Zoxide, and the local filesystem).

```
+-------------------------------------------------------------------------+
| [Host Terminal: Windows Terminal / conhost / VSCode Terminal]           |
+-------------------------------------------------------------------------+
       ^ (Raw Key Events)                          | (Sanitized Host Output)
       |                                           v
+-------------------------------------------------------------------------+
| shell-panel Reactor Loop                                                |
|  - RawModeGuard (RAII raw mode & panic safety)                          |
|  - StdioProxy & Key Classifier (Interception & VT byte encoding)       |
|  - Headless VT100 Emulator (Buffer tracking & PSReadLine ghost filter)  |
|  - Completion Engine (Fig specs, Carapace, Zoxide, Files)               |
|  - Virtual Patching TUI Renderer (Non-destructive line restoration)     |
+-------------------------------------------------------------------------+
       | (Piped Child Input)                       ^ (Master PTY Bytes)
       v                                           |
+-------------------------------------------------------------------------+
| ConPTY Pseudo-Terminal (`portable-pty`)                                  |
|   `-- PowerShell Child (`pwsh.exe` or `powershell.exe`)                 |
|         `-- `shellIntegration.ps1` (OSC 6973 lifecycle & CWD tracking)  |
+-------------------------------------------------------------------------+
```

---

## ✨ Features

- ⚡ **Zero Node.js Dependency:** Pure native Rust 2021 binary (~1.5 MB release build with LTO). Starts instantly with sub-millisecond overhead.
- 🪟 **First-Class Windows Support:** Built directly on top of Microsoft Windows ConPTY (`portable-pty`).
- 🔄 **Non-Destructive Virtual Patching:** The suggestion dropdown floats directly above or below your prompt line and restores the original terminal buffer cells on dismissal or scroll without screen tearing.
- 👻 **PSReadLine Ghost Text Filtering:** Differentiates between real typed command characters and PSReadLine inline prediction ghost text (`dim` / `italic` / gray text), ensuring autocomplete activates on what you actually typed.
- 🔌 **Modular Completion Providers:**
  - **Fig CLI Specs:** Embedded and extensible JSON specifications (supports `git`, `docker`, and converted Fig schemas).
  - **Carapace Bridge:** Queries `carapace <cmd> export` dynamically when available on `PATH`.
  - **Zoxide Integration:** Instant fuzzy directory suggestions for `cd`, `z`, and `zi`.
  - **Local Filesystem:** Fast asynchronous directory and file suggestions with Windows and Unix path separator awareness.
- 📦 **100% Self-Contained Binary:** Automatically embeds `shellIntegration.ps1` inside the executable. No external script files needed on the target machine.
- 🛡️ **Panic & Exit Safety:** RAII guards and panic hooks ensure the host console is always restored to normal mode and cursor visibility is restored, even on abrupt termination or `exit` with non-zero error codes.

---

## 🚀 Quick Start

### Prerequisites

- **OS:** Windows 10/11 or Windows Server 2019+
- **Rust:** Rust 1.75+ (for building from source)
- **Shell:** PowerShell 7 (`pwsh.exe`) or Windows PowerShell 5.1 (`powershell.exe`)

### Building from Source

```powershell
# Clone the repository
git clone https://github.com/your-username/shell-panel.git
cd shell-panel

# Build release binary
cargo build --release

# Run shell-panel
.\target\release\shell-panel.exe
```

---

## ⌨️ Controls & Navigation

When typing in PowerShell with `shell-panel` active:

| Key | Action |
|---|---|
| <kbd>↓</kbd> (Down Arrow) | Select next suggestion (cyclical wrapping) |
| <kbd>↑</kbd> (Up Arrow) / <kbd>Shift+Tab</kbd> | Select previous suggestion (cyclical wrapping) |
| <kbd>Tab</kbd> | Accept highlighted suggestion (auto-inserts text and trailing space) |
| <kbd>Esc</kbd> | Dismiss autocomplete menu |
| Any other key | Passed through directly to PowerShell |

---

## ⚙️ Command Line Options

```powershell
shell-panel [OPTIONS]

Options:
  -s, --shell <SHELL>  Shell to run (pwsh, powershell). Defaults to auto-detecting pwsh.exe on PATH
  -v, --verbose        Enable verbose debug logging to stderr
  -c, --check          Check if currently running inside a shell-panel session
  -h, --help           Print help
  -V, --version        Print version
```

---

## 🧪 Testing

The test suite includes 62 unit, integration, and end-to-end tests exercising real Windows ConPTY sessions, VT emulation, and completion algorithms:

```powershell
cargo test
```

---

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for more information.
