use shell_panel::shell::command_state::CommandState;
use shell_panel::shell::integration::{new_session_token, script};
use shell_panel::shell::osc::{parse_osc_sequence, unescape_value, OscEvent};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn test_unknown_messages_are_ignored() {
    assert_eq!(parse_osc_sequence("1337;Other", TOKEN), None);
    assert_eq!(
        parse_osc_sequence(&format!("6973;{TOKEN};UNKNOWN"), TOKEN),
        None
    );
    assert_eq!(parse_osc_sequence(&format!("6973;{TOKEN};PS"), TOKEN), None); // protocol v1
    assert_eq!(parse_osc_sequence("", TOKEN), None);
}

#[test]
fn test_unescape_value() {
    assert_eq!(unescape_value(r"C:\\Users\\test"), r"C:\Users\test");
    assert_eq!(unescape_value(r"C:\Users\test"), r"C:\Users\test");
    assert_eq!(
        unescape_value(r"path\x3bwith\x3bsemicolon"),
        "path;with;semicolon"
    );
    assert_eq!(
        unescape_value(r"path\x5cwith\x5cbackslash"),
        r"path\with\backslash"
    );
    assert_eq!(unescape_value(r"mixed\\path\x3btest"), r"mixed\path;test");
}

#[test]
fn test_unescape_value_utf8_multibyte() {
    // "café": 'é' is UTF-8 0xc3 0xa9
    assert_eq!(unescape_value(r"C:\Users\caf\xc3\xa9"), "C:\\Users\\café");
    // Invalid / partial hex
    assert_eq!(unescape_value(r"C:\path\x"), r"C:\path\x");
    assert_eq!(unescape_value(r"C:\path\xZ1"), r"C:\path\xZ1");
}

use shell_panel::shell::report::{ShellMatch, ShellReport};

#[test]
fn test_command_state_follows_readline_markers() {
    let mut state = CommandState::new(TOKEN);
    assert!(!state.reading_line);

    state.handle_osc(OscEvent::ReadLineStarted {
        cwd: Some("C:\\Project".into()),
    });
    assert!(state.reading_line);
    assert_eq!(state.cwd, "C:\\Project");

    // A non-filesystem location (e.g. HKLM:) keeps the last filesystem cwd.
    state.handle_osc(OscEvent::ReadLineStarted { cwd: None });
    assert_eq!(state.cwd, "C:\\Project");

    let report = ShellReport {
        line: "git".into(),
        cursor: 3,
        replacement_index: 0,
        replacement_length: 3,
        matches: vec![],
    };
    state.request_report();
    state.handle_osc(OscEvent::Report(report.clone()));
    assert_eq!(state.report, Some(report));

    state.handle_osc(OscEvent::ReadLineEnded);
    assert!(!state.reading_line);
    state.handle_osc(OscEvent::ReadLineStarted { cwd: None });
    assert_eq!(state.report, None);
}

#[test]
fn test_parse_readline_markers() {
    assert_eq!(
        parse_osc_sequence(&format!(r"6973;{TOKEN};RS;C:\x5cdev\x5cx64"), TOKEN),
        Some(OscEvent::ReadLineStarted {
            cwd: Some(r"C:\dev\x64".to_string())
        })
    );
    assert_eq!(
        parse_osc_sequence(&format!("6973;{TOKEN};RS;"), TOKEN),
        Some(OscEvent::ReadLineStarted { cwd: None })
    );
    assert_eq!(
        parse_osc_sequence(&format!("6973;{TOKEN};RE"), TOKEN),
        Some(OscEvent::ReadLineEnded)
    );
}

#[test]
fn test_parse_completion_report() {
    // The JSON is escaped as a whole: `;` -> \x3b, and each backslash of JSON's `\\` -> \x5c.
    let payload = format!(
        r#"6973;{TOKEN};CMP;{{"line":"cd .\x5c\x5cs\x3b","cursor":7,"replacementIndex":3,"replacementLength":4,"matches":[[".\x5c\x5csrc","src","ProviderContainer","C:\x5c\x5cp\x5c\x5csrc"]]}}"#
    );
    assert_eq!(
        parse_osc_sequence(&payload, TOKEN),
        Some(OscEvent::Report(ShellReport {
            line: r"cd .\s;".to_string(),
            cursor: 7,
            replacement_index: 3,
            replacement_length: 4,
            matches: vec![ShellMatch(
                r".\src".into(),
                "src".into(),
                "ProviderContainer".into(),
                r"C:\p\src".into()
            )],
        }))
    );
    assert_eq!(
        parse_osc_sequence(&format!("6973;{TOKEN};CMP;not json"), TOKEN),
        None
    );
}

