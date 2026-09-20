use std::borrow::Cow;

/// Strips Win32 input mode (`\x1b[?9001h` / `\x1b[?9001l`) and Kitty keyboard protocol upgrade queries
/// from the terminal output stream so they don't corrupt terminal parsing.
///
/// Uses `Cow` to avoid memory allocation when no matching escape sequences are present.
pub fn sanitize_output_stream(data: &[u8]) -> Cow<'_, [u8]> {
    if !data.contains(&0x1b) {
        return Cow::Borrowed(data);
    }

    let needs_stripping = data.windows(4).any(|w| {
        w == b"\x1b[?u"
            || w.starts_with(b"\x1b[?")
            || w.starts_with(b"\x1b[=")
            || w.starts_with(b"\x1b[>")
            || w.starts_with(b"\x1b[<")
    });

    if !needs_stripping {
        return Cow::Borrowed(data);
    }

    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    let mut last_copied = 0;

    while i < data.len() {
        if data[i..].starts_with(b"\x1b[?9001h") || data[i..].starts_with(b"\x1b[?9001l") {
            out.extend_from_slice(&data[last_copied..i]);
            i += 8;
            last_copied = i;
        } else if data[i..].starts_with(b"\x1b[?u") {
            out.extend_from_slice(&data[last_copied..i]);
            i += 4;
            last_copied = i;
        } else if data[i..].starts_with(b"\x1b[?")
            || data[i..].starts_with(b"\x1b[>")
            || data[i..].starts_with(b"\x1b[=")
            || data[i..].starts_with(b"\x1b[<")
        {
            let sub = &data[i + 3..];
            let mut len = 0;
            let mut matched = false;
            for &b in sub {
                if b.is_ascii_digit() || b == b';' {
                    len += 1;
                } else if b == b'u' {
                    len += 1;
                    matched = true;
                    break;
                } else {
                    break;
                }
            }
            if matched {
                out.extend_from_slice(&data[last_copied..i]);
                i += 3 + len;
                last_copied = i;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    if last_copied == 0 {
        Cow::Borrowed(data)
    } else {
        out.extend_from_slice(&data[last_copied..]);
        Cow::Owned(out)
    }
}
