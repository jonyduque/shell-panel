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
