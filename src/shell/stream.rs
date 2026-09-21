use crate::io::filter::sanitize_output_stream;
use crate::shell::command_state::CommandState;
use crate::shell::osc::parse_osc_sequence;
use crate::vt::emulator::HeadlessTerminal;

const OSC_PREFIX: &[u8] = b"\x1b]6973;";

/// Largest unterminated message kept waiting for its terminator. A completion report is capped
/// at 100 matches (tens of kilobytes); anything larger is passed through as ordinary output.
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

/// Processes one chunk of PTY output: strips input-protocol negotiation, applies and removes
/// shell-panel's OSC 6973 messages, feeds the rest to the headless terminal and returns it for
/// echoing to the host terminal. A message split across chunks waits in `residual`.
pub fn ingest_pty_chunk(
    chunk: &[u8],
    term: &mut HeadlessTerminal,
    command_state: &mut CommandState,
    residual: &mut Vec<u8>,
) -> Vec<u8> {
    let mut data = std::mem::take(residual);
    data.extend_from_slice(chunk);
    let sanitized = sanitize_output_stream(&data);
    scan_messages(&sanitized, term, command_state, residual)
}

fn scan_messages(
    data: &[u8],
    term: &mut HeadlessTerminal,
    command_state: &mut CommandState,
    residual: &mut Vec<u8>,
) -> Vec<u8> {
    let mut clean_output = Vec::with_capacity(data.len());
    let mut i = 0;
    let mut last = 0;

    while i < data.len() {
        if !data[i..].starts_with(OSC_PREFIX) {
            i += 1;
            continue;
        }

        if i > last {
            term.process(&data[last..i]);
            clean_output.extend_from_slice(&data[last..i]);
        }

        let body = &data[i + OSC_PREFIX.len()..];
        let mut terminator = None;
        let mut invalid = false;
        for (offset, &b) in body.iter().enumerate() {
            if b == 0x07 {
                terminator = Some((offset, 1));
                break;
            }
            if b == 0x1b {
                if body[offset..].starts_with(b"\x1b\\") {
                    terminator = Some((offset, 2));
                    break;
                }
                if offset == body.len() - 1 {
                    // Could be the start of an ST terminator split across the chunk boundary.
                    break;
                }
                invalid = true;
                break;
            }
            if b < 0x20 {
                // The shell escapes every control character in a payload (`__SP-Escape`), so a
                // raw one here proves this was never one of our messages.
                invalid = true;
                break;
            }
        }

        if invalid {
            // Not one of our messages: emit the prefix literally and resume right after it.
            term.process(OSC_PREFIX);
            clean_output.extend_from_slice(OSC_PREFIX);
            i += OSC_PREFIX.len();
            last = i;
            continue;
        }

        let Some((offset, terminator_len)) = terminator else {
            if data.len() - i > MAX_MESSAGE_BYTES {
                // Never terminated: stop waiting and treat it as ordinary output.
                term.process(&data[i..]);
                clean_output.extend_from_slice(&data[i..]);
            } else {
                // Unterminated message at the chunk boundary: wait for the next chunk.
                residual.extend_from_slice(&data[i..]);
            }
            return clean_output;
        };

        let payload_end = i + OSC_PREFIX.len() + offset;
        if let Ok(payload) = std::str::from_utf8(&data[i + 2..payload_end]) {
            if let Some(event) = parse_osc_sequence(payload) {
                command_state.handle_osc(event);
            }
        }
        i = payload_end + terminator_len;
        last = i;
    }

    // Keep back a tail that could be the start of a message prefix.
    let tail = &data[last..];
    let held = (1..=OSC_PREFIX.len().min(tail.len()))
        .rev()
        .find(|&len| tail.ends_with(&OSC_PREFIX[..len]))
        .unwrap_or(0);
    let safe = &tail[..tail.len() - held];
    term.process(safe);
    clean_output.extend_from_slice(safe);
    residual.extend_from_slice(&tail[tail.len() - held..]);

    clean_output
}
