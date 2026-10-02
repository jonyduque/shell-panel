# Configuration

shell-panel is configured with one optional [TOML](https://toml.io) file. Without it, every
setting takes its default, so the file only needs the keys you want to change.

- [File location](#file-location)
- [Creating the file](#creating-the-file)
- [How the file is loaded](#how-the-file-is-loaded)
- [File structure](#file-structure)
- [Top-level keys](#top-level-keys)
- [Command-line options and precedence](#command-line-options-and-precedence)
- [What is not configurable here](#what-is-not-configurable-here)

## File location

| Order | Location | Used when |
|-------|----------|-----------|
| 1 | The path given to `--config <PATH>` | The option is present |
| 2 | `%USERPROFILE%\.config\shell-panel.toml` | No `--config`; `USERPROFILE` is set |
| 3 | `%HOME%\.config\shell-panel.toml` | No `--config`; `USERPROFILE` is not set but `HOME` is |

Only one file is read: `--config` replaces the default location, the two are never merged.
If neither `USERPROFILE` nor `HOME` is set and no `--config` is given, the defaults are used.

On a typical machine the default path is `C:\Users\<you>\.config\shell-panel.toml`.

## Creating the file

`--print-default-config` prints a commented file with every key at its default value:

```powershell
# PowerShell 7 or Windows PowerShell 5.1
New-Item -ItemType Directory -Force "$HOME\.config" | Out-Null
# Decode shell-panel's output as UTF-8 (the default is the OEM code page, which turns the emoji into "?"),
# and write the file as UTF-8 without a BOM. Works in PowerShell 7 and Windows PowerShell 5.1.
$enc = [Console]::OutputEncoding; [Console]::OutputEncoding = [Text.Encoding]::UTF8
try { [IO.File]::WriteAllText("$HOME\.config\shell-panel.toml", (shell-panel --print-default-config | Out-String), [Text.UTF8Encoding]::new($false)) }
finally { [Console]::OutputEncoding = $enc }
```

The file must be **UTF-8**. In Windows PowerShell 5.1 the `>` operator writes UTF-16, which
shell-panel cannot parse, and the default output decoding replaces the emoji with `?`; the
snippet above sets the console decoding to UTF-8 and writes the file without a BOM, so it works
in both shells.

## How the file is loaded

The file is read **once, when shell-panel starts**. Changes take effect the next time you start
it; there is no reload command.

| Situation | Result | Message |
|-----------|--------|---------|
| Default location, file missing | Defaults | none |
| `--config` path missing; or either file unreadable or not UTF-8 text | Defaults | `could not read config <path>: <error>` |
| File is not valid TOML, or a value has the wrong type | **All** defaults — the whole file is discarded | `invalid config <path>: <error>; using defaults` |
| File is valid but contains a key shell-panel does not know | The known keys apply, the unknown key is ignored | ``unknown key `<key>` in <path> (ignored)`` |
| A key is absent | That key takes its default | none |

Messages are written to standard error, prefixed with `shell-panel: `, **before** the shell
starts, so they sit just above the first prompt. None of them stops shell-panel from starting.

Details worth knowing:

- A single wrong type (for example `max_suggestions = "5"` with quotes) invalidates the whole file,
  not just that key. The message names the line and column.
- Unknown keys are reported with their dotted path: `colors.selcted_bg`, `icons.folder`,
  `debounce_ms`. This is how a typo shows up.
- Color *values* that shell-panel cannot understand are **not** reported; the key silently falls
  back. See [Colors](Configuration-Colors#fallbacks).

## File structure

```toml
# top-level keys
max_suggestions = 5
shell = "pwsh"

[colors]
# see the Colors page

[icons]
# see the Icons page
```

Top-level keys must come **before** the first `[section]` header. In TOML everything after
`[colors]` belongs to `colors` until the next header, so a `max_suggestions` line placed below
`[colors]` is read as `colors.max_suggestions` and reported as an unknown key.

## Top-level keys

### `max_suggestions`

|  |  |
|--|--|
| Type | integer, 0 or greater |
| Default | `5` |

The number of rows of the dropdown, which is also the page size: with more matches than rows,
Down/Up move through pages of this many items.

- `0` is treated as `5`.
- A negative number or a non-integer makes the file invalid (see above).
- The dropdown never covers the line you are typing. It opens below the cursor when the rows fit
  there, otherwise above; when neither side has room for `max_suggestions` rows it uses the larger
  side and shows fewer rows per page. In a very short terminal the effective page can therefore be
  smaller than the configured value.

```toml
max_suggestions = 10
```

### `shell`

|  |  |
|--|--|
| Type | string |
| Default | not set: `pwsh.exe` when it is on `PATH`, otherwise `powershell.exe` |
| Accepted | `"pwsh"`, `"pwsh.exe"`, `"powershell"`, `"powershell.exe"` (case-insensitive) |

Which PowerShell to launch. `pwsh` is PowerShell 7+, `powershell` is Windows PowerShell 5.1.

Any other value stops shell-panel at start-up with exit code 2:

```
shell-panel: unsupported shell "bash"; use "pwsh" or "powershell"
```

shell-panel's integration is PowerShell-only, so other shells are rejected rather than started
without completions. The value is a name, not a path.

```toml
shell = "powershell"
```

## Command-line options and precedence

| Option | Relation to the file |
|--------|----------------------|
| `--config <PATH>` | Reads this file instead of the default location |
| `-s`, `--shell <SHELL>` | Overrides the `shell` key. Validated the same way |
| `--print-default-config` | Prints the sample file and exits. Does not read any file |
| `--no-profile` | Starts the shell with `-NoProfile`. No equivalent key |
| `-v`, `--verbose` | Writes a debug log to `%TEMP%\shell-panel\shell-panel.log`. No equivalent key |
| `-c`, `--check` | Reports whether the terminal is already inside a shell-panel session. Does not read any file |

Precedence for the shell: `--shell` → `shell` key → automatic detection.

## What is not configurable here

- **Completion specs.** Custom command specs are JSON files in
  `%USERPROFILE%\.config\shell-panel\specs\`, not part of this file.
- **Keys.** Tab, Enter, Up/Down, Shift+Tab and Esc are fixed.
- **Completion sources and their order.** PowerShell's own completions, specs, carapace and zoxide
  are always merged by fixed rules.
- **The log file location and the 3-second report timeout.**

Next: [Colors](Configuration-Colors) · [Icons](Configuration-Icons)
