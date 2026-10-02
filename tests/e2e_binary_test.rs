mod common;

use std::time::Duration;

use common::{TempDir, Terminal};

const START: Duration = Duration::from_secs(40);
const STEP: Duration = Duration::from_secs(15);

fn temp_dir(tag: &str) -> TempDir {
    TempDir::new(&format!("e2e_{tag}"))
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
    term.quiet_session();
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
    term.quiet_session();
    term.warm_completion();

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
        term.wait_until(STEP, |t| !t.screen().lines().any(|l| l.contains("> git"))),
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
    term.quiet_session();
    term.warm_completion();

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
    term.quiet_session();
    term.warm_completion();

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
    term.quiet_session();

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

#[test]
fn test_cursor_is_visible_after_exit() {
    // I7: the raw-mode guard must show the cursor again on the way out.
    let dir = temp_dir("cursor");
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
    term.drain(Duration::from_millis(500), Duration::from_secs(5));
    let last_show_or_hide = term
        .raw
        .windows(6)
        .rposition(|w| w == b"\x1b[?25h" || w == b"\x1b[?25l")
        .map(|i| &term.raw[i..i + 6]);
    assert_eq!(last_show_or_hide, Some(&b"\x1b[?25h"[..]));
}

#[test]
fn test_tab_falls_back_to_powershell_when_no_report_arrives() {
    // I9: without the chord handler no report comes; after 3 s the Tab goes to PowerShell.
    let dir = temp_dir("fallback");
    std::fs::write(dir.join("zz_unique_file.txt"), "x").unwrap();
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();
    term.send(b"Remove-PSReadLineKeyHandler -Chord 'Ctrl+Alt+Shift+F12'; 'UNBO' + 'UND'\r");
    assert!(
        term.wait_for_text("UNBOUND", STEP),
        "screen: {}",
        term.screen()
    );
    term.send(b"echo zz_uni");
    assert!(term.wait_for_text("echo zz_uni", STEP));
    term.send(b"\t");
    assert!(
        term.wait_for_text("zz_unique_file.txt", Duration::from_secs(20)),
        "screen: {}",
        term.screen()
    );
    term.send(b"\x1b");
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_key_typed_before_the_report_keeps_the_tab() {
    // I10: Tab then Q in one write; the withheld Tab is replayed before the Q.
    let dir = temp_dir("withheld");
    std::fs::write(dir.join("zz_unique_file.txt"), "x").unwrap();
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();
    term.send(b"echo zz_uni");
    assert!(term.wait_for_text("echo zz_uni", STEP));
    term.send(b"\tQ");
    assert!(
        term.wait_for_text("zz_unique_file.txtQ", Duration::from_secs(20)),
        "screen: {}",
        term.screen()
    );
    term.send(b"\x1b");
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_reserved_chord_never_reaches_a_running_program() {
    // I11: while a program reads keys, Tab is a plain Tab, never Ctrl+Alt+Shift+F12.
    let dir = temp_dir("readkey");
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();
    term.send(
        b"'READY' + 'KEY'; $k = [Console]::ReadKey($true); 'GOT-' + $k.Key + '-' + $k.Modifiers\r",
    );
    assert!(
        term.wait_for_text("READYKEY", STEP),
        "screen: {}",
        term.screen()
    );
    term.send(b"\t");
    // The answer is a screen line that is only the answer: the echoed command line also starts
    // with `GOT-` when it wraps, but it carries quotes and `$`.
    fn answer_line(screen: &str) -> Option<String> {
        screen
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("GOT-") && !l.contains('\'') && !l.contains('$'))
            .map(str::to_string)
    }
    assert!(
        term.wait_until(STEP, |t| answer_line(&t.screen()).is_some()),
        "screen: {}",
        term.screen()
    );
    let got = answer_line(&term.screen()).unwrap();
    assert_eq!(got, "GOT-Tab-None", "screen: {}", term.screen());
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_session_marks_itself_for_nested_start_detection() {
    // I14: the child shell sees SHELL_PANEL_SESSION=1, which --check and nested starts rely on.
    let dir = temp_dir("sessionenv");
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();
    term.send(b"'SPS=' + $env:SHELL_PANEL_SESSION\r");
    assert!(
        term.wait_until(STEP, |t| t.screen().lines().any(|l| l.trim() == "SPS=1")),
        "screen: {}",
        term.screen()
    );
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_console_resize_reaches_the_shell() {
    let dir = temp_dir("resize");
    let mut term = Terminal::shell_panel(&dir);
    let name = dir.file_name().unwrap().to_str().unwrap().to_string();
    assert!(
        term.wait_for_text(&name, START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();
    term.resize(100, 40);
    // Concatenation: the echoed command cannot satisfy the wait. Retried, since the resize
    // event travels through shell-panel and the console before the shell sees the new size.
    let ask = b"'SZ=' + $Host.UI.RawUI.WindowSize.Width + 'x' + $Host.UI.RawUI.WindowSize.Height\r";
    let mut seen = false;
    for _ in 0..5 {
        term.send(ask);
        if term.wait_until(Duration::from_secs(5), |t| {
            t.screen().lines().any(|l| l.trim() == "SZ=100x40")
        }) {
            seen = true;
            break;
        }
    }
    assert!(seen, "screen: {}", term.screen());
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}
