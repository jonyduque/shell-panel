mod common;

use std::path::Path;
use std::time::Duration;

use common::{Terminal, COLS, ROWS};
use portable_pty::CommandBuilder;
use shell_panel::pty::conpty::{ConPtySession, SpawnOptions};
use shell_panel::pty::shell::detect_shell;
use shell_panel::shell::integration::{base64_encode, encoded_command, script, SCRIPT};
use shell_panel::shell::osc::{parse_osc_sequence, OscEvent, REPORT_REQUEST_KEY};
use shell_panel::shell::report::ShellReport;

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

/// A shell with the test token, started in a fresh directory, waiting at its first prompt.
fn ready_shell(tag: &str) -> (Terminal, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("sp_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut term = shell_in(&dir);
    assert!(
        term.wait_until(Duration::from_secs(40), |t| last_cwd(&t.raw).is_some()),
        "no ReadLine marker"
    );
    (term, dir)
}

fn request_report(term: &mut Terminal) -> ShellReport {
    term.send(REPORT_REQUEST_KEY);
    assert!(
        term.wait_until(Duration::from_secs(20), |t| last_report(&t.raw, TOKEN)
            .is_some()),
        "no report, screen: {}",
        term.screen()
    );
    let Some(OscEvent::Report(report)) = last_report(&term.raw, TOKEN) else {
        unreachable!()
    };
    report
}

fn count_of(raw: &[u8], needle: &str) -> usize {
    let n = needle.as_bytes();
    raw.windows(n.len()).filter(|w| *w == n).count()
}

#[test]
fn test_session_survives_a_script_that_clears_the_global_variables() {
    let (mut term, dir) = ready_shell("novars");
    let rs = format!("\x1b]6973;{TOKEN};RS;");
    let re = format!("\x1b]6973;{TOKEN};RE\x07");
    // A script run in the session may clear the global scope; neither the token nor the original
    // ReadLine may live there.
    term.send(b"Remove-Variable * -Scope Global -ErrorAction SilentlyContinue; 'CLEAR' + 'ED'\r");
    assert!(term.wait_for_text("CLEARED", Duration::from_secs(15)));
    let before = (count_of(&term.raw, &rs), count_of(&term.raw, &re));
    // Enter ends the line (RE) and the next prompt starts a new one (RS).
    term.send(b"'AFT' + 'ER'\r");
    assert!(
        term.wait_until(Duration::from_secs(15), |t| count_of(&t.raw, &rs)
            > before.0
            && count_of(&t.raw, &re) > before.1),
        "no RE/RS after Enter, screen: {}",
        term.screen()
    );
    term.send(b"echo hi");
    assert!(term.wait_for_text("echo hi", Duration::from_secs(15)));
    let report = request_report(&mut term);
    assert_eq!(report.line, "echo hi");
    drop(term);
    remove_dir_when_released(&dir);
}

#[test]
fn test_loading_the_script_twice_does_not_wrap_the_readline_twice() {
    let (mut term, dir) = ready_shell("twice");
    let file = dir.join("again.ps1");
    std::fs::write(&file, script(TOKEN)).unwrap();
    term.send(b"Invoke-Expression (Get-Content -Raw .\\again.ps1); 'LOAD' + 'ED'\r");
    assert!(term.wait_for_text("LOADED", Duration::from_secs(20)));
    let rs = format!("\x1b]6973;{TOKEN};RS;");
    // The prompt after the load line sends its own RS; let it arrive before counting.
    term.drain(Duration::from_millis(1500));
    let before = count_of(&term.raw, &rs);
    term.send(b"'ONE' + 'LINE'\r");
    assert!(term.wait_for_text("ONELINE", Duration::from_secs(15)));
    // Let any second marker arrive before counting.
    term.drain(Duration::from_millis(1500));
    assert_eq!(count_of(&term.raw, &rs) - before, 1, "one RS per prompt");
    drop(term);
    remove_dir_when_released(&dir);
}

#[test]
fn test_lone_surrogate_in_the_line_becomes_the_replacement_character() {
    let (mut term, dir) = ready_shell("surrogate");
    // The handler inserts a lone high surrogate, which a terminal cannot be typed into.
    term.send(
        b"Set-PSReadLineKeyHandler -Chord 'Ctrl+Alt+Shift+F11' -ScriptBlock { \
[Microsoft.PowerShell.PSConsoleReadLine]::Insert('a' + [string][char]0xD83D + 'b') }; 'BOU' + 'ND'\r",
    );
    assert!(term.wait_for_text("BOUND", Duration::from_secs(15)));
    term.send(b"\x1b[23;8~");
    assert!(
        term.wait_until(Duration::from_secs(15), |t| t.screen().contains("a")
            && t.screen().contains("b")
            && t.screen()
                .lines()
                .any(|l| l.contains("> a") && l.trim_end().ends_with('b')))
    );
    let report = request_report(&mut term);
    assert_eq!(report.line, "a\u{FFFD}b");
    assert_eq!(report.cursor, 3);
    drop(term);
    remove_dir_when_released(&dir);
}

#[test]
fn test_backslash_and_semicolon_next_to_non_ascii_round_trip() {
    let (mut term, dir) = ready_shell("escapes");
    let line = r"echo 'a\b;c\\d é;日本\x41\' > C:\tmp\";
    term.send(line.as_bytes());
    assert!(term.wait_for_text(r"C:\tmp\", Duration::from_secs(15)));
    let report = request_report(&mut term);
    assert_eq!(report.line, line);
    assert_eq!(report.cursor, line.encode_utf16().count());
    drop(term);
    remove_dir_when_released(&dir);
}
