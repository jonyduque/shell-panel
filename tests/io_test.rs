use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use shell_panel::io::filter::sanitize_output_stream;
use shell_panel::io::key_event::{classify_key, ActionKey};
use shell_panel::io::raw_mode::RawModeGuard;

#[test]
fn test_classify_arrow_keys() {
    let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(classify_key(&down), ActionKey::MenuDown);

    let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(classify_key(&up), ActionKey::MenuUp);
}

#[test]
fn test_classify_tab_and_esc() {
    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(classify_key(&tab), ActionKey::AcceptSuggestion);

    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(classify_key(&esc), ActionKey::DismissMenu);
}

#[test]
fn test_classify_ctrl_keys_are_passthrough() {
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(classify_key(&ctrl_c), ActionKey::Passthrough);

    let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_eq!(classify_key(&ctrl_d), ActionKey::Passthrough);

    let ctrl_up = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL);
    assert_eq!(classify_key(&ctrl_up), ActionKey::Passthrough);

    let ctrl_down = KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL);
    assert_eq!(classify_key(&ctrl_down), ActionKey::Passthrough);
}

#[test]
fn test_classify_standard_keys_are_passthrough() {
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(classify_key(&enter), ActionKey::Passthrough);

    let char_a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    assert_eq!(classify_key(&char_a), ActionKey::Passthrough);

    let backspace = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(classify_key(&backspace), ActionKey::Passthrough);
}

#[test]
fn test_sanitize_output_stream_win32_input_mode() {
    let input = b"before\x1b[?9001hmiddle\x1b[?9001lafter";
    let sanitized = sanitize_output_stream(input);
    assert_eq!(sanitized, b"beforemiddleafter");
}

#[test]
fn test_sanitize_output_stream_kitty_protocol() {
    let input = b"abc\x1b[?udef\x1b[=1ughi\x1b[>31ujkl\x1b[<1umn";
    let sanitized = sanitize_output_stream(input);
    assert_eq!(sanitized, b"abcdefghijklmn");
}

#[test]
fn test_sanitize_output_stream_preserves_standard_ansi() {
    // \x1b[?25h (show cursor), \x1b[?25l (hide cursor), \x1b[?1049h (alt buffer), \x1b[u (restore cursor)
    let input = b"\x1b[?25h\x1b[?25l\x1b[?1049h\x1b[0m\x1b[uhello";
    let sanitized = sanitize_output_stream(input);
    assert_eq!(sanitized, input);
}

#[test]
fn test_raw_mode_guard_creation() {
    // Test that RawModeGuard::enter() can be called and returns Result<RawModeGuard>
    let res = RawModeGuard::enter();
    if let Ok(guard) = res {
        drop(guard);
    }
}
