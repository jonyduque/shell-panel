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

/// The screen row holding the line being edited.
fn edited_line(term: &Terminal) -> String {
    term.screen()
        .lines()
        .rfind(|l| l.contains("> zzf"))
        .unwrap_or("")
        .trim_end()
        .to_string()
}

/// A session where `zzf <Tab>` runs a completer that sleeps `delay_ms` and offers `zzalpha`
/// when it matches the word being completed. Ends with `zzf z` typed.
fn session_with_slow_completer(tag: &str, delay_ms: u32) -> Terminal {
    let mut term = Terminal::shell_panel(&temp_dir(tag));
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();
    let setup = format!(
        "function zzf {{ param($p) }}; Register-ArgumentCompleter -CommandName zzf -ParameterName p \
-ScriptBlock {{ param($c, $p, $w) Start-Sleep -Milliseconds {delay_ms}; 'zzalpha' | Where-Object {{ $_ -like \"$w*\" }} }}; \
'SETUP' + 'DONE'\r"
    );
    term.send(setup.as_bytes());
    assert!(
        term.wait_for_text("SETUPDONE", STEP),
        "screen: {}",
        term.screen()
    );
    term.send(b"zzf z");
    assert!(
        term.wait_until(STEP, |t| edited_line(t).ends_with("> zzf z")),
        "screen: {}",
        term.screen()
    );
    term
}

/// Lets queued keys, completers and reports finish, pumping output meanwhile.
fn settle(term: &mut Terminal, secs: u64) {
    term.wait_until(Duration::from_secs(secs), |_| false);
}

#[test]
fn test_late_report_of_a_timed_out_tab_does_not_answer_the_next_tab() {
    // Tab 1 times out at 3 s (its `\t` goes to PowerShell), Tab 2 at 3.5 s; Tab 1's report
    // lands at 4 s while Tab 2 waits.
    let mut term = session_with_slow_completer("late", 4000);
    term.send(b"\t");
    std::thread::sleep(Duration::from_millis(3500));
    term.send(b"\t");
    assert!(
        term.wait_until(Duration::from_secs(25), |t| edited_line(t)
            .contains("zzalpha")),
        "screen: {}",
        term.screen()
    );
    settle(&mut term, 12);
    let line = edited_line(&term);
    assert!(line.ends_with("> zzf zzalpha"), "line: {line:?}");
}

#[test]
fn test_report_of_the_first_tab_does_not_answer_a_tab_typed_after_more_text() {
    // Tab, `q` (Tab 1 is handed to PowerShell before it), Tab: the first report describes
    // `zzf z` and must not be applied to `zzf zzalphaq`.
    let mut term = session_with_slow_completer("typed", 2500);
    term.send(b"\t");
    std::thread::sleep(Duration::from_millis(500));
    term.send(b"q");
    std::thread::sleep(Duration::from_millis(500));
    term.send(b"\t");
    assert!(
        term.wait_until(Duration::from_secs(25), |t| edited_line(t)
            .contains("zzalphaq")),
        "screen: {}",
        term.screen()
    );
    settle(&mut term, 12);
    let line = edited_line(&term);
    assert!(line.ends_with("> zzf zzalphaq"), "line: {line:?}");
}
