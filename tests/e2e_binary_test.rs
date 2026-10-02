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

const DA1_ANSWER: &[u8] = b"\x1b[?65;4;6;18;22;52c";

/// Answers every DA1 query (`ESC[c`) the terminal sees, as a real terminal would, until `done`.
/// `answered` counts the queries answered so far, across calls: a query is answered once.
fn answer_queries_until(
    term: &mut Terminal,
    answered: &mut usize,
    timeout: Duration,
    done: impl Fn(&Terminal) -> bool,
) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        let seen = term.raw.windows(3).filter(|w| *w == b"\x1b[c").count();
        while *answered < seen {
            term.send(DA1_ANSWER);
            *answered += 1;
        }
        if done(term) {
            return true;
        }
        term.wait_until(Duration::from_millis(100), |_| false);
    }
    false
}

#[test]
fn test_terminal_answers_at_start_up_are_not_typed() {
    // The user's report: a slow start and `[?65;4;6;18;22;52c` typed at the first prompt.
    let dir = temp_dir("da1");
    let mut term = Terminal::shell_panel(&dir);
    let started = std::time::Instant::now();
    let mut answered = 0;
    assert!(
        answer_queries_until(&mut term, &mut answered, START, |t| t
            .screen()
            .contains("PS ")),
        "screen: {}",
        term.screen()
    );
    eprintln!("time to first prompt: {:?}", started.elapsed());
    // Give a late answer time to land on the line.
    answer_queries_until(&mut term, &mut answered, Duration::from_secs(3), |_| false);
    let screen = term.screen();
    assert!(
        !screen.contains("65;4;6"),
        "a terminal answer was typed: {screen}"
    );
    assert!(
        !screen.contains("[?6"),
        "a terminal answer was typed: {screen}"
    );
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

/// Starts `[Console]::ReadKey` in the session and returns once it is waiting.
fn read_one_key(term: &mut Terminal) {
    term.send(
        b"'READY' + 'KEY'; $k = [Console]::ReadKey($true); 'GOT-' + $k.Key + '-' + $k.Modifiers\r",
    );
    assert!(
        term.wait_for_text("READYKEY", STEP),
        "screen: {}",
        term.screen()
    );
}

/// The answer line printed by `read_one_key`, not a wrapped echo of the command.
fn got_line(term: &Terminal) -> Option<String> {
    term.screen()
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("GOT-") && !l.contains('\'') && !l.contains('$'))
        .map(str::to_string)
}

#[test]
fn test_program_receives_escape_sequences_intact() {
    let dir = temp_dir("arrow");
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();
    read_one_key(&mut term);
    term.send(b"\x1b[A");
    assert!(
        term.wait_until(STEP, |t| got_line(t).is_some()),
        "screen: {}",
        term.screen()
    );
    assert_eq!(
        got_line(&term).as_deref(),
        Some("GOT-UpArrow-None"),
        "screen: {}",
        term.screen()
    );
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_resize_reaches_a_running_program() {
    let dir = temp_dir("resizeprog");
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();
    term.send(b"Start-Sleep -Seconds 3; 'SZ=' + $Host.UI.RawUI.WindowSize.Width + 'x' + $Host.UI.RawUI.WindowSize.Height\r");
    term.wait_until(Duration::from_millis(800), |_| false); // the program is running
    term.resize(100, 40);
    assert!(
        term.wait_until(Duration::from_secs(15), |t| t
            .screen()
            .lines()
            .any(|l| l.trim() == "SZ=100x40")),
        "screen: {}",
        term.screen()
    );
    term.send(b"exit\r");
    assert_eq!(term.wait_exit(STEP), Some(0));
}

#[test]
fn test_console_input_mode_is_restored_after_exit() {
    // shell-panel and a probe share one console: a Windows PowerShell script runs shell-panel, then
    // prints the input mode. (cmd.exe would not do: it resets the console mode after every command,
    // which hides a mode shell-panel left behind.) The commands go through a script file because
    // quoting them on a command line does not survive.
    let dir = temp_dir("restore");
    let script = format!(
        "& \"{}\" --no-profile\r\n\
$s='[DllImport(\"kernel32.dll\")] public static extern IntPtr GetStdHandle(int n); [DllImport(\"kernel32.dll\")] public static extern bool GetConsoleMode(IntPtr h, out uint m);'\r\n\
$t=Add-Type -MemberDefinition $s -Name K -Namespace SpProbe -PassThru; $m=[uint32]0\r\n\
[void]$t::GetConsoleMode($t::GetStdHandle(-10),[ref]$m); 'VTIN=' + ($m -band 0x200)\r\n",
        env!("CARGO_BIN_EXE_shell-panel")
    );
    std::fs::write(dir.join("run.ps1"), script).unwrap();
    let mut cmd = portable_pty::CommandBuilder::new("powershell.exe");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]);
    cmd.arg(dir.join("run.ps1"));
    cmd.cwd(&*dir);
    cmd.env_remove("SHELL_PANEL_SESSION");
    let mut term = Terminal::spawn(cmd);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.send(b"exit\r");
    assert!(
        term.wait_until(Duration::from_secs(30), |t| t.screen().contains("VTIN=")),
        "screen: {}",
        term.screen()
    );
    assert!(
        term.screen().lines().any(|l| l.trim() == "VTIN=0"),
        "screen: {}",
        term.screen()
    );
}
