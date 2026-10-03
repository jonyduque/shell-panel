//! Tests never write the user's real PSReadLine history.
//!
//! `.cargo/config.toml` sets `SHELL_PANEL_TEST_HISTORY` for every process cargo starts, and the
//! integration script, which every test session runs before its first prompt, moves PSReadLine's
//! history there. Setting `APPDATA` does not work: PowerShell finds the history folder through the
//! Windows known-folder API, not the environment.
mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::Terminal;
use shell_panel::pty::conpty::{ConPtySession, SpawnOptions};
use shell_panel::pty::shell::detect_shell;

fn test_history() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-history.txt")
}

#[test]
fn test_cargo_points_the_test_history_into_the_target_directory() {
    let value = std::env::var("SHELL_PANEL_TEST_HISTORY").expect("set by .cargo/config.toml");
    assert_eq!(Path::new(&value), test_history());
}

#[test]
fn test_sessions_started_by_tests_save_their_history_under_target() {
    let ConPtySession { pair, child, token } = ConPtySession::spawn(
        detect_shell(None),
        common::COLS,
        common::ROWS,
        SpawnOptions { no_profile: true },
    )
    .unwrap();
    let mut term = Terminal::attach(pair.master, child);
    assert!(
        term.wait_for_raw(
            format!("\x1b]6973;{token};RS;").as_bytes(),
            Duration::from_secs(40)
        ),
        "no ReadLine marker, screen: {}",
        term.screen()
    );
    // Quiet first: until the path is proven, nothing typed here may reach a real history file.
    term.quiet_session();
    term.send(b"'PATH=' + (Get-PSReadLineOption).HistorySavePath + '=E' + 'ND'\r");
    assert!(
        term.wait_for_text("=END", Duration::from_secs(15)),
        "screen: {}",
        term.screen()
    );
    let screen = term.screen().replace(['\r', '\n'], "");
    let expected = format!("PATH={}=END", test_history().display());
    assert!(
        screen.contains(&expected),
        "the session saves its history elsewhere; expected `{expected}`, screen: {screen}"
    );

    // The path is proven: saving again must land in the test file.
    let marker = format!("sp_history_isolation_{}", std::process::id());
    term.send(b"Set-PSReadLineOption -HistorySaveStyle SaveIncrementally\r");
    term.send(format!("'{marker}' + '_DONE'\r").as_bytes());
    assert!(
        term.wait_for_text(&format!("{marker}_DONE"), Duration::from_secs(15)),
        "screen: {}",
        term.screen()
    );
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(Duration::from_secs(15)), Some(0));
    let history = std::fs::read_to_string(test_history()).unwrap_or_default();
    assert!(
        history.contains(&marker),
        "the line is not in {}",
        test_history().display()
    );
}