#[test]
fn test_report_ranges() {
    let report = ShellReport {
        line: "echo ação Get-ChildItem".into(),
        cursor: 18, // after "Get-Chil" (UTF-16 units)
        replacement_index: 10,
        replacement_length: 13,
        matches: vec![],
    };
    assert_eq!(report.text_before_cursor(), Some("echo ação Get-Chil"));
    let (start, end) = report.replacement_range().unwrap();
    assert_eq!(&report.line[start..end], "Get-ChildItem");

    let bogus = ShellReport {
        replacement_index: -1,
        ..report.clone()
    };
    assert_eq!(bogus.replacement_range(), None);
    // A range that does not contain the cursor cannot be applied with Backspace/Delete.
    let away = ShellReport {
        replacement_index: 0,
        replacement_length: 4,
        ..report
    };
    assert_eq!(away.replacement_range(), None);

    // A range that starts after the cursor cannot be applied either; slicing it would panic.
    let ahead = ShellReport {
        line: "abcdef".into(),
        cursor: 2,
        replacement_index: 3,
        replacement_length: 2,
        matches: vec![],
    };
    assert_eq!(ahead.replacement_range(), None);
}

#[test]
fn test_message_without_the_session_token_is_ignored() {
    // The protocol before this change, and what a printed file can contain.
    assert_eq!(parse_osc_sequence("6973;RE", TOKEN), None);
    assert_eq!(parse_osc_sequence("6973;RS;C:\\x5cp", TOKEN), None);
    // A different token.
    assert_eq!(
        parse_osc_sequence("6973;ffffffffffffffffffffffffffffffff;RE", TOKEN),
        None
    );
    // The token as a prefix of a longer one is not the token.
    assert_eq!(
        parse_osc_sequence(&format!("6973;{TOKEN}0;RE"), TOKEN),
        None
    );
    // An empty session token never matches, not even an empty field.
    assert_eq!(parse_osc_sequence("6973;;RE", ""), None);
    assert_eq!(
        parse_osc_sequence(&format!("6973;{TOKEN};RE"), TOKEN),
        Some(OscEvent::ReadLineEnded)
    );
}

#[test]
fn test_session_tokens_are_fresh_hex() {
    let a = new_session_token();
    let b = new_session_token();
    assert_eq!(a.len(), 32);
    assert!(a
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(a, b);
}

#[test]
fn test_script_carries_the_token_before_the_integration() {
    let s = script(TOKEN);
    let assignment = format!("$Global:__SP_Token = '{TOKEN}'");
    assert!(
        s.starts_with(&assignment),
        "script starts with: {:?}",
        &s[..80.min(s.len())]
    );
    assert!(s.contains("Set-PSReadLineKeyHandler"));
}

fn report_of(line: &str) -> ShellReport {
    ShellReport {
        line: line.into(),
        cursor: line.encode_utf16().count(),
        replacement_index: 0,
        replacement_length: 0,
        matches: vec![],
    }
}

fn reading() -> CommandState {
    let mut state = CommandState::new(TOKEN);
    state.handle_osc(OscEvent::ReadLineStarted { cwd: None });
    state
}

#[test]
fn test_unrequested_report_is_dropped() {
    let mut state = reading();
    state.handle_osc(OscEvent::Report(report_of("x")));
    assert_eq!(state.report, None);
}

#[test]
fn test_only_the_answer_to_the_last_request_is_kept() {
    let mut state = reading();
    state.request_report(); // Tab 1
    state.abandon_report(); // a key, or the 3 s timeout
    state.request_report(); // Tab 2
    state.handle_osc(OscEvent::Report(report_of("old")));
    assert_eq!(
        state.report, None,
        "the answer to Tab 1 was taken for Tab 2"
    );
    state.handle_osc(OscEvent::Report(report_of("new")));
    assert_eq!(state.report.take().map(|r| r.line), Some("new".to_string()));
}

#[test]
fn test_late_answer_after_abandon_is_dropped() {
    let mut state = reading();
    state.request_report();
    state.abandon_report();
    state.handle_osc(OscEvent::Report(report_of("late")));
    assert_eq!(state.report, None);
}

#[test]
fn test_answer_is_kept_once() {
    let mut state = reading();
    state.request_report();
    state.handle_osc(OscEvent::Report(report_of("a")));
    assert!(state.report.take().is_some());
    // A duplicate (or forged) second report finds no Tab waiting.
    state.handle_osc(OscEvent::Report(report_of("b")));
    assert_eq!(state.report, None);
}

#[test]
fn test_new_line_forgets_requests_that_were_never_answered() {
    let mut state = reading();
    state.request_report(); // chord swallowed: no report will ever come
    state.abandon_report();
    state.handle_osc(OscEvent::ReadLineStarted { cwd: None });
    state.request_report();
    state.handle_osc(OscEvent::Report(report_of("fresh")));
    assert_eq!(
        state.report.take().map(|r| r.line),
        Some("fresh".to_string())
    );
}
