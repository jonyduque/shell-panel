use crate::shell::report::ShellReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscEvent {
    /// PSReadLine started reading a line; `cwd` is set when the location is a filesystem path.
    ReadLineStarted { cwd: Option<String> },
    /// PSReadLine returned the line (it was accepted or cancelled).
    ReadLineEnded,
    /// Answer to [`REPORT_REQUEST_KEY`].
    Report(ShellReport),
}

/// Key sequence for Ctrl+Alt+Shift+F12. The integration script binds this chord to a PSReadLine
/// handler that answers with `OSC 6973;CMP;<json>`.
pub const REPORT_REQUEST_KEY: &[u8] = b"\x1b[24;8~";

/// Parses an OSC sequence payload according to the 6973 protocol.
///
/// Supported sequences:
/// - `6973;RS;<escaped_path>` -> `OscEvent::ReadLineStarted`
/// - `6973;RE` -> `OscEvent::ReadLineEnded`
/// - `6973;CMP;<escaped_json>` -> `OscEvent::Report`
pub fn parse_osc_sequence(payload: &str) -> Option<OscEvent> {
    let body = payload.strip_prefix("6973;")?;
    if body == "RE" {
        return Some(OscEvent::ReadLineEnded);
    }
    if let Some(cwd) = body.strip_prefix("RS;") {
        let cwd = unescape_value(cwd);
        return Some(OscEvent::ReadLineStarted {
            cwd: (!cwd.is_empty()).then_some(cwd),
        });
    }
    if let Some(json) = body.strip_prefix("CMP;") {
        return serde_json::from_str(&unescape_value(json))
            .ok()
            .map(OscEvent::Report);
    }
    None
}

/// Unescapes a value escaped by the shell integration script.
///
/// Handles:
/// - `\\` -> `\`
/// - `\xHH` -> raw UTF-8 byte
/// Multibyte UTF-8 sequences are decoded safely.
pub fn unescape_value(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut buf = [0u8; 4];

    while let Some(c) = chars.next() {
        if c == '\\' {
            if chars.peek() == Some(&'\\') {
                chars.next();
                bytes.push(b'\\');
            } else if chars.peek() == Some(&'x') {
                chars.next(); // consume 'x'
                let h1 = chars.next();
                let h2 = chars.next();
                match (h1, h2) {
                    (Some(c1), Some(c2)) if c1.is_ascii_hexdigit() && c2.is_ascii_hexdigit() => {
                        let hex_str = [c1 as u8, c2 as u8];
                        if let Ok(s) = std::str::from_utf8(&hex_str) {
                            if let Ok(val) = u8::from_str_radix(s, 16) {
                                bytes.push(val);
                            }
                        }
                    }
                    _ => {
                        bytes.extend_from_slice(b"\\x");
                        if let Some(c1) = h1 {
                            bytes.extend_from_slice(c1.encode_utf8(&mut buf).as_bytes());
                        }
                        if let Some(c2) = h2 {
                            bytes.extend_from_slice(c2.encode_utf8(&mut buf).as_bytes());
                        }
                    }
                }
            } else {
                bytes.push(b'\\');
            }
        } else {
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
