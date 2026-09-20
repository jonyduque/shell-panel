use shell_panel::vt::cpr::has_cpr_query;
use shell_panel::vt::emulator::{CellExt, HeadlessTerminal};

#[test]
fn test_vt_headless_basic_processing_and_cursor() {
    let mut vt = HeadlessTerminal::new(80, 24);
    assert_eq!(vt.cols, 80);
    assert_eq!(vt.rows, 24);
    assert_eq!(vt.cursor_position(), (0, 0));

    vt.process(b"Hello from shell");
    // "Hello from shell" is 16 chars: col 16, row 0
    assert_eq!(vt.cursor_position(), (16, 0));

    // Move to next line
    vt.process(b"\r\nNext line");
    assert_eq!(vt.cursor_position(), (9, 1));

    // Resize terminal
    vt.resize(120, 40);
    assert_eq!(vt.cols, 120);
    assert_eq!(vt.rows, 40);
}

#[test]
fn test_cpr_query_detection() {
    // Exact CPR queries
    assert!(has_cpr_query(b"\x1b[6n"));
    assert!(has_cpr_query(b"\x1b[?6n"));

    // Embedded CPR queries within data streams
    assert!(has_cpr_query(b"prefix data \x1b[6n suffix"));
    assert!(has_cpr_query(b"\x00\x1b[?6n\xff"));

    // Non-CPR queries or normal text
    assert!(!has_cpr_query(b"normal text"));
    assert!(!has_cpr_query(b""));
    assert!(!has_cpr_query(b"\x1b[5n"));
    assert!(!has_cpr_query(b"\x1b[6m"));
    assert!(!has_cpr_query(b"\x1b[?5n"));
}

#[test]
fn test_command_text_extraction() {
    let mut vt = HeadlessTerminal::new(80, 24);
    // PS prompt: "PS C:\Users> " (13 chars) followed by command
    vt.process(b"PS C:\\Users> cargo build --release");
    let cmd = vt.extract_command_text(0, 13);
    assert_eq!(cmd, "cargo build --release");

    // Command with multiple spaces preserved
    let mut vt2 = HeadlessTerminal::new(80, 24);
    vt2.process(b"> echo   foo   bar");
    let cmd2 = vt2.extract_command_text(0, 2);
    assert_eq!(cmd2, "echo   foo   bar");
}

#[test]
fn test_ghost_text_filtering_italic() {
    let mut vt = HeadlessTerminal::new(80, 24);
    // Prompt: "PS > " (5 chars). Command: "git ", ghost text in italic: "commit -m 'feat'"
    vt.process(b"PS > git \x1b[3mcommit -m 'feat'\x1b[0m");

    let cmd = vt.extract_command_text(0, 5);
    // Italic characters should be completely filtered out
    assert_eq!(cmd, "git");
}

#[test]
fn test_ghost_text_filtering_dim() {
    let mut vt = HeadlessTerminal::new(80, 24);
    // Prompt: "PS > " (5 chars). Command: "cargo ", ghost text in dim (SGR 2): "check --all"
    vt.process(b"PS > cargo \x1b[2mcheck --all\x1b[0m");

    let cmd = vt.extract_command_text(0, 5);
    // Dim characters should be completely filtered out
    assert_eq!(cmd, "cargo");
}

#[test]
fn test_ghost_text_filtering_prediction_color() {
    let mut vt = HeadlessTerminal::new(80, 24);
    // PSReadLine standard inline prediction color: bright black (\x1b[90m)
    vt.process(b"PS > dir \x1b[90m-Force -Recurse\x1b[0m");

    let cmd = vt.extract_command_text(0, 5);
    // Prediction color characters should be filtered out
    assert_eq!(cmd, "dir");
}

#[test]
fn test_ghost_text_filtering_entire_prediction() {
    let mut vt = HeadlessTerminal::new(80, 24);
    // Prompt only, and PSReadLine suggests entire command in dim/italic
    vt.process(b"PS > \x1b[2mGet-ChildItem\x1b[0m");

    let cmd = vt.extract_command_text(0, 5);
    assert_eq!(cmd, "");
}

#[test]
fn test_alternate_screen_buffer_detection() {
    let mut vt = HeadlessTerminal::new(80, 24);
    assert!(!vt.is_alternate_buffer());

    // Switch to alternate screen buffer (DECSET 1049)
    vt.process(b"\x1b[?1049h");
    assert!(vt.is_alternate_buffer());

    // Switch back to normal screen buffer (DECRST 1049)
    vt.process(b"\x1b[?1049l");
    assert!(!vt.is_alternate_buffer());
}

#[test]
fn test_extract_command_text_boundary_cases() {
    let mut vt = HeadlessTerminal::new(80, 24);
    vt.process(b"PS > ");

    // Cursor is at prompt end (col 5, row 0); no command typed
    assert_eq!(vt.extract_command_text(0, 5), "");

    // Prompt row is after cursor row
    assert_eq!(vt.extract_command_text(1, 0), "");

    // Prompt end is after cursor col
    assert_eq!(vt.extract_command_text(0, 10), "");
}

#[test]
fn test_screen_cell_inspection() {
    let mut vt = HeadlessTerminal::new(80, 24);
    vt.process(b"A\x1b[3mB\x1b[0m\x1b[2mC\x1b[0m");

    let screen = vt.screen();
    let cell_a = screen.cell(0, 0).expect("cell A");
    let cell_b = screen.cell(0, 1).expect("cell B");
    let cell_c = screen.cell(0, 2).expect("cell C");

    assert_eq!(cell_a.contents(), "A");
    assert!(!cell_a.italic());
    assert!(!cell_a.dim());

    assert_eq!(cell_b.contents(), "B");
    assert!(cell_b.italic());

    assert_eq!(cell_c.contents(), "C");
    assert!(cell_c.dim());
}
