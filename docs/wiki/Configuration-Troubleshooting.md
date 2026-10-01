# Configuration: troubleshooting

Configuration messages are printed on standard error, prefixed with `shell-panel: `, before the
shell starts. Scroll up to just above the first prompt to find them.

## Messages

### ``unknown key `<key>` in <path> (ignored)``

The file is valid and was applied; this one key is not something shell-panel knows.

| Usual cause | Fix |
|-------------|-----|
| A typo: `colors.selcted_bg`, `icons.folder` | Compare with the [Reference](Configuration-Reference) |
| A top-level key written below a `[section]` header, reported as `colors.max_suggestions` or `icons.shell` | Move it above the first `[section]` header |
| A key from an older version, such as `debounce_ms` | Delete the line |

### `invalid config <path>: <error>; using defaults`

The file could not be parsed, so **none of it** was applied. The error text names the line and
column.

| Usual cause | Fix |
|-------------|-----|
| A color written as a number: `selected_bg = 244` | Quote it: `selected_bg = "244"` |
| `max_suggestions` written as a string or as a negative number | Use a bare integer, 0 or greater: `max_suggestions = 8` |
| A missing closing quote or bracket | Fix the line the message points at |
| A backslash inside a double-quoted string, such as a path | Use single quotes (`'C:\x'`) or double the backslash |
| The same key or the same `[section]` appears twice | Remove the duplicate |

### `could not read config <path>: <error>`

The file could not be opened or is not text shell-panel can read. The defaults are used.

| Usual cause | Fix |
|-------------|-----|
| The path given to `--config` does not exist | Correct the path. A missing file at the *default* location is normal and prints nothing |
| `stream did not contain valid UTF-8`: the file was saved as UTF-16, typically created with `>` in Windows PowerShell 5.1 | Save it as UTF-8 |
| Access denied, or the path is a folder | Fix the permissions or the path |

### `unsupported shell "<name>"; use "pwsh" or "powershell"`

The `shell` key, or `--shell`, holds a value other than `pwsh`, `pwsh.exe`, `powershell` or
`powershell.exe`. shell-panel exits with code **2** without starting a shell.

## Symptoms without a message

| Symptom | Cause | Fix |
|---------|-------|-----|
| A color is ignored | Color values are not validated; an unrecognized value silently falls back | Check the spelling against [Colors](Configuration-Colors#color-formats). Indices go from `0` to `255`; hex needs 3 or 6 digits after `#` |
| Truecolor (`#rrggbb`) looks wrong or is missing | The terminal does not support 24-bit color (the legacy console window) | Use Windows Terminal, or a 256-color index |
| `description_fg` has no effect on the selected row | By design: the selected row is one block in the selected colors | — |
| Rows are misaligned by one column | The terminal draws an emoji narrower or wider than it is measured | Add or remove a trailing space in that icon; see [Icons](Configuration-Icons#writing-icon-values) |
| Text shifts sideways when the selection moves | The two prefixes have different widths | Give `selected_prefix` and `unselected_prefix` the same width |
| The dropdown shows fewer rows than `max_suggestions` | There are fewer matches, or not enough free rows above or below the cursor | Enlarge the terminal, or clear the screen so the prompt sits near the top |
| Edits have no effect | The file is read once, at start-up | `exit` and start shell-panel again |
| Edits have no effect after a restart | A different file is being read | Check for `--config` in the shortcut or terminal profile that starts shell-panel, and check `$env:USERPROFILE` |
| `max_suggestions = 0` still shows five rows | `0` means "use the default" | Use a positive number |

## Checking what shell-panel sees

```powershell
# The file shell-panel reads by default
Get-Content "$env:USERPROFILE\.config\shell-panel.toml"

# The defaults, for comparison
shell-panel --print-default-config

# Start-up and completion details, written to a log file
shell-panel --verbose
Get-Content "$env:TEMP\shell-panel\shell-panel.log" -Tail 40
```

## Exit codes related to configuration

| Code | Meaning |
|------|---------|
| `2` | Unsupported shell in the `shell` key or in `--shell` |
| other | The exit code of the shell itself; configuration problems never change it |

Next: [Reference](Configuration-Reference)
