mod common;

use std::time::Duration;

use common::Terminal;

const START: Duration = Duration::from_secs(40);
const STEP: Duration = Duration::from_secs(15);

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sp_e2e_{}_{}", tag, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_shell_starts_in_the_directory_shell_panel_was_started_from() {
    let dir = temp_dir("cwd");
    let mut term = Terminal::shell_panel(&dir);
    // The default prompt is `PS <path>> `.
    let name = dir.file_name().unwrap().to_str().unwrap().to_string();
    assert!(
        term.wait_for_text(&name, START),
        "screen: {}",
        term.screen()
    );
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_tab_inserts_single_match_opens_dropdown_and_exit_code_propagates() {
    let dir = temp_dir("tab");
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );

    term.send(b"git sta");
    assert!(term.wait_for_text("git sta", STEP));
    term.send(b"\t");
    assert!(
        term.wait_for_text("git status", STEP),
        "screen: {}",
        term.screen()
    );

    term.send(b"\x1b"); // Esc: PSReadLine reverts the line
    term.send(b"git ");
    // Wait until the mirror shows exactly the new line: the old `git status` also contains `> git`.
    assert!(
        term.wait_until(STEP, |t| t.screen().trim_end().ends_with("> git")),
        "screen: {}",
        term.screen()
    );
    term.send(b"\t");
    assert!(
        term.wait_for_text("Record changes to the repository", STEP),
        "screen: {}",
        term.screen()
    );

    term.send(b"\x1b"); // closes the dropdown
    assert!(
        term.wait_until(STEP, |t| !t
            .screen()
            .contains("Record changes to the repository")),
        "dropdown not cleared: {}",
        term.screen()
    );

    term.send(b"\x1b");
    term.send(b"exit 5\r");
    assert_eq!(term.wait_exit(STEP), Some(5));
}
