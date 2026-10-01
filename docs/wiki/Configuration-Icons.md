# Configuration: icons

The `[icons]` section sets the marker drawn between the row prefix and the suggestion text. There
is one key per kind of suggestion.

```toml
[icons]
directory = "📁 "
file = "📄 "
command = "⚡ "
subcommand = "🔹 "
option = "🏷️  "
powershell_cmdlet = ">_ "
alias = "🔗 "
other = "  "
```

## Keys

| Key | Default | Used for |
|-----|---------|----------|
| `directory` | `"📁 "` | Folders and other containers from PowerShell's path completion (`ProviderContainer`); zoxide directories |
| `file` | `"📄 "` | Files and other items from PowerShell's path completion (`ProviderItem`) |
| `command` | `"⚡ "` | Commands reported by PowerShell whose name has no `-`: executables, scripts, functions (`git`, `node`) |
| `subcommand` | `"🔹 "` | Subcommands and argument values from command specs and from carapace (`git status`) |
| `option` | `"🏷️  "` | Flags from specs and carapace (`--force`), and PowerShell parameter names (`-Recurse`) |
| `powershell_cmdlet` | `">_ "` | Commands reported by PowerShell whose name contains `-` (`Get-ChildItem`) |
| `alias` | `"🔗 "` | PowerShell's built-in aliases (`ls`, `gci`, `cd`) |
| `other` | `"  "` | Everything else PowerShell reports: variables, types, namespaces, members, property names, history entries |

## Writing icon values

- A value is a plain string and **includes its own spacing**. The defaults end with a space so the
  text does not touch the icon; keep a trailing space in your own values.
- To remove an icon, use an empty string. To keep the text of all rows aligned, either remove
  every icon or give every icon the same display width.
- Terminals do not agree on the width of some emoji. `🏷️` carries a variation selector and is
  drawn narrower than it is measured in several terminals, which is why its default has two
  trailing spaces. If a row looks misaligned in your terminal, adjust the spaces of that one icon.
- Any text works, not only emoji: ASCII tags (`"[d] "`), or glyphs from a
  [Nerd Font](https://www.nerdfonts.com) if your terminal uses one.
- Icons take the row's color: `unselected_fg` on ordinary rows, the selected colors on the
  selected row. Emoji ignore the foreground color in most terminals; text glyphs follow it.

## Examples

ASCII only, for fonts without emoji:

```toml
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

No icons at all:

```toml
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

Next: [Examples](Configuration-Examples) · [Troubleshooting](Configuration-Troubleshooting)
