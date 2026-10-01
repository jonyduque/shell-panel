mod common;

use std::path::Path;
use std::time::Duration;

use common::{Terminal, COLS, ROWS};
use portable_pty::CommandBuilder;
use shell_panel::pty::conpty::{ConPtySession, SpawnOptions};
use shell_panel::pty::shell::detect_shell;
use shell_panel::shell::integration::{base64_encode, encoded_command, script, SCRIPT};
use shell_panel::shell::osc::{parse_osc_sequence, OscEvent, REPORT_REQUEST_KEY};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

/// Removes `dir` once the killed shell has let go of it as its working directory.
fn remove_dir_when_released(dir: &std::path::Path) {
    for _ in 0..50 {
        if std::fs::remove_dir_all(dir).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

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
        encoded_command(TOKEN).len() < 30_000,
        "CreateProcess limit is 32767 characters"
    );
}

/// A real shell that runs `prelude` before the integration script, the way a profile would.
fn shell_with_prelude(prelude: &str) -> Terminal {
    let script = format!("{prelude}\n{}", script(TOKEN));
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut cmd = CommandBuilder::new(detect_shell(None).executable_name());
    cmd.arg("-NoLogo");
    cmd.arg("-NoProfile");
    cmd.arg("-NoExit");
    cmd.arg("-EncodedCommand");
    cmd.arg(base64_encode(&utf16));
    cmd.env_remove("SHELL_PANEL_SESSION");
    Terminal::spawn(cmd)
}

/// Asks the session for both prediction options and checks the view it answers with.
fn assert_prediction_view(term: &mut Terminal, expected_view: &str) {
    assert!(
        term.wait_for_raw(
            format!("\x1b]6973;{TOKEN};RS;").as_bytes(),
            Duration::from_secs(40)
        ),
        "no ReadLine marker"
    );
    let option = "(Get-PSReadLineOption)";
    term.send(
        format!("\"VIEW=$({option}.PredictionViewStyle) SRC=$({option}.PredictionSource)\"\r")
            .as_bytes(),
    );
    // The question echoes as `$(...)`, so only the answer spells the source out.
    assert!(
        term.wait_for_text("SRC=History", Duration::from_secs(20)),
        "no answer, screen: {}",
        term.screen()
    );
    let expected = format!("VIEW={expected_view} SRC=History");
    assert!(
        term.screen().contains(&expected),
        "expected `{expected}`, screen: {}",
        term.screen()
    );
}

#[test]
fn test_prediction_list_view_is_switched_off_for_the_session() {
    // PSReadLine's list view draws over the rows shell-panel's dropdown needs.
    let mut term = shell_with_prelude(
        "Set-PSReadLineOption -PredictionSource History -PredictionViewStyle ListView",
    );
    assert_prediction_view(&mut term, "InlineView");
}

#[test]
fn test_an_inline_prediction_session_is_left_alone() {
    let mut term = shell_with_prelude(
        "Set-PSReadLineOption -PredictionSource History -PredictionViewStyle InlineView",
    );
    assert_prediction_view(&mut term, "InlineView");
}

fn last_report(raw: &[u8], token: &str) -> Option<OscEvent> {
    let marker = format!("\x1b]6973;{token};CMP;");
    let marker = marker.as_bytes();
    let start = raw.windows(marker.len()).rposition(|w| w == marker)?;
    let end = start + raw[start..].iter().position(|&b| b == 0x07)?;
    parse_osc_sequence(std::str::from_utf8(&raw[start + 2..end]).ok()?, token)
}

