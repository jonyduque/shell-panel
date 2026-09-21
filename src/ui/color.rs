/// Flexible ANSI color parser for terminal UI configuration.
///
/// Supports:
/// - Standard ANSI color names (e.g. "cyan", "bright_red", "purple", "gray")
/// - 256-color palette indices (0..255)
/// - Truecolor hex RGB strings ("#RRGGBB" or "#RGB")
/// - Invert / reverse mode ("invert" or "reverse")
/// - Default / empty values ("none", "default", "")

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorSpec {
    Standard { fg: u8, bg: u8 },
    Palette256(u8),
    Rgb(u8, u8, u8),
    Invert,
}

fn parse_color_spec(s: &str) -> Option<ColorSpec> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();

    // Check special empty/none/default keywords
    if lower == "none" || lower == "default" {
        return None;
    }

    // Check invert / reverse mode
    if lower == "invert" || lower == "reverse" {
        return Some(ColorSpec::Invert);
    }

    // Check hex RGB format (#RRGGBB or #RGB)
    if let Some(hex_part) = lower.strip_prefix('#') {
        if hex_part.len() == 6 {
            let r = u8::from_str_radix(&hex_part[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex_part[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex_part[4..6], 16).ok()?;
            return Some(ColorSpec::Rgb(r, g, b));
        } else if hex_part.len() == 3 {
            let r = u8::from_str_radix(&hex_part[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex_part[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex_part[2..3], 16).ok()?;
            return Some(ColorSpec::Rgb(r * 17, g * 17, b * 17));
        } else {
            return None;
        }
    }

    // Check 256-color palette (numeric 0..255)
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(code) = trimmed.parse::<u8>() {
            return Some(ColorSpec::Palette256(code));
        } else {
            return None;
        }
    }

    // Standard ANSI color names
    let normalized = lower.replace(['-', ' '], "_");
    match normalized.as_str() {
        "black" => Some(ColorSpec::Standard { fg: 30, bg: 40 }),
        "red" => Some(ColorSpec::Standard { fg: 31, bg: 41 }),
        "green" => Some(ColorSpec::Standard { fg: 32, bg: 42 }),
        "yellow" => Some(ColorSpec::Standard { fg: 33, bg: 43 }),
        "blue" => Some(ColorSpec::Standard { fg: 34, bg: 44 }),
        "magenta" | "purple" => Some(ColorSpec::Standard { fg: 35, bg: 45 }),
        "cyan" => Some(ColorSpec::Standard { fg: 36, bg: 46 }),
        "white" => Some(ColorSpec::Standard { fg: 37, bg: 47 }),
        "gray" | "grey" | "bright_black" => Some(ColorSpec::Standard { fg: 90, bg: 100 }),
        "bright_red" => Some(ColorSpec::Standard { fg: 91, bg: 101 }),
        "bright_green" => Some(ColorSpec::Standard { fg: 92, bg: 102 }),
        "bright_yellow" => Some(ColorSpec::Standard { fg: 93, bg: 103 }),
        "bright_blue" => Some(ColorSpec::Standard { fg: 94, bg: 104 }),
        "bright_magenta" | "bright_purple" => Some(ColorSpec::Standard { fg: 95, bg: 105 }),
        "bright_cyan" => Some(ColorSpec::Standard { fg: 96, bg: 106 }),
        "bright_white" => Some(ColorSpec::Standard { fg: 97, bg: 107 }),
        _ => None,
    }
}

/// Parses a color string into an ANSI foreground escape sequence.
pub fn parse_color_fg(s: &str) -> Option<String> {
    match parse_color_spec(s)? {
        ColorSpec::Standard { fg, .. } => Some(format!("\x1b[{}m", fg)),
        ColorSpec::Palette256(n) => Some(format!("\x1b[38;5;{}m", n)),
        ColorSpec::Rgb(r, g, b) => Some(format!("\x1b[38;2;{};{};{}m", r, g, b)),
        ColorSpec::Invert => Some("\x1b[7m".to_string()),
    }
}

/// Parses a color string into an ANSI background escape sequence.
pub fn parse_color_bg(s: &str) -> Option<String> {
    match parse_color_spec(s)? {
        ColorSpec::Standard { bg, .. } => Some(format!("\x1b[{}m", bg)),
        ColorSpec::Palette256(n) => Some(format!("\x1b[48;5;{}m", n)),
        ColorSpec::Rgb(r, g, b) => Some(format!("\x1b[48;2;{};{};{}m", r, g, b)),
        ColorSpec::Invert => Some("\x1b[7m".to_string()),
    }
}
