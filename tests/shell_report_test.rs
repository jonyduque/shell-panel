mod common;

use std::time::Duration;

use common::{Terminal, COLS, ROWS};
use shell_panel::pty::conpty::{ConPtySession, SpawnOptions};
use shell_panel::pty::shell::detect_shell;
use shell_panel::shell::integration::{base64_encode, encoded_command, SCRIPT};
use shell_panel::shell::osc::{parse_osc_sequence, OscEvent, REPORT_REQUEST_KEY};

#[test]
fn test_base64_encode_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn test_encoded_command_fits_the_windows_command_line() {
    assert!(SCRIPT.contains("PSConsoleHostReadLine"));
    assert!(
        encoded_command().len() < 30_000,
        "CreateProcess limit is 32767 characters"
    );
}

fn last_report(raw: &[u8]) -> Option<OscEvent> {
    let marker = b"\x1b]6973;CMP;";
    let start = raw.windows(marker.len()).rposition(|w| w == marker)?;
    let end = start + raw[start..].iter().position(|&b| b == 0x07)?;
    parse_osc_sequence(std::str::from_utf8(&raw[start + 2..end]).ok()?)
}

#[test]
fn test_session_reports_readline_state_line_cursor_and_completions() {
    let ConPtySession { pair, child } = ConPtySession::spawn(
        detect_shell(None),
        COLS,
        ROWS,
        SpawnOptions { no_profile: true },
    )
    .unwrap();
    let mut term = Terminal::attach(pair.master, child);

    assert!(
        term.wait_for_raw(b"\x1b]6973;RS;", Duration::from_secs(40)),
        "no ReadLine marker"
    );

    // A variable that exists only in this session proves completion runs inside it.
    term.send(b"$sp_report_zz = 1\r");
    assert!(term.wait_for_raw(b"\x1b]6973;RE\x07", Duration::from_secs(15)));

    term.send("echo 'ação' > $sp_report_".as_bytes());
    assert!(term.wait_for_text("$sp_report_", Duration::from_secs(15)));
    term.send(REPORT_REQUEST_KEY);
    assert!(
        term.wait_until(Duration::from_secs(20), |t| last_report(&t.raw).is_some()),
        "no report"
    );

    let Some(OscEvent::Report(report)) = last_report(&term.raw) else {
        unreachable!()
    };
    assert_eq!(report.line, "echo 'ação' > $sp_report_");
    assert_eq!(
        report.text_before_cursor(),
        Some("echo 'ação' > $sp_report_")
    );
    let (start, end) = report.replacement_range().expect("range");
    assert_eq!(&report.line[start..end], "$sp_report_");
    assert!(
        report.matches.iter().any(|m| m.0 == "$sp_report_zz"),
        "matches: {:?}",
        report.matches
    );
}
