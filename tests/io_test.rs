use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use shell_panel::io::filter::sanitize_output_stream;
use shell_panel::io::key_event::{classify_key, encode_key_event, withheld_tab_bytes, ActionKey};
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
    let ctrl_c = make_key_event(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
        KeyEventKind::Press,
    );
    assert_eq!(classify_key(&ctrl_c), ActionKey::Passthrough);

    let ctrl_d = make_key_event(
        KeyCode::Char('d'),
        KeyModifiers::CONTROL,
        KeyEventKind::Press,
    );
    assert_eq!(classify_key(&ctrl_d), ActionKey::Passthrough);

    let alt_down = make_key_event(KeyCode::Down, KeyModifiers::ALT, KeyEventKind::Press);
    assert_eq!(classify_key(&alt_down), ActionKey::Passthrough);
}

#[test]
fn test_classify_enter_accepts_but_only_unmodified() {
    // With the dropdown open Enter takes the highlighted suggestion, like Tab.
    let enter = make_key_event(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&enter), ActionKey::AcceptSuggestion);

    // Shift+Enter and Ctrl+Enter keep their own meaning and their own encoding.
    for (modifiers, bytes) in [
        (KeyModifiers::SHIFT, b"\x1b[13;28;13;1;16;1_".as_slice()),
        (KeyModifiers::CONTROL, b"\x1b[13;28;13;1;8;1_".as_slice()),
    ] {
        let modified = make_key_event(KeyCode::Enter, modifiers, KeyEventKind::Press);
        assert_eq!(classify_key(&modified), ActionKey::Passthrough);
        assert_eq!(encode_key_event(&modified), bytes);
    }

    // An accepted Enter is consumed, but with no dropdown open it is still a plain carriage return.
    assert_eq!(encode_key_event(&enter), b"\r");

    // A release event is never an accept, or one Enter would accept twice.
    let release = make_key_event(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release);
    assert_eq!(classify_key(&release), ActionKey::Passthrough);
}

#[test]
fn test_classify_standard_keys_are_passthrough() {
    let backspace = make_key_event(KeyCode::Backspace, KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&backspace), ActionKey::Passthrough);

    let ch = make_key_event(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Press);
    assert_eq!(classify_key(&ch), ActionKey::Passthrough);
}

#[test]
fn test_withheld_tab_is_replayed_before_the_next_key() {
    // Nothing outstanding: the key goes to the shell on its own.
    assert_eq!(withheld_tab_bytes(false, KeyCode::Char('-')), b"");

    // `git sta<Tab> -s`: the Tab was answered with a chord, so the `\t` must precede the `-`.
    assert_eq!(withheld_tab_bytes(true, KeyCode::Char('-')), b"\t");
    assert_eq!(withheld_tab_bytes(true, KeyCode::Enter), b"\t");
    assert_eq!(withheld_tab_bytes(true, KeyCode::Backspace), b"\t");

    // Another Tab starts a request of its own instead of replaying the first one.
    assert_eq!(withheld_tab_bytes(true, KeyCode::Tab), b"");
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
    let ctrl_c = make_key_event(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
        KeyEventKind::Press,
    );
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
    let right_ctrl_shift =
        KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    assert_eq!(encode_key_event(&right_ctrl_shift), b"\x1b[1;6C".to_vec());

    let left_ctrl_shift = KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    assert_eq!(encode_key_event(&left_ctrl_shift), b"\x1b[1;6D".to_vec());

    let home_shift = KeyEvent::new(KeyCode::Home, KeyModifiers::SHIFT);
    assert_eq!(encode_key_event(&home_shift), b"\x1b[1;2H".to_vec());

    let end_ctrl_shift = KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    assert_eq!(encode_key_event(&end_ctrl_shift), b"\x1b[1;6F".to_vec());
}

