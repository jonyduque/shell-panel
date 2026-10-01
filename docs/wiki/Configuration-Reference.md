# Configuration: reference

Default location: `%USERPROFILE%\.config\shell-panel.toml` · format: TOML, UTF-8 · read once at
start-up · every key is optional.

## All keys

| Key | Type | Default | Details |
|-----|------|---------|---------|
| `max_suggestions` | integer ≥ 0 | `5` | Rows per dropdown page; `0` means `5`. [More](Configuration#max_suggestions) |
| `shell` | string | auto-detect | `"pwsh"` or `"powershell"` (also with `.exe`). [More](Configuration#shell) |
| `colors.selected_bg` | color | `"cyan"` | [Colors](Configuration-Colors) |
| `colors.selected_fg` | color | `"black"` | |
| `colors.unselected_fg` | color | `""` | Empty = terminal default |
| `colors.description_fg` | color | `"gray"` | Unselected rows only |
| `colors.selected_prefix` | string | `"> "` | |
| `colors.unselected_prefix` | string | `"  "` | |
| `icons.directory` | string | `"📁 "` | [Icons](Configuration-Icons) |
| `icons.file` | string | `"📄 "` | |
| `icons.command` | string | `"⚡ "` | |
| `icons.subcommand` | string | `"🔹 "` | |
| `icons.option` | string | `"🏷️  "` | |
| `icons.powershell_cmdlet` | string | `">_ "` | |
| `icons.alias` | string | `"🔗 "` | |
| `icons.other` | string | `"  "` | |

## Color values

| Format | Example |
|--------|---------|
| ANSI name | `"cyan"`, `"bright_red"`, `"gray"`, `"purple"` |
| 256-color index | `"0"` … `"255"` |
| Hex RGB | `"#3b82f6"`, `"#38f"` |
| Reverse video | `"reverse"`, `"invert"` |
| Terminal default | `""`, `"none"`, `"default"` |

Always a quoted string. Names are case-insensitive; `-` and space are accepted for `_`.

## The default file

This is what `shell-panel --print-default-config` prints:

```toml
# Shell-Panel Configuration

# Maximum number of suggestions to display in the dropdown (default: 5)
max_suggestions = 5

# Shell to launch: "pwsh" or "powershell" (default: pwsh.exe when on PATH, else powershell.exe)
# shell = "pwsh"

[colors]
# Selected item background color (e.g. "cyan", "blue", "reverse", "#3b82f6", "244")
selected_bg = "cyan"

# Selected item foreground color
selected_fg = "black"

# Unselected item foreground color (empty for terminal default)
unselected_fg = ""

# Description text foreground color
description_fg = "gray"

# Indicator prefix for the selected row
selected_prefix = "> "

# Indicator prefix for unselected rows
unselected_prefix = "  "

[icons]
# Icons displayed for each suggestion category
directory = "📁 "
file = "📄 "
command = "⚡ "
subcommand = "🔹 "
option = "🏷️  "
powershell_cmdlet = ">_ "
alias = "🔗 "
other = "  "
```

## Related command-line options

| Option | Effect |
|--------|--------|
| `--config <PATH>` | Read this file instead of the default location |
| `--print-default-config` | Print the file above and exit |
| `-s`, `--shell <SHELL>` | Override the `shell` key |
