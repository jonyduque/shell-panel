use shell_panel::vt::cpr::has_cpr_query;
use shell_panel::vt::emulator::{preprocess_vt_bytes, HeadlessTerminal};

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
fn test_command_text_extraction() {
    let mut vt = HeadlessTerminal::new(80, 24);
    // Print prompt "PS > " (end_x = 5) followed by command
    vt.process(b"PS > git status");
    let cmd = vt.extract_command_text(0, 5);
    assert_eq!(cmd, "git status");
}

#[test]
fn test_command_text_preserves_trailing_spaces() {
    let mut vt = HeadlessTerminal::new(80, 24);
    vt.process(b"PS > cargo ");
    let cmd = vt.extract_command_text(0, 5);
    assert_eq!(cmd, "cargo ");
}

#[test]
fn test_ghost_text_filtering_dim_and_italic() {
    let mut vt = HeadlessTerminal::new(80, 24);
    // "git " typed, followed by italic/dim prediction "commit"
    vt.process(b"PS > git \x1b[3mcommit\x1b[0m");
    let cmd = vt.extract_command_text(0, 5);
    assert_eq!(cmd, "git ");

    let mut vt2 = HeadlessTerminal::new(80, 24);
    vt2.process(b"PS > docker \x1b[2mrun -d nginx\x1b[0m");
    let cmd2 = vt2.extract_command_text(0, 5);
    assert_eq!(cmd2, "docker ");
}

#[test]
fn test_ghost_text_filtering_psreadline_prediction_color() {
    let mut vt = HeadlessTerminal::new(80, 24);
    // PSReadLine bright black prediction (90m)
    vt.process(b"PS > npm \x1b[90minstall lodash\x1b[0m");
    let cmd = vt.extract_command_text(0, 5);
    assert_eq!(cmd, "npm ");
}

#[test]
fn test_ansi_preprocessor_does_not_corrupt_cursor_cup() {
    // \x1b[2;10H must remain \x1b[2;10H, moving to row 1 (0-indexed), col 9
    let mut vt = HeadlessTerminal::new(80, 24);
    vt.process(b"\x1b[2;10H");
    assert_eq!(vt.cursor_position(), (9, 1));
}

#[test]
fn test_ansi_preprocessor_does_not_corrupt_rgb_or_256_colors() {
    // RGB 24-bit color: \x1b[38;2;100;150;200m
    let rgb_seq = b"\x1b[38;2;100;150;200mHello\x1b[0m";
    let processed = preprocess_vt_bytes(rgb_seq);
    assert_eq!(processed.as_ref(), rgb_seq);

    // 256 color palette index 2: \x1b[38;5;2m
    let pal_seq = b"\x1b[38;5;2mGreen\x1b[0m";
    let processed_pal = preprocess_vt_bytes(pal_seq);
    assert_eq!(processed_pal.as_ref(), pal_seq);
}

#[test]
fn test_wide_characters_cjk_and_emojis() {
    let mut vt = HeadlessTerminal::new(80, 24);
    vt.process("PS > echo 你好 🚀".as_bytes());
    let cmd = vt.extract_command_text(0, 5);
    assert_eq!(cmd, "echo 你好 🚀");
}

#[test]
fn test_multiline_command_extraction() {
    let mut vt = HeadlessTerminal::new(20, 24);
    // Wrap to line 2
    vt.process(b"PS > very_long_comma\r\nnd_continued");
    let cmd = vt.extract_command_text(0, 5);
    assert_eq!(cmd, "very_long_command_continued");
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
