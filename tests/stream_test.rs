use shell_panel::shell::command_state::CommandState;
use shell_panel::shell::stream::{ingest_pty_chunk, MAX_MESSAGE_BYTES};
use shell_panel::vt::emulator::HeadlessTerminal;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn test_message_split_across_chunks_is_reassembled_and_hidden() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::new(TOKEN);
    let mut residual = Vec::new();

    let first = ingest_pty_chunk(
        format!("hi\x1b]6973;{TOKEN};RS;C:/p").as_bytes(),
        &mut term,
        &mut state,
        &mut residual,
    );
    let second = ingest_pty_chunk(b"roj\x07there", &mut term, &mut state, &mut residual);

    assert_eq!(first, b"hi");
    assert_eq!(second, b"there");
    assert_eq!(state.cwd, "C:/proj");
    assert!(state.reading_line);
    assert!(residual.is_empty());
    assert_eq!(term.cursor_position(), (7, 0)); // "hithere"
}

#[test]
fn test_split_prefix_and_st_terminator() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::new(TOKEN);
    let mut residual = Vec::new();

    let first = ingest_pty_chunk(b"a\x1b]69", &mut term, &mut state, &mut residual);
    let second = ingest_pty_chunk(
        format!("73;{TOKEN};RE\x1b\\b").as_bytes(),
        &mut term,
        &mut state,
        &mut residual,
    );

    assert_eq!(first, b"a");
    assert_eq!(second, b"b");
    assert!(!state.reading_line);
}

#[test]
fn test_split_st_terminator_after_a_readline_start() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::new(TOKEN);
    let mut residual = Vec::new();
    ingest_pty_chunk(
        format!("\x1b]6973;{TOKEN};RS;C:/p\x07").as_bytes(),
        &mut term,
        &mut state,
        &mut residual,
    );
    assert!(state.reading_line);

    let first = ingest_pty_chunk(
        format!("a\x1b]6973;{TOKEN};RE\x1b").as_bytes(),
        &mut term,
        &mut state,
        &mut residual,
    );
    let second = ingest_pty_chunk(b"\\b", &mut term, &mut state, &mut residual);

    assert_eq!(first, b"a");
    assert_eq!(second, b"b");
    assert!(!state.reading_line);
}

#[test]
fn test_unterminated_message_is_not_buffered_forever() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::new(TOKEN);
    let mut residual = Vec::new();

    let mut chunk = format!("\x1b]6973;{TOKEN};CMP;").into_bytes();
    chunk.extend(std::iter::repeat_n(b'a', MAX_MESSAGE_BYTES + 1));
    let out = ingest_pty_chunk(&chunk, &mut term, &mut state, &mut residual);

    assert!(residual.is_empty());
    assert_eq!(out.len(), chunk.len());
}

#[test]
fn test_control_byte_in_unterminated_message_is_rejected() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::new(TOKEN);
    let mut residual = Vec::new();

    let mut chunk = format!("\x1b]6973;{TOKEN};RS;C:\n").into_bytes();
    chunk.extend_from_slice(b"hello");
    chunk.push(0x07);
    let out = ingest_pty_chunk(&chunk, &mut term, &mut state, &mut residual);

    assert!(
        out.windows(5).any(|w| w == b"hello"),
        "output must contain the literal bytes: {:?}",
        String::from_utf8_lossy(&out)
    );
    assert!(
        !state.reading_line,
        "a raw control byte in the payload must not be parsed as an RS message"
    );
    assert!(residual.is_empty());
}

#[test]
fn test_valid_message_after_a_rejected_one_is_still_parsed() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::new(TOKEN);
    let mut residual = Vec::new();

    let mut chunk = format!("\x1b]6973;{TOKEN};RS;C:\nhello\x07").into_bytes();
    chunk.extend_from_slice(format!("\x1b]6973;{TOKEN};RS;C:/proj\x07").as_bytes());
    let out = ingest_pty_chunk(&chunk, &mut term, &mut state, &mut residual);

    assert!(out.windows(5).any(|w| w == b"hello"));
    assert!(state.reading_line);
    assert_eq!(state.cwd, "C:/proj");
    assert!(residual.is_empty());
}

#[test]
fn test_payload_split_across_chunks_without_control_bytes_reassembles() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::new(TOKEN);
    let mut residual = Vec::new();

    let first = ingest_pty_chunk(
        format!("x\x1b]6973;{TOKEN};RS;C:/pro").as_bytes(),
        &mut term,
        &mut state,
        &mut residual,
    );
    let second = ingest_pty_chunk(b"ject\x07y", &mut term, &mut state, &mut residual);

    assert_eq!(first, b"x");
    assert_eq!(second, b"y");
    assert_eq!(state.cwd, "C:/project");
    assert!(state.reading_line);
    assert!(residual.is_empty());
}

#[test]
fn test_forged_marker_in_output_has_no_effect() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::new(TOKEN);
    let mut residual = Vec::new();

    // What `Get-Content forged.txt` prints: the old wire format, no token.
    let out = ingest_pty_chunk(
        b"before\x1b]6973;RS;C:/evil\x07\x1b]6973;CMP;{\"line\":\"x\",\"cursor\":1}\x07after",
        &mut term,
        &mut state,
        &mut residual,
    );

    assert!(!state.reading_line, "a forged RS must not start a line");
    assert_eq!(state.cwd, "");
    assert_eq!(state.report, None);
    assert_eq!(out, b"beforeafter");
    assert!(residual.is_empty());
}
