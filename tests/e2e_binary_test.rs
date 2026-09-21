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

    // Esc: PSReadLine reverts the line. Wait for the revert, so that the next wait cannot
    // match the stale `git status`.
    term.send(b"\x1b");
    assert!(
        term.wait_until(STEP, |t| !t.screen().contains("git")),
        "line not reverted: {}",
        term.screen()
    );
    term.send(b"git ");
    assert!(
        term.wait_for_text("> git", STEP),
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

#[test]
fn test_completion_uses_the_real_line_and_the_real_session() {
    let dir = temp_dir("session");
    std::fs::write(dir.join("zz_unique_file.txt"), "x").unwrap();
    std::fs::create_dir_all(dir.join("Zq Folder")).unwrap();
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START));

    // `>` inside the command used to cut the scraped line.
    term.send(b"echo a > zz_uni");
    assert!(term.wait_for_text("zz_uni", STEP));
    term.send(b"\t");
    assert!(
        term.wait_for_text("zz_unique_file.txt", STEP),
        "screen: {}",
        term.screen()
    );
    term.send(b"\x1b");

    // A variable that exists only in this session.
    term.send(b"$sp_e2e_var_zz = 1\r");
    term.send(b"$sp_e2e_v");
    assert!(term.wait_for_text("$sp_e2e_v", STEP));
    term.send(b"\t");
    assert!(
        term.wait_for_text("$sp_e2e_var_zz", STEP),
        "screen: {}",
        term.screen()
    );
    term.send(b"\x1b");

    // Names with spaces arrive quoted from PowerShell.
    // (An unusual name, so that zoxide history cannot add a second match.)
    term.send(b"cd Zq");
    assert!(term.wait_for_text("cd Zq", STEP));
    term.send(b"\t");
    assert!(
        term.wait_for_text(r"'.\Zq Folder'", STEP),
        "screen: {}",
        term.screen()
    );
    term.send(b"\x1b");

    // Completion in the middle of the line replaces the whole token: PowerShell's replacement
    // range reaches past the cursor, so the trailing `e` is deleted instead of left behind.
    term.send(b"Get-ChildIte");
    assert!(term.wait_for_text("Get-ChildIte", STEP));
    term.send(b"\x1b[D"); // cursor before the final `e`
    term.send(b"\t");
    assert!(
        term.wait_until(STEP, |t| t.screen().contains("Get-ChildItem")
            && !t.screen().contains("Get-ChildItem e")),
        "screen: {}",
        term.screen()
    );

    term.send(b"\x1b");
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_enter_accepts_the_highlighted_suggestion_without_running_the_line() {
    let dir = temp_dir("enter");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START));

    term.send(b"git ");
    assert!(
        term.wait_for_text("> git", STEP),
        "screen: {}",
        term.screen()
    );
    term.send(b"\t");
    assert!(
        term.wait_for_text("Record changes to the repository", STEP),
        "screen: {}",
        term.screen()
    );

    term.send(b"\r");
    assert!(
        term.wait_until(STEP, |t| !t
            .screen()
            .contains("Record changes to the repository")),
        "dropdown still open: {}",
        term.screen()
    );

    // The Enter was consumed, so the accepted suggestion is still an editable line: what is typed
    // next lands on it. Had the line run, `git` would be above a fresh prompt instead.
    term.send(b"zzmark");
    assert!(
        term.wait_until(STEP, |t| t
            .screen()
            .lines()
            .any(|l| l.contains("git") && l.contains("zzmark"))),
        "line was executed: {}",
        term.screen()
    );

    term.send(b"\x1b");
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_tab_outside_psreadline_is_a_plain_tab() {
    let dir = temp_dir("readhost");
    let mut term = Terminal::shell_panel(&dir);
    assert!(term.wait_for_text("PS ", START));

    term.send(b"$v = Read-Host 'name'\r");
    assert!(term.wait_for_text("name:", STEP));
    term.send(b"a\tb\r");
    // Read-Host may keep or drop the Tab, but the reserved chord (`[24;8~`) must never reach it.
    // (The answer is built by concatenation so the echoed command line cannot match.)
    term.send(b"if ($v -match '^a\\s*b$') { 'TAB-' + 'CLEAN' } else { 'TAB-' + 'DIRTY' }\r");
    assert!(
        term.wait_until(STEP, |t| t.screen().contains("TAB-CLEAN")
            || t.screen().contains("TAB-DIRTY")),
        "screen: {}",
        term.screen()
    );
    assert!(
        term.screen().contains("TAB-CLEAN"),
        "screen: {}",
        term.screen()
    );

    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}
