# shell-panel wiki

shell-panel wraps PowerShell in a pseudo console and draws a completion dropdown when you press
Tab. This wiki documents its configuration file.

## Configuration

| Page | What it covers |
|------|----------------|
| [[Configuration]] | Where the file lives, how it is loaded, the top-level keys, precedence over command-line options |
| [Colors](Configuration-Colors) | The `[colors]` section: every key, every accepted color format, fallbacks |
| [Icons](Configuration-Icons) | The `[icons]` section: every key and which suggestions use it |
| [Examples](Configuration-Examples) | Complete, copy-ready configuration files |
| [Troubleshooting](Configuration-Troubleshooting) | Warnings, errors, exit codes and the usual mistakes |
| [Reference](Configuration-Reference) | One table with every key, type and default |

## Quick start

```powershell
# PowerShell 7 or Windows PowerShell 5.1
New-Item -ItemType Directory -Force "$HOME\.config" | Out-Null
# Decode shell-panel's output as UTF-8 (the default is the OEM code page, which turns the emoji into "?"),
# and write the file as UTF-8 without a BOM. Works in PowerShell 7 and Windows PowerShell 5.1.
[Console]::OutputEncoding = [Text.Encoding]::UTF8
[IO.File]::WriteAllText("$HOME\.config\shell-panel.toml", (shell-panel --print-default-config | Out-String), [Text.UTF8Encoding]::new($false))
notepad "$HOME\.config\shell-panel.toml"
```

Restart shell-panel after editing: the file is read once, at start-up.