#[test]
fn test_session_reports_readline_state_line_cursor_and_completions() {
    let ConPtySession { pair, child, token } = ConPtySession::spawn(
        detect_shell(None),
        COLS,
        ROWS,
        SpawnOptions { no_profile: true },
    )
    .unwrap();
    let mut term = Terminal::attach(pair.master, child);

    assert!(
        term.wait_for_raw(
            format!("\x1b]6973;{token};RS;").as_bytes(),
            Duration::from_secs(40)
        ),
        "no ReadLine marker"
    );

    // A variable that exists only in this session proves completion runs inside it.
    term.send(b"$sp_report_zz = 1\r");
    assert!(term.wait_for_raw(
        format!("\x1b]6973;{token};RE\x07").as_bytes(),
        Duration::from_secs(15)
    ));

    term.send("echo 'ação' > $sp_report_".as_bytes());
    assert!(term.wait_for_text("$sp_report_", Duration::from_secs(15)));
    term.send(REPORT_REQUEST_KEY);
    assert!(
        term.wait_until(Duration::from_secs(20), |t| last_report(&t.raw, &token)
            .is_some()),
        "no report"
    );

    let Some(OscEvent::Report(report)) = last_report(&term.raw, &token) else {
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

/// The integration script in a real shell started in `dir`, with the test token.
fn shell_in(dir: &Path) -> Terminal {
    let utf16: Vec<u8> = script(TOKEN)
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut cmd = CommandBuilder::new(detect_shell(None).executable_name());
    for arg in ["-NoLogo", "-NoProfile", "-NoExit", "-EncodedCommand"] {
        cmd.arg(arg);
    }
    cmd.arg(base64_encode(&utf16));
    cmd.cwd(dir);
    cmd.env_remove("SHELL_PANEL_SESSION");
    Terminal::spawn(cmd)
}

fn last_cwd(raw: &[u8]) -> Option<String> {
    let marker = format!("\x1b]6973;{TOKEN};RS;");
    let marker = marker.as_bytes();
    let start = raw.windows(marker.len()).rposition(|w| w == marker)?;
    let end = start + raw[start..].iter().position(|&b| b == 0x07)?;
    match parse_osc_sequence(std::str::from_utf8(&raw[start + 2..end]).ok()?, TOKEN)? {
        OscEvent::ReadLineStarted { cwd } => cwd,
        _ => None,
    }
}

#[test]
fn test_text_outside_the_console_code_page_survives_the_report() {
    let dir = std::env::temp_dir().join(format!("sp_cp_日本😀_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut term = shell_in(&dir);
    assert!(
        term.wait_until(Duration::from_secs(40), |t| last_cwd(&t.raw).is_some()),
        "no ReadLine marker"
    );
    // The location is reported intact, not as `sp_cp_???_`. Only the last component is compared:
    // %TEMP% can be an 8.3 short path (`C:\Users\RUNNER~1\...` on CI runners), which PowerShell
    // reports in its long form.
    let cwd = last_cwd(&term.raw).expect("cwd");
    let leaf = dir.file_name().unwrap().to_str().unwrap();
    assert!(
        cwd.ends_with(&format!("\\{leaf}")),
        "cwd: {cwd:?}, expected to end with {leaf:?}"
    );

    let line = "echo '日本😀ação' > $sp_cp_";
    term.send(line.as_bytes());
    assert!(term.wait_for_text("$sp_cp_", Duration::from_secs(15)));
    term.send(REPORT_REQUEST_KEY);
    assert!(
        term.wait_until(Duration::from_secs(20), |t| last_report(&t.raw, TOKEN)
            .is_some()),
        "no report"
    );
    let Some(OscEvent::Report(report)) = last_report(&term.raw, TOKEN) else {
        unreachable!()
    };
    assert_eq!(report.line, line);
    // 😀 is two UTF-16 units: the cursor counts them both.
    assert_eq!(report.cursor, line.encode_utf16().count());
    assert_eq!(report.text_before_cursor(), Some(line));
    drop(term);
    remove_dir_when_released(&dir);
}

#[test]
fn test_completion_of_a_name_outside_the_code_page_is_not_a_wildcard() {
    let dir = std::env::temp_dir().join(format!("sp_cpname_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("日本.txt"), b"x").unwrap();
    let mut term = shell_in(&dir);
    assert!(term.wait_until(Duration::from_secs(40), |t| last_cwd(&t.raw).is_some()));
    term.send(b"Get-Item .\\");
    assert!(term.wait_for_text("Get-Item .\\", Duration::from_secs(15)));
    term.send(REPORT_REQUEST_KEY);
    assert!(
        term.wait_until(Duration::from_secs(20), |t| last_report(&t.raw, TOKEN)
            .is_some())
    );
    let Some(OscEvent::Report(report)) = last_report(&term.raw, TOKEN) else {
        unreachable!()
    };
    assert!(
        report.matches.iter().any(|m| m.0 == ".\\日本.txt"),
        "matches: {:?}",
        report.matches
    );
    drop(term);
    remove_dir_when_released(&dir);
}
