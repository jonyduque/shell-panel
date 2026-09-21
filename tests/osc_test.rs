use shell_panel::shell::command_state::CommandState;
use shell_panel::shell::osc::{parse_osc_sequence, unescape_value, OscEvent};

#[test]
fn test_unknown_messages_are_ignored() {
    assert_eq!(parse_osc_sequence("1337;Other"), None);
    assert_eq!(parse_osc_sequence("6973;UNKNOWN"), None);
    assert_eq!(parse_osc_sequence("6973;PS"), None); // protocol v1
    assert_eq!(parse_osc_sequence(""), None);
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
    let mut state = CommandState::default();
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
        parse_osc_sequence(r"6973;RS;C:\x5cdev\x5cx64"),
        Some(OscEvent::ReadLineStarted {
            cwd: Some(r"C:\dev\x64".to_string())
        })
    );
    assert_eq!(
        parse_osc_sequence("6973;RS;"),
        Some(OscEvent::ReadLineStarted { cwd: None })
    );
    assert_eq!(parse_osc_sequence("6973;RE"), Some(OscEvent::ReadLineEnded));
}

#[test]
fn test_parse_completion_report() {
    // The JSON is escaped as a whole: `;` -> \x3b, and each backslash of JSON's `\\` -> \x5c.
    let payload = r#"6973;CMP;{"line":"cd .\x5c\x5cs\x3b","cursor":7,"replacementIndex":3,"replacementLength":4,"matches":[[".\x5c\x5csrc","src","ProviderContainer","C:\x5c\x5cp\x5c\x5csrc"]]}"#;
    assert_eq!(
        parse_osc_sequence(payload),
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
    assert_eq!(parse_osc_sequence("6973;CMP;not json"), None);
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
