# Configuration: examples

Every example is a complete file. Keys left out keep their defaults, so you can also copy a single
section into your own file.

## Minimal

Only what differs from the defaults:

```toml
max_suggestions = 8
```

## Windows PowerShell 5.1 with a taller dropdown

```toml
max_suggestions = 12
shell = "powershell"
```

## Follow the terminal's color scheme

Reverse video for the selection, nothing else colored. Readable in light and dark schemes alike:

```toml
[colors]
selected_bg = "reverse"
unselected_fg = ""
description_fg = "gray"
```

## Truecolor, dark scheme

```toml
max_suggestions = 8

[colors]
selected_bg = "#3b82f6"
selected_fg = "#ffffff"
unselected_fg = "#c0caf5"
description_fg = "#565f89"
selected_prefix = "❯ "
unselected_prefix = "  "
```

## 256-color palette

For terminals without truecolor:

```toml
[colors]
selected_bg = "24"
selected_fg = "255"
unselected_fg = "250"
description_fg = "244"
```

## Plain ASCII

No emoji, no symbols; safe with any font and over any remote session:

```toml
[colors]
selected_bg = "reverse"
selected_prefix = "> "
unselected_prefix = "  "

[icons]
directory = "d "
file = "f "
command = "c "
subcommand = "s "
option = "- "
powershell_cmdlet = "p "
alias = "a "
other = "  "
```

## Text only

No icons, no prefixes; the selection is shown by color alone:

```toml
[colors]
selected_bg = "cyan"
selected_fg = "black"
selected_prefix = ""
unselected_prefix = ""

[icons]
directory = ""
file = ""
command = ""
subcommand = ""
option = ""
powershell_cmdlet = ""
alias = ""
other = ""
```

## Trying a file without replacing yours

```powershell
shell-panel --config .\experiment.toml
```

`--config` reads that file *instead of* the default one. Leave the session with `exit`.

Next: [Troubleshooting](Configuration-Troubleshooting) · [Reference](Configuration-Reference)
