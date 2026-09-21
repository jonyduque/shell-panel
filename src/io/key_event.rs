use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum ActionKey {
    MenuUp,
    MenuDown,
    AcceptSuggestion,
    DismissMenu,
    Passthrough,
}

pub fn classify_key(event: &KeyEvent) -> ActionKey {
    // Ignore key release events on Windows to prevent duplicate menu navigation
    if event.kind == KeyEventKind::Release {
        return ActionKey::Passthrough;
    }

    // Do not hijack Ctrl or Alt key combinations
    if event.modifiers.contains(KeyModifiers::CONTROL)
        || event.modifiers.contains(KeyModifiers::ALT)
    {
        return ActionKey::Passthrough;
    }

    match event.code {
        KeyCode::Up => ActionKey::MenuUp,
        KeyCode::Down => ActionKey::MenuDown,
        KeyCode::Tab => ActionKey::AcceptSuggestion,
        KeyCode::BackTab => ActionKey::MenuUp, // Shift+Tab cycles backwards
        KeyCode::Esc => ActionKey::DismissMenu,
        _ => ActionKey::Passthrough,
    }
}

fn xterm_modifier_code(modifiers: KeyModifiers) -> u8 {
    let mut code = 1u8;
    if modifiers.contains(KeyModifiers::SHIFT) {
        code += 1;
    }
    if modifiers.contains(KeyModifiers::ALT) {
        code += 2;
    }
    if modifiers.contains(KeyModifiers::CONTROL) {
        code += 4;
    }
    code
}

/// Encodes a crossterm [`KeyEvent`] into raw VT/terminal byte sequence for writing to PTY.
pub fn encode_key_event(event: &KeyEvent) -> Vec<u8> {
    let mod_code = xterm_modifier_code(event.modifiers);
    match event.code {
        KeyCode::Char(c) => {
            if event.modifiers.contains(KeyModifiers::CONTROL) {
                if c.is_ascii_alphabetic() {
                    vec![(c.to_ascii_lowercase() as u8) - b'a' + 1]
                } else {
                    match c {
                        '@' | ' ' => vec![0],
                        '[' => vec![0x1b],
                        '\\' => vec![0x1c],
                        ']' => vec![0x1d],
                        '^' => vec![0x1e],
                        '_' => vec![0x1f],
                        '?' => vec![0x7f],
                        _ => {
                            let mut buf = [0u8; 4];
                            c.encode_utf8(&mut buf).as_bytes().to_vec()
                        }
                    }
                }
            } else if event.modifiers.contains(KeyModifiers::ALT) {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                let mut bytes = vec![0x1b];
                bytes.extend_from_slice(s.as_bytes());
                bytes
            } else {
                let mut buf = [0u8; 4];
                c.encode_utf8(&mut buf).as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![b'\x7f'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => vec![0x1b, b'[', b'Z'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => {
            if mod_code > 1 {
                format!("\x1b[1;{}A", mod_code).into_bytes()
            } else {
                b"\x1b[A".to_vec()
            }
        }
        KeyCode::Down => {
            if mod_code > 1 {
                format!("\x1b[1;{}B", mod_code).into_bytes()
            } else {
                b"\x1b[B".to_vec()
            }
        }
        KeyCode::Right => {
            if mod_code > 1 {
                format!("\x1b[1;{}C", mod_code).into_bytes()
            } else {
                b"\x1b[C".to_vec()
            }
        }
        KeyCode::Left => {
            if mod_code > 1 {
                format!("\x1b[1;{}D", mod_code).into_bytes()
            } else {
                b"\x1b[D".to_vec()
            }
        }
        KeyCode::Home => {
            if mod_code > 1 {
                format!("\x1b[1;{}H", mod_code).into_bytes()
            } else {
                b"\x1b[H".to_vec()
            }
        }
        KeyCode::End => {
            if mod_code > 1 {
                format!("\x1b[1;{}F", mod_code).into_bytes()
            } else {
                b"\x1b[F".to_vec()
            }
        }
        KeyCode::PageUp => {
            if mod_code > 1 {
                format!("\x1b[5;{}~", mod_code).into_bytes()
            } else {
                b"\x1b[5~".to_vec()
            }
        }
        KeyCode::PageDown => {
            if mod_code > 1 {
                format!("\x1b[6;{}~", mod_code).into_bytes()
            } else {
                b"\x1b[6~".to_vec()
            }
        }
        KeyCode::Delete => {
            if mod_code > 1 {
                format!("\x1b[3;{}~", mod_code).into_bytes()
            } else {
                b"\x1b[3~".to_vec()
            }
        }
        KeyCode::Insert => {
            if mod_code > 1 {
                format!("\x1b[2;{}~", mod_code).into_bytes()
            } else {
                b"\x1b[2~".to_vec()
            }
        }
        KeyCode::F(n) => match n {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            12 => b"\x1b[24~".to_vec(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}
