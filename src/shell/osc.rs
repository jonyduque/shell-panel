#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscEvent {
    PromptStarted,
    PromptEnded,
    Cwd(String),
}

/// Parses an OSC sequence payload.
/// Supports OSC 6973 sequences used by PowerShell shell integration:
/// - `6973;PS` -> `OscEvent::PromptStarted`
/// - `6973;PE` -> `OscEvent::PromptEnded`
/// - `6973;CWD;<path>` -> `OscEvent::Cwd(<unescaped path>)`
pub fn parse_osc_sequence(payload: &str) -> Option<OscEvent> {
    if !payload.starts_with("6973;") {
        return None;
    }
    let body = &payload[5..];
    if body == "PS" {
        return Some(OscEvent::PromptStarted);
    }
    if body == "PE" {
        return Some(OscEvent::PromptEnded);
    }
    if let Some(cwd) = body.strip_prefix("CWD;") {
        return Some(OscEvent::Cwd(unescape_cwd(cwd)));
    }
    None
}

/// Unescapes escaped characters in the CWD payload:
/// - `\\` -> `\`
/// - `\xHH` (2 hex digits) -> corresponding char
pub fn unescape_cwd(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if chars.peek() == Some(&'\\') {
                chars.next();
                out.push('\\');
            } else if chars.peek() == Some(&'x') {
                chars.next(); // consume 'x'
                let hex: String = chars.by_ref().take(2).collect();
                if hex.len() == 2 {
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        out.push(byte as char);
                    } else {
                        out.push_str("\\x");
                        out.push_str(&hex);
                    }
                } else {
                    out.push_str("\\x");
                    out.push_str(&hex);
                }
            } else {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }
    out
}
