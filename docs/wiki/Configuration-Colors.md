# Configuration: colors

The `[colors]` section styles the rows of the completion dropdown. Despite its name it also holds
the two row prefixes.

```toml
[colors]
selected_bg = "cyan"
selected_fg = "black"
unselected_fg = ""
description_fg = "gray"
selected_prefix = "> "
unselected_prefix = "  "
```

- [Anatomy of a row](#anatomy-of-a-row)
- [Keys](#keys)
- [Color formats](#color-formats)
- [Fallbacks](#fallbacks)
- [Reverse video](#reverse-video)

## Anatomy of a row

```
> ⚡ status  Show the working tree status
│ │  │       └ description   (description_fg)
│ │  └ suggestion text       (selected_bg/selected_fg, or unselected_fg)
│ └ icon                     (see the Icons page)
└ prefix                     (selected_prefix or unselected_prefix)
```

- The **selected** row is painted as one block: prefix, icon, text and description all use
  `selected_bg` + `selected_fg`. `description_fg` does not apply to it.
- An **unselected** row uses `unselected_fg` for prefix, icon and text, and `description_fg` for
  the description. Its background is always the terminal's.
- Rows are padded to at least 30 columns and cut at the right edge of the terminal. When a row
  does not fit, the description is shortened first, then the text.

## Keys

| Key | Type | Default | Applies to |
|-----|------|---------|------------|
| `selected_bg` | color | `"cyan"` | Background of the selected row |
| `selected_fg` | color | `"black"` | Foreground of the selected row |
| `unselected_fg` | color | `""` (terminal default) | Prefix, icon and text of the other rows |
| `description_fg` | color | `"gray"` | Description of the unselected rows |
| `selected_prefix` | string | `"> "` | Text placed before the icon of the selected row |
| `unselected_prefix` | string | `"  "` | Text placed before the icon of the other rows |

The prefixes are plain strings: any text works, including an empty string or a symbol such as
`"❯ "`. Give both prefixes the same display width, otherwise the text jumps sideways as the
selection moves.

## Color formats

Every color key accepts the same formats. Names are case-insensitive, and `-` or a space may be
used instead of `_` (`"bright-red"`, `"Bright Red"`).

| Format | Examples | Notes |
|--------|----------|-------|
| Color name | `"cyan"`, `"bright_red"` | The 16 ANSI colors; the actual shade comes from your terminal's color scheme |
| 256-color index | `"244"`, `"39"` | A string of digits from `0` to `255` |
| Hex RGB | `"#3b82f6"`, `"#38f"` | `#RRGGBB`, or `#RGB` where each digit is doubled (`#38f` = `#3388ff`). Needs a truecolor terminal; Windows Terminal is one |
| Reverse video | `"reverse"`, `"invert"` | Swaps foreground and background. See [Reverse video](#reverse-video) |
| Terminal default | `""`, `"none"`, `"default"` | No color is set |

Values are always TOML **strings**: write `"244"`, not `244`. A bare number has the wrong type and
invalidates the whole file.

### Color names

| Normal | Bright |
|--------|--------|
| `black` | `bright_black` = `gray` = `grey` |
| `red` | `bright_red` |
| `green` | `bright_green` |
| `yellow` | `bright_yellow` |
| `blue` | `bright_blue` |
| `magenta` = `purple` | `bright_magenta` = `bright_purple` |
| `cyan` | `bright_cyan` |
| `white` | `bright_white` |

## Fallbacks

A color value shell-panel cannot understand — a misspelt name, an index above 255, a hex string of
the wrong length — produces **no warning**. The key behaves as if it were empty:

| Key | When empty, `"none"`, `"default"` or not understood |
|-----|------------------------------------------------------|
| `selected_bg` | Only `selected_fg` is applied |
| `selected_fg` | Only `selected_bg` is applied |
| both `selected_*` | The selected row is drawn in reverse video |
| `unselected_fg` | Terminal default foreground |
| `description_fg` | Bright black (`gray`) |

If a color does not show up, check its spelling against the tables above first.

## Reverse video

Setting **either** `selected_bg` or `selected_fg` to `"reverse"` (or `"invert"`) draws the whole
selected row in reverse video and ignores the other key. It follows the terminal's color scheme,
so it stays readable in both light and dark themes:

```toml
[colors]
selected_bg = "reverse"
```

Next: [Icons](Configuration-Icons) · [Examples](Configuration-Examples)
