use std::io::{Read, Write};
use std::time::{Duration, Instant};

use shell_panel::core::app::{default_json_spec_provider, get_shell_integration_path};
use shell_panel::engine::lexer::lex_command_line;
use shell_panel::engine::provider::CompletionProvider;
use shell_panel::engine::providers::files::FileProvider;
use shell_panel::pty::conpty::ConPtySession;
use shell_panel::pty::shell::detect_shell;
use shell_panel::shell::command_state::CommandState;
use shell_panel::shell::osc::parse_osc_sequence;
use shell_panel::io::filter::sanitize_output_stream;
use shell_panel::vt::emulator::HeadlessTerminal;

fn scan_osc_test(
    data: &[u8],
    term: &mut HeadlessTerminal,
    command_state: &mut CommandState,
    residual: &mut Vec<u8>,
) -> Vec<u8> {
    let mut clean_output = Vec::with_capacity(data.len());
    let mut i = 0;
    let mut last = 0;

    while i < data.len() {
        if data[i..].starts_with(b"\x1b]6973;") {
            if i > last {
                let slice = &data[last..i];
                term.process(slice);
                clean_output.extend_from_slice(slice);
            }
            let payload_start = i + 2;
            let mut j = i + 7;
            let mut term_len = 0;

            while j < data.len() {
                if data[j] == 0x07 {
                    term_len = 1;
                    break;
                }
                if data[j..].starts_with(b"\x1b\\") {
                    term_len = 2;
                    break;
                }
                j += 1;
            }

            if term_len > 0 {
                let payload = &data[payload_start..j];
                if let Ok(payload_str) = std::str::from_utf8(payload) {
                    if let Some(event) = parse_osc_sequence(payload_str) {
                        let (cx, cy) = term.cursor_position();
                        command_state.handle_osc(event, cy, cx);
                    }
                }
                i = j + term_len;
                last = i;
            } else {
                residual.extend_from_slice(&data[i..]);
                return clean_output;
            }
        } else {
            i += 1;
        }
    }

    if last < data.len() {
        let tail = &data[last..];
        term.process(tail);
        clean_output.extend_from_slice(tail);
    }

    clean_output
}

#[tokio::test]
async fn test_debug_git_c_interactive() {
    let script_path = get_shell_integration_path().unwrap();
    let shell = detect_shell(None);
    let session = ConPtySession::spawn(shell, 120, 30, &script_path).unwrap();

    let mut reader = session.pair.master.try_clone_reader().unwrap();
    let mut writer = session.pair.master.take_writer().unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1024);
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut term = HeadlessTerminal::new(120, 30);
    let mut command_state = CommandState::default();
    let mut residual = Vec::new();

    // Wait until prompt appears (in_prompt is false and prompt_line is Some)
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(chunk) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            if let Some(c) = chunk {
                let mut data_to_process = std::mem::take(&mut residual);
                data_to_process.extend_from_slice(&c);
                let sanitized = sanitize_output_stream(&data_to_process);
                scan_osc_test(&sanitized, &mut term, &mut command_state, &mut residual);
                if !command_state.in_prompt && command_state.prompt_line.is_some() {
                    break;
                }
            }
        }
    }

    println!("Initial prompt state: in_prompt={}, prompt_line={:?}, prompt_end_x={:?}, cwd={}",
        command_state.in_prompt, command_state.prompt_line, command_state.prompt_end_x, command_state.cwd);

    // Check prompt function
    writer.write_all(b"(Get-Command prompt).Definition\r\n").unwrap();
    writer.flush().unwrap();

    let start_def = Instant::now();
    let mut def_output = String::new();
    while start_def.elapsed() < Duration::from_secs(2) {
        if let Ok(chunk) = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            if let Some(c) = chunk {
                def_output.push_str(&String::from_utf8_lossy(&c));
            }
        }
    }
    println!("PROMPT DEFINITION:\n{}", def_output);

    // Now send "git c" to PTY
    writer.write_all(b"git c").unwrap();
    writer.flush().unwrap();

    // Read PTY echo
    let start2 = Instant::now();
    while start2.elapsed() < Duration::from_secs(2) {
        if let Ok(chunk) = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            if let Some(c) = chunk {
                let mut data_to_process = std::mem::take(&mut residual);
                data_to_process.extend_from_slice(&c);
                let sanitized = sanitize_output_stream(&data_to_process);
                scan_osc_test(&sanitized, &mut term, &mut command_state, &mut residual);
            }
        }
    }

    let p_row = command_state.prompt_line.unwrap_or(0);
    let p_col = command_state.prompt_end_x.unwrap_or(0);
    let extracted = term.extract_command_text(p_row, p_col);
    let (cx, cy) = term.cursor_position();
    println!("After typing 'git c': cursor=({}, {}), p_row={}, p_col={}, extracted='{}'",
        cx, cy, p_row, p_col, extracted);

    let tokens = lex_command_line(&extracted);
    println!("Tokens: {:?}", tokens);

    let root_cmd = tokens.first().map(|t| t.text.as_str()).unwrap_or("");
    println!("root_cmd: '{}'", root_cmd);

    let json_spec = default_json_spec_provider();
    println!("json_spec can_handle('{}'): {}", root_cmd, json_spec.can_handle(root_cmd));
    let json_sugs = json_spec.complete(&extracted, &command_state.cwd).await;
    println!("json_sugs ({}): {:?}", json_sugs.len(), json_sugs);

    let file_prov = FileProvider::new();
    let file_sugs = file_prov.complete(&extracted, &command_state.cwd).await;
    println!("file_sugs ({}): {:?}", file_sugs.len(), file_sugs);

    writer.write_all(b"exit\r\n").unwrap();
    writer.flush().unwrap();
}
