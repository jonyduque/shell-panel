use shell_panel::shell::command_state::CommandState;
use shell_panel::shell::osc::{parse_osc_sequence, unescape_cwd, OscEvent};

#[test]
fn test_parse_osc_sequences() {
    assert_eq!(parse_osc_sequence("6973;PS"), Some(OscEvent::PromptStarted));
    assert_eq!(parse_osc_sequence("6973;PE"), Some(OscEvent::PromptEnded));
    assert_eq!(
        parse_osc_sequence("6973;CWD;C:\\Users\\test"),
        Some(OscEvent::Cwd("C:\\Users\\test".to_string()))
    );
    assert_eq!(parse_osc_sequence("1337;Other"), None);
    assert_eq!(parse_osc_sequence("6973;UNKNOWN"), None);
    assert_eq!(parse_osc_sequence(""), None);
    assert_eq!(
        parse_osc_sequence("6973;CWD;"),
        Some(OscEvent::Cwd("".to_string()))
    );
}

#[test]
fn test_unescape_cwd() {
    assert_eq!(unescape_cwd(r"C:\\Users\\test"), r"C:\Users\test");
    assert_eq!(unescape_cwd(r"C:\Users\test"), r"C:\Users\test");
    assert_eq!(
        unescape_cwd(r"path\x3bwith\x3bsemicolon"),
        "path;with;semicolon"
    );
    assert_eq!(
        unescape_cwd(r"path\x5cwith\x5cbackslash"),
        r"path\with\backslash"
    );
    assert_eq!(unescape_cwd(r"mixed\\path\x3btest"), r"mixed\path;test");
}

#[test]
fn test_unescape_cwd_utf8_multibyte() {
    // "café": 'é' is UTF-8 0xc3 0xa9
    assert_eq!(unescape_cwd(r"C:\Users\caf\xc3\xa9"), "C:\\Users\\café");
    // Invalid / partial hex
    assert_eq!(unescape_cwd(r"C:\path\x"), r"C:\path\x");
    assert_eq!(unescape_cwd(r"C:\path\xZ1"), r"C:\path\xZ1");
}

#[test]
fn test_parse_osc_with_escapes() {
    assert_eq!(
        parse_osc_sequence(r"6973;CWD;C:\\Users\\john\x3bdoe"),
        Some(OscEvent::Cwd(r"C:\Users\john;doe".to_string()))
    );
    assert_eq!(
        parse_osc_sequence(r"6973;CWD;D:\\proj\x5csubdir"),
        Some(OscEvent::Cwd(r"D:\proj\subdir".to_string()))
    );
}

#[test]
fn test_command_state_handle_osc() {
    let mut state = CommandState::default();
    assert_eq!(state.prompt_line, None);
    assert_eq!(state.prompt_end_x, None);
    assert!(!state.in_prompt);

    state.handle_osc(OscEvent::Cwd("C:\\Project".to_string()), 0, 0);
    assert_eq!(state.cwd, "C:\\Project");

    // Multi-line prompt starts at row 2, ends at row 4
    state.handle_osc(OscEvent::PromptStarted, 2, 0);
    assert!(state.in_prompt);
    assert!(!state.has_output);
    assert_eq!(state.prompt_line, Some(2));
    assert_eq!(state.prompt_end_x, None);

    state.command_text = "partial".to_string();

    state.handle_osc(OscEvent::PromptEnded, 4, 15);
    assert!(!state.in_prompt);
    assert_eq!(state.prompt_line, Some(4)); // updated to actual input line
    assert_eq!(state.prompt_end_x, Some(15));

    // Next prompt clears command_text and updates prompt_line
    state.handle_osc(OscEvent::PromptStarted, 5, 0);
    assert!(state.in_prompt);
    assert_eq!(state.prompt_line, Some(5));
    assert_eq!(state.prompt_end_x, None);
    assert!(state.command_text.is_empty());
}
