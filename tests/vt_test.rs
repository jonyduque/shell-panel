use shell_panel::vt::cpr::has_cpr_query;
use shell_panel::vt::emulator::HeadlessTerminal;

#[test]
fn test_vt_headless_basic_processing_and_cursor() {
    let mut vt = HeadlessTerminal::new(80, 24);
    assert_eq!(vt.cols, 80);
    assert_eq!(vt.rows, 24);
    assert_eq!(vt.cursor_position(), (0, 0));

    vt.process(b"Hello world");
    assert_eq!(vt.cursor_position(), (11, 0));

    vt.process(b"\r\nNext line");
    assert_eq!(vt.cursor_position(), (9, 1));

    vt.resize(120, 40);
    assert_eq!(vt.cols, 120);
    assert_eq!(vt.rows, 40);
}

#[test]
fn test_cpr_query_detection() {
    assert!(has_cpr_query(b"\x1b[6n"));
    assert!(has_cpr_query(b"\x1b[?6n"));
    assert!(has_cpr_query(b"some prefix \x1b[6n and suffix"));
    assert!(has_cpr_query(b"prefix \x1b[?6n suffix"));
    assert!(!has_cpr_query(b"normal text without cpr"));
    assert!(!has_cpr_query(b"\x1b[5n"));
    assert!(!has_cpr_query(b""));
}

#[test]
fn test_alternate_screen_buffer_detection() {
    let mut vt = HeadlessTerminal::new(80, 24);
    assert!(!vt.is_alternate_buffer());

    // Enter alternate screen buffer (DECSET 1049)
    vt.process(b"\x1b[?1049h");
    assert!(vt.is_alternate_buffer());

    // Exit alternate screen buffer (DECRST 1049)
    vt.process(b"\x1b[?1049l");
    assert!(!vt.is_alternate_buffer());
}
