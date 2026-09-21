use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyEventState};
use shell_panel::io::filter::sanitize_output_stream;
use shell_panel::io::key_event::{classify_key, encode_key_event, ActionKey};
use shell_panel::io::raw_mode::RawModeGuard;

fn make_key_event(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind,
        state: KeyEventState::empty(),
    }
}

#[test]
fn test_classify_arrow_keys() {
    let down = make_key_event(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&down), ActionKey::MenuDown);

    let up = make_key_event(KeyCode::Up, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&up), ActionKey::MenuUp);
}

#[test]
fn test_classify_ignores_release_events() {
    let down_release = make_key_event(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Release);
    assert_eq!(classify_key(&down_release), ActionKey::Passthrough);

    let tab_release = make_key_event(KeyCode::Tab, KeyModifiers::NONE, KeyEventKind::Release);
    assert_eq!(classify_key(&tab_release), ActionKey::Passthrough);
}

#[test]
fn test_classify_tab_backtab_and_esc() {
    let tab = make_key_event(KeyCode::Tab, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&tab), ActionKey::AcceptSuggestion);

    let backtab = make_key_event(KeyCode::BackTab, KeyModifiers::SHIFT, KeyEventKind::Press);
    assert_eq!(classify_key(&backtab), ActionKey::MenuUp);

    let esc = make_key_event(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&esc), ActionKey::DismissMenu);
}

#[test]
fn test_classify_ctrl_and_alt_are_passthrough() {
    let ctrl_c = make_key_event(KeyCode::Char('c'), KeyModifiers::CONTROL, KeyEventKind::Press);
    assert_eq!(classify_key(&ctrl_c), ActionKey::Passthrough);

    let ctrl_d = make_key_event(KeyCode::Char('d'), KeyModifiers::CONTROL, KeyEventKind::Press);
    assert_eq!(classify_key(&ctrl_d), ActionKey::Passthrough);

    let alt_down = make_key_event(KeyCode::Down, KeyModifiers::ALT, KeyEventKind::Press);
    assert_eq!(classify_key(&alt_down), ActionKey::Passthrough);
}

#[test]
fn test_classify_standard_keys_are_passthrough() {
    let enter = make_key_event(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&enter), ActionKey::Passthrough);

    let backspace = make_key_event(KeyCode::Backspace, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&backspace), ActionKey::Passthrough);

    let ch = make_key_event(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&ch), ActionKey::Passthrough);
}

#[test]
fn test_sanitize_output_stream_win32_input_mode() {
    let raw = b"prefix\x1b[?9001hsuffix\x1b[?9001lfinal";
    let cleaned = sanitize_output_stream(raw);
    assert_eq!(cleaned.as_ref(), b"prefixsuffixfinal");
}

#[test]
fn test_sanitize_output_stream_kitty_protocol() {
    let raw = b"start\x1b[?1u\x1b[>1u\x1b[=2u\x1b[<3umiddle\x1b[?uend";
    let cleaned = sanitize_output_stream(raw);
    assert_eq!(cleaned.as_ref(), b"startmiddleend");
}

#[test]
fn test_sanitize_output_stream_preserves_standard_ansi() {
    let raw = b"\x1b[?25h\x1b[?25l\x1b[?1049h\x1b[0m\x1b[u";
    let cleaned = sanitize_output_stream(raw);
    assert_eq!(cleaned.as_ref(), raw);
    // Verify zero allocation when borrowed
    assert!(matches!(cleaned, std::borrow::Cow::Borrowed(_)));
}

#[test]
fn test_raw_mode_guard_creation() {
    if let Ok(guard) = RawModeGuard::enter() {
        drop(guard);
    }
}

#[test]
fn test_encode_key_event() {
    use shell_panel::io::key_event::encode_key_event;

    // Normal characters
    let a = make_key_event(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(encode_key_event(&a), b"a");

    // Enter, backspace, tab, esc
    let enter = make_key_event(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(encode_key_event(&enter), b"\r");

    let backspace = make_key_event(KeyCode::Backspace, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(encode_key_event(&backspace), b"\x7f");

    let tab = make_key_event(KeyCode::Tab, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(encode_key_event(&tab), b"\t");

    let esc = make_key_event(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(encode_key_event(&esc), b"\x1b");

    // Ctrl+C -> 0x03
    let ctrl_c = make_key_event(KeyCode::Char('c'), KeyModifiers::CONTROL, KeyEventKind::Press);
    assert_eq!(encode_key_event(&ctrl_c), vec![0x03]);

    // Arrows
    let up = make_key_event(KeyCode::Up, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(encode_key_event(&up), b"\x1b[A");

    let down = make_key_event(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(encode_key_event(&down), b"\x1b[B");
}

#[test]
fn test_backspace_emits_del_0x7f() {
    let ev = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(encode_key_event(&ev), vec![0x7f]);
}

#[test]
fn test_ctrl_shift_arrows_emit_xterm_code_6() {
    let right_ctrl_shift = KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(encode_key_event(&right_ctrl_shift), b"\x1b[1;6C".to_vec());

    let left_ctrl_shift = KeyEvent::new(
        KeyCode::Left,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(encode_key_event(&left_ctrl_shift), b"\x1b[1;6D".to_vec());

    let home_shift = KeyEvent::new(KeyCode::Home, KeyModifiers::SHIFT);
    assert_eq!(encode_key_event(&home_shift), b"\x1b[1;2H".to_vec());

    let end_ctrl_shift = KeyEvent::new(
        KeyCode::End,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(encode_key_event(&end_ctrl_shift), b"\x1b[1;6F".to_vec());
}


