mod common;

use std::time::Duration;

use common::Terminal;

const START: Duration = Duration::from_secs(40);
const STEP: Duration = Duration::from_secs(15);

/// The line printed right above `marker` (the answer to the command before it).
fn answer_before(term: &Terminal, marker: &str) -> Option<String> {
    let screen = term.screen();
    let lines: Vec<&str> = screen.lines().map(str::trim_end).collect();
    let at = lines.iter().rposition(|l| *l == marker)?;
    at.checked_sub(1).map(|i| lines[i].to_string())
}

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
fn test_characters_outside_the_bmp_reach_powershell_once() {
    let dir = std::env::temp_dir().join(format!("sp_e2e_astral_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut term = Terminal::shell_panel(&dir);
    assert!(
        term.wait_for_text("PS ", START),
        "screen: {}",
        term.screen()
    );
    term.quiet_session();

    // 😀 and 🐛 are two UTF-16 units each; 日 is one. Expected length: 1+2+1+2+1 = 7.
    term.send("'X😀日🐛Y'.Length; 'LEN' + 'ONE'\r".as_bytes());
    assert!(
        term.wait_for_text("LENONE", STEP),
        "screen: {}",
        term.screen()
    );
    assert_eq!(
        answer_before(&term, "LENONE").as_deref(),
        Some("7"),
        "screen: {}",
        term.screen()
    );
    drop(term);
    remove_dir_when_released(&dir);
}
