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
        // Only an unmodified Enter accepts: Shift+Enter continues the line and keeps its encoding.
        // With no dropdown open only Tab requests a report, so a plain Enter still reaches the shell.
        KeyCode::Enter if !event.modifiers.contains(KeyModifiers::SHIFT) => {
            ActionKey::AcceptSuggestion
        }
        KeyCode::BackTab => ActionKey::MenuUp, // Shift+Tab cycles backwards
        KeyCode::Esc => ActionKey::DismissMenu,
        _ => ActionKey::Passthrough,
    }
}

/// Bytes to send before a key that arrives while a Tab's completion report is still outstanding.
///
/// That Tab never reached the shell — a reserved chord did — so it has to be replayed before the
/// new key, or `git sta<Tab> -s` would end up as `git sta -s`. Another Tab is the exception: it
/// requests a report of its own and keeps the Tab outstanding.
pub fn withheld_tab_bytes(tab_pending: bool, code: KeyCode) -> &'static [u8] {
    if tab_pending && code != KeyCode::Tab {
        b"\t"
    } else {
        b""
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

/// Control code produced by Ctrl+`c`, if any.
fn control_byte(c: char) -> Option<u8> {
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_lowercase() as u8 - b'a' + 1);
    }
    match c {
        '@' | ' ' => Some(0),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' => Some(0x1f),
        '?' => Some(0x7f),
        _ => None,
    }
}

/// Encodes a crossterm [`KeyEvent`] into raw VT/terminal byte sequence for writing to PTY.
pub fn encode_key_event(event: &KeyEvent) -> Vec<u8> {
    let mod_code = xterm_modifier_code(event.modifiers);
    match event.code {
        KeyCode::Char(c) => {
            let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
            let alt = event.modifiers.contains(KeyModifiers::ALT);
            let mut buf = [0u8; 4];
            let text = c.encode_utf8(&mut buf).as_bytes().to_vec();

            // Windows reports AltGr as Ctrl+Alt. A non-letter with both modifiers is the character
            // the keyboard layout produced (e.g. '@' via AltGr+Q on German layouts), not a chord.
            if ctrl && alt && !c.is_ascii_alphabetic() {
                return text;
            }

            let mut bytes = if ctrl {
                control_byte(c).map(|b| vec![b]).unwrap_or(text)
            } else {
                text
            };
            if alt {
                bytes.insert(0, 0x1b);
            }
            bytes
        }
        KeyCode::Enter => {
            // win32-input-mode record: Vk=13, Sc=28, Uc=13, KeyDown, control-key state, repeat 1.
            let shift = event.modifiers.contains(KeyModifiers::SHIFT);
            let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
            let alt = event.modifiers.contains(KeyModifiers::ALT);
            match (shift, ctrl, alt) {
                (false, false, false) => vec![b'\r'],
                _ => {
                    let state = (if shift { 0x10 } else { 0 })
                        | (if ctrl { 0x08 } else { 0 })
                        | (if alt { 0x02 } else { 0 });
                    format!("\x1b[13;28;13;1;{};1_", state).into_bytes()
                }
            }
        }
        KeyCode::Backspace => {
            if event.modifiers.contains(KeyModifiers::CONTROL) {
                vec![0x08]
            } else if event.modifiers.contains(KeyModifiers::ALT) {
                vec![0x1b, 0x7f]
            } else {
                vec![0x7f]
            }
        }
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
        KeyCode::F(n) => {
            // F1-F4 are SS3 P..S (CSI 1;m P..S with modifiers); the rest are CSI <code>~.
            let code = match n {
                5 => 15,
                6 => 17,
                7 => 18,
                8 => 19,
                9 => 20,
                10 => 21,
                11 => 23,
                12 => 24,
                _ => 0,
            };
            match (n, mod_code > 1) {
                (1..=4, false) => vec![0x1b, b'O', b'P' + (n - 1)],
                (1..=4, true) => {
                    format!("\x1b[1;{}{}", mod_code, (b'P' + (n - 1)) as char).into_bytes()
                }
                (5..=12, false) => format!("\x1b[{}~", code).into_bytes(),
                (5..=12, true) => format!("\x1b[{};{}~", code, mod_code).into_bytes(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// What program mode hands the PTY for one key event: the host's own text for a character, the
/// usual encoding for anything else. A classic-mode event read just after the console switched
/// to virtual terminal input (Ctrl+C typed right after Enter) still carries its modifiers, so a
/// printable character with Ctrl or Alt is encoded like a key press.
pub fn program_mode_bytes(event: &KeyEvent) -> Vec<u8> {
    match event.code {
        KeyCode::Char(c)
            if !c.is_control()
                && event
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            encode_key_event(event)
        }
        KeyCode::Char(c) => c.to_string().into_bytes(),
        _ => encode_key_event(event),
    }
}

/// An ESC with no modifiers, as a `Char`. A real Esc arrives as `KeyCode::Esc` and a real Ctrl+[
/// as `Char('[')` with Control, so this can only be the first byte of a host sequence that the
/// vendored crossterm patch turned into a key (virtual-key code 0). In prompt mode it must not
/// reach PSReadLine: an ESC there reverts the line. Other bare control characters are not
/// stray: Enter, Tab and Backspace typed just as the console switches back to classic mode are
/// still queued as virtual terminal input (`\r`, `\t`, DEL) and must get through.
pub fn is_stray_escape(event: &KeyEvent) -> bool {
    event.code == KeyCode::Char('\x1b') && event.modifiers == KeyModifiers::NONE
}

/// Whether `bytes` end inside an escape sequence: a lone ESC, an unfinished CSI (`ESC [` and
/// parameters without a final byte) or SS3 (`ESC O` without its letter). ConPTY reads an ESC
/// that ends a write as the Escape key, so such a buffer must wait for the rest.
pub fn ends_in_unfinished_escape(bytes: &[u8]) -> bool {
    let Some(esc) = bytes.iter().rposition(|&b| b == 0x1b) else {
        return false;
    };
    match &bytes[esc + 1..] {
        [] => true,
        [b'[', rest @ ..] => !rest.iter().any(|b| (0x40..=0x7e).contains(b)),
        [b'O'] => true,
        _ => false,
    }
}
