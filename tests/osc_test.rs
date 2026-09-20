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
}

#[test]
fn test_unescape_cwd() {
    // Escaped backslashes
    assert_eq!(unescape_cwd("C:\\\\Users\\\\test"), "C:\\Users\\test");
    // Single backslashes in Windows paths preserved
    assert_eq!(unescape_cwd("C:\\Users\\test"), "C:\\Users\\test");
    // Hex escape \x3b (semicolon)
    assert_eq!(unescape_cwd("path\\x3bwith\\x3bsemicolons"), "path;with;semicolons");
    // Hex escape \x5c (backslash)
    assert_eq!(unescape_cwd("C:\\x5cUsers\\x5ctest"), "C:\\Users\\test");
    // Combined
    assert_eq!(unescape_cwd("C:\\x5cUsers\\x5cfoo\\x3bbar"), "C:\\Users\\foo;bar");
}

#[test]
fn test_parse_osc_with_escapes() {
    assert_eq!(
        parse_osc_sequence("6973;CWD;C:\\Users\\test\\x3bproject"),
        Some(OscEvent::Cwd("C:\\Users\\test;project".to_string()))
    );
    assert_eq!(
        parse_osc_sequence("6973;CWD;C:\\x5cUsers\\x5ctest"),
        Some(OscEvent::Cwd("C:\\Users\\test".to_string()))
    );
}

#[test]
fn test_command_state_handle_osc() {
    let mut state = CommandState::default();

    assert_eq!(state.in_prompt, false);
    assert_eq!(state.has_output, false);
    assert_eq!(state.prompt_line, None);
    assert_eq!(state.prompt_end_x, None);
    assert_eq!(state.cwd, "");
    assert_eq!(state.command_text, "");

    // 1. CWD update
    state.handle_osc(OscEvent::Cwd("C:\\Users\\tester".to_string()), 0, 0);
    assert_eq!(state.cwd, "C:\\Users\\tester");

    // 2. PromptStarted at line 5, col 0
    state.handle_osc(OscEvent::PromptStarted, 5, 0);
    assert_eq!(state.in_prompt, true);
    assert_eq!(state.has_output, false);
    assert_eq!(state.prompt_line, Some(5));
    assert_eq!(state.prompt_end_x, None);
    assert_eq!(state.command_text, "");

    // Simulate typing and output
    state.command_text = "Get-ChildItem".to_string();
    state.has_output = true;

    // 3. PromptEnded at col 24
    state.handle_osc(OscEvent::PromptEnded, 5, 24);
    assert_eq!(state.in_prompt, false);
    assert_eq!(state.prompt_line, Some(5));
    assert_eq!(state.prompt_end_x, Some(24));
    // command_text and has_output should remain intact after PromptEnded
    assert_eq!(state.command_text, "Get-ChildItem");
    assert_eq!(state.has_output, true);

    // 4. Next PromptStarted should reset prompt_end_x, command_text, has_output
    state.handle_osc(OscEvent::PromptStarted, 6, 0);
    assert_eq!(state.in_prompt, true);
    assert_eq!(state.has_output, false);
    assert_eq!(state.prompt_line, Some(6));
    assert_eq!(state.prompt_end_x, None);
    assert_eq!(state.command_text, "");
}