#[test]
fn test_altgr_characters_are_sent_as_text() {
    // Windows reports AltGr as Ctrl+Alt.
    let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    for c in ['@', '?', '[', '\\', ']', '€', '{'] {
        let ev = make_key_event(KeyCode::Char(c), altgr, KeyEventKind::Press);
        assert_eq!(
            encode_key_event(&ev),
            c.to_string().into_bytes(),
            "AltGr char {:?}",
            c
        );
    }
    let ev = make_key_event(KeyCode::Char('a'), altgr, KeyEventKind::Press);
    assert_eq!(encode_key_event(&ev), vec![0x1b, 0x01]); // a real Ctrl+Alt+A chord
}

#[test]
fn test_backspace_and_enter_chords() {
    let key = |code, mods| encode_key_event(&make_key_event(code, mods, KeyEventKind::Press));
    assert_eq!(key(KeyCode::Backspace, KeyModifiers::NONE), vec![0x7f]);
    assert_eq!(key(KeyCode::Backspace, KeyModifiers::CONTROL), vec![0x08]); // BackwardKillWord
    assert_eq!(key(KeyCode::Backspace, KeyModifiers::ALT), vec![0x1b, 0x7f]);
    assert_eq!(key(KeyCode::Enter, KeyModifiers::NONE), b"\r".to_vec());
    // xterm has no sequence for these; ConPTY accepts win32-input-mode records.
    assert_eq!(
        key(KeyCode::Enter, KeyModifiers::SHIFT),
        b"\x1b[13;28;13;1;16;1_".to_vec()
    );
    assert_eq!(
        key(KeyCode::Enter, KeyModifiers::CONTROL),
        b"\x1b[13;28;13;1;8;1_".to_vec()
    );
}

#[test]
fn test_function_keys_keep_modifiers() {
    let key = |code, mods| encode_key_event(&make_key_event(code, mods, KeyEventKind::Press));
    assert_eq!(key(KeyCode::F(1), KeyModifiers::NONE), b"\x1bOP".to_vec());
    assert_eq!(
        key(KeyCode::F(1), KeyModifiers::SHIFT),
        b"\x1b[1;2P".to_vec()
    );
    assert_eq!(
        key(KeyCode::F(5), KeyModifiers::CONTROL),
        b"\x1b[15;5~".to_vec()
    );
    assert_eq!(
        key(KeyCode::F(12), KeyModifiers::NONE),
        b"\x1b[24~".to_vec()
    );
}

#[test]
fn test_alt_enter_is_a_record_not_a_carriage_return() {
    let enter = |m| encode_key_event(&make_key_event(KeyCode::Enter, m, KeyEventKind::Press));
    assert_eq!(enter(KeyModifiers::ALT), b"\x1b[13;28;13;1;2;1_".to_vec());
    assert_eq!(
        enter(KeyModifiers::ALT | KeyModifiers::SHIFT),
        b"\x1b[13;28;13;1;18;1_".to_vec()
    );
    // Unchanged:
    assert_eq!(enter(KeyModifiers::NONE), b"\r".to_vec());
    assert_eq!(
        enter(KeyModifiers::SHIFT),
        b"\x1b[13;28;13;1;16;1_".to_vec()
    );
    assert_eq!(
        enter(KeyModifiers::CONTROL),
        b"\x1b[13;28;13;1;8;1_".to_vec()
    );
}

#[test]
fn test_control_chords_for_space_and_brackets() {
    for (c, byte) in [(' ', 0x00u8), ('[', 0x1b), (']', 0x1d)] {
        let ev = make_key_event(KeyCode::Char(c), KeyModifiers::CONTROL, KeyEventKind::Press);
        assert_eq!(encode_key_event(&ev), vec![byte], "Ctrl+{c:?}");
    }
}

#[test]
fn test_vt_input_flag_is_set_and_cleared_without_touching_other_bits() {
    use shell_panel::io::console_mode::{with_vt_input, ENABLE_VIRTUAL_TERMINAL_INPUT};
    assert_eq!(ENABLE_VIRTUAL_TERMINAL_INPUT, 0x0200);
    assert_eq!(with_vt_input(0x01f0, true), 0x03f0);
    assert_eq!(with_vt_input(0x03f0, false), 0x01f0);
    assert_eq!(with_vt_input(0x03f0, true), 0x03f0);
    assert_eq!(with_vt_input(0x0000, false), 0x0000);
}

