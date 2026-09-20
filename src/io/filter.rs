/// Strip Kitty keyboard protocol & Win32 input mode upgrades
pub fn sanitize_output_stream(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i..].starts_with(b"\x1b[?9001h") || data[i..].starts_with(b"\x1b[?9001l") {
            i += 8;
        } else if data[i..].starts_with(b"\x1b[?u") {
            i += 4;
        } else if data[i..].starts_with(b"\x1b[?")
            || data[i..].starts_with(b"\x1b[>")
            || data[i..].starts_with(b"\x1b[=")
            || data[i..].starts_with(b"\x1b[<")
        {
            // Check for Kitty keyboard protocol sequences: ESC [ [?>=<] [0-9;]* u
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
                i += 3 + len;
            } else {
                out.push(data[i]);
                i += 1;
            }
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}
