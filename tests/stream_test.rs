use shell_panel::shell::command_state::CommandState;
use shell_panel::shell::stream::{ingest_pty_chunk, MAX_MESSAGE_BYTES};
use shell_panel::vt::emulator::HeadlessTerminal;

#[test]
fn test_message_split_across_chunks_is_reassembled_and_hidden() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::default();
    let mut residual = Vec::new();

    let first = ingest_pty_chunk(b"hi\x1b]6973;RS;C:/p", &mut term, &mut state, &mut residual);
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
    let mut state = CommandState::default();
    let mut residual = Vec::new();

    let first = ingest_pty_chunk(b"a\x1b]69", &mut term, &mut state, &mut residual);
    let second = ingest_pty_chunk(b"73;RE\x1b\\b", &mut term, &mut state, &mut residual);

    assert_eq!(first, b"a");
    assert_eq!(second, b"b");
    assert!(!state.reading_line);
}

#[test]
fn test_unterminated_message_is_not_buffered_forever() {
    let mut term = HeadlessTerminal::new(80, 24);
    let mut state = CommandState::default();
    let mut residual = Vec::new();

    let mut chunk = b"\x1b]6973;CMP;".to_vec();
    chunk.extend(std::iter::repeat(b'a').take(MAX_MESSAGE_BYTES + 1));
    let out = ingest_pty_chunk(&chunk, &mut term, &mut state, &mut residual);

    assert!(residual.is_empty());
    assert_eq!(out.len(), chunk.len());
}