#[test]
fn test_unfinished_escape_sequences_are_told_from_finished_ones() {
    use shell_panel::io::key_event::ends_in_unfinished_escape;
    for unfinished in [
        &b"\x1b"[..],
        b"\x1b[",
        b"\x1b[1;",
        b"\x1b[?65;4",
        b"\x1bO",
        b"abc\x1b",
        b"\x1b[A\x1b[",
    ] {
        assert!(ends_in_unfinished_escape(unfinished), "{unfinished:?}");
    }
    for finished in [
        &b""[..],
        b"a",
        b"abc",
        b"\x1b[A",
        b"\x1b[25~",
        b"\x1b[1;5A",
        b"\x1bOP",
        b"\x1b[?65;4;6c",
        b"\x1bx",
        b"\x1b[Aabc",
    ] {
        assert!(!ends_in_unfinished_escape(finished), "{finished:?}");
    }
}

#[test]
fn test_program_mode_bytes_keep_modifiers_and_control_chars() {
    use shell_panel::io::key_event::program_mode_bytes;
    let key = |code, mods| make_key_event(code, mods, KeyEventKind::Press);
    assert_eq!(
        program_mode_bytes(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        vec![0x03]
    );
    assert_eq!(
        program_mode_bytes(&key(KeyCode::Char('a'), KeyModifiers::NONE)),
        b"a".to_vec()
    );
    assert_eq!(
        program_mode_bytes(&key(KeyCode::Char('\x1b'), KeyModifiers::NONE)),
        vec![0x1b]
    );
    assert_eq!(
        program_mode_bytes(&key(KeyCode::Char('x'), KeyModifiers::ALT)),
        vec![0x1b, b'x']
    );
    assert_eq!(
        program_mode_bytes(&key(KeyCode::Char('A'), KeyModifiers::SHIFT)),
        b"A".to_vec()
    );
    assert_eq!(
        program_mode_bytes(&key(KeyCode::Char('\u{e9}'), KeyModifiers::NONE)),
        "\u{e9}".as_bytes().to_vec()
    );
    // Not a control char, and the control char itself keeps its own byte whatever the modifiers.
    assert_eq!(
        program_mode_bytes(&key(KeyCode::Char('\x1b'), KeyModifiers::CONTROL)),
        vec![0x1b]
    );
}

#[test]
fn test_a_bare_esc_is_a_stray_host_sequence_byte() {
    use shell_panel::io::key_event::is_stray_escape;
    let key = |code, mods| make_key_event(code, mods, KeyEventKind::Press);
    assert!(is_stray_escape(&key(
        KeyCode::Char('\x1b'),
        KeyModifiers::NONE
    )));
    // Real keys: Esc, Ctrl+[, Ctrl+A, plain text.
    assert!(!is_stray_escape(&key(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(!is_stray_escape(&key(
        KeyCode::Char('['),
        KeyModifiers::CONTROL
    )));
    assert!(!is_stray_escape(&key(
        KeyCode::Char('a'),
        KeyModifiers::CONTROL
    )));
    assert!(!is_stray_escape(&key(
        KeyCode::Char('a'),
        KeyModifiers::NONE
    )));
    assert!(!is_stray_escape(&key(KeyCode::Tab, KeyModifiers::NONE)));
    // Enter, Tab and Backspace still queued as VT input when the console switches back.
    for c in ['\r', '\t', '\x7f', '\x08'] {
        assert!(
            !is_stray_escape(&key(KeyCode::Char(c), KeyModifiers::NONE)),
            "{c:?}"
        );
    }
}

#[test]
fn test_recorded_vt_input_bit_decides_what_is_restored() {
    use shell_panel::io::console_mode::{decode_recorded, encode_recorded};
    assert_eq!(decode_recorded(encode_recorded(Some(true))), Some(true));
    assert_eq!(decode_recorded(encode_recorded(Some(false))), Some(false));
    // Nothing recorded (never entered, or the mode was unreadable): leave the console alone.
    assert_eq!(decode_recorded(encode_recorded(None)), None);
    assert_eq!(decode_recorded(0), None);
}
