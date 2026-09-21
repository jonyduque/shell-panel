use std::io::{Read, Write};
use std::time::{Duration, Instant};

use shell_panel::core::app::{
    default_json_spec_provider, get_shell_integration_path, App, SHELL_INTEGRATION_SCRIPT,
};
use shell_panel::core::config::Config;
use shell_panel::engine::provider::CompletionProvider;
use shell_panel::pty::conpty::ConPtySession;
use shell_panel::pty::shell::detect_shell;

#[test]
fn test_embedded_script_and_path_resolution() {
    assert!(!SHELL_INTEGRATION_SCRIPT.is_empty());
    assert!(SHELL_INTEGRATION_SCRIPT.contains("6973;PS"));
    assert!(SHELL_INTEGRATION_SCRIPT.contains("6973;PE"));
    assert!(SHELL_INTEGRATION_SCRIPT.contains("6973;CWD;"));

    let path = get_shell_integration_path().expect("Failed to get shell integration script path");
    assert!(
        path.is_file(),
        "Script path must be a valid file: {:?}",
        path
    );
}

#[tokio::test]
async fn test_default_json_spec_provider_git_and_docker() {
    let provider = default_json_spec_provider();
    assert_eq!(provider.name(), "json_spec");
    assert!(provider.can_handle("git"));
    assert!(provider.can_handle("docker"));

    // git completions
    let git_sugs = provider.complete("git stat", "").await;
    assert!(!git_sugs.is_empty());
    assert!(git_sugs.iter().any(|s| s.name == "status"));

    // docker completions
    let docker_sugs = provider.complete("docker r", "").await;
    assert!(!docker_sugs.is_empty());
    assert!(docker_sugs.iter().any(|s| s.name == "run"));
    assert!(docker_sugs
        .iter()
        .any(|s| s.name == "restart" || s.name == "rm" || s.name == "rmi"));
}

#[test]
fn test_app_new_and_config() {
    let mut config = Config::new(10, 20);
    config.colors.selected_bg = "blue".to_string();
    config.colors.selected_fg = "white".to_string();
    let app = App::new(config, Some("pwsh".to_string()));
    assert_eq!(app.config.max_suggestions, 10);
    assert_eq!(app.config.debounce_ms, 20);
    assert_eq!(app.override_shell, Some("pwsh".to_string()));
    assert_eq!(app.theme.selected_start, "\x1b[44m\x1b[37m");
}

#[test]
fn test_e2e_pty_powershell_session() {
    let script_path = get_shell_integration_path().expect("Failed to resolve integration script");
    assert!(script_path.is_file());

    let shell = detect_shell(None);
    let mut session =
        ConPtySession::spawn(shell, 120, 30, &script_path).expect("Failed to spawn ConPty session");

    let mut reader = session
        .pair
        .master
        .try_clone_reader()
        .expect("Failed to clone PTY master reader");
    let mut writer = session
        .pair
        .master
        .take_writer()
        .expect("Failed to take PTY master writer");

    // Spawn a background thread to read from PTY master reader
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Write command: Write-Output "HELLO_SHELL_PANEL"
    writer
        .write_all(b"Write-Output \"HELLO_SHELL_PANEL\"\r\n")
        .expect("Failed to write command to PTY");
    writer.flush().expect("Failed to flush PTY writer");

    // Read PTY output until HELLO_SHELL_PANEL is detected
    let mut output = String::new();
    let start = Instant::now();
    let timeout = Duration::from_secs(15);
    let mut found = false;

    while start.elapsed() < timeout {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                output.push_str(&text);
                if output.contains("HELLO_SHELL_PANEL") {
                    found = true;
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Keep polling
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    assert!(
        found,
        "Timed out waiting for 'HELLO_SHELL_PANEL'. Received output:\n{}",
        output
    );

    // Send exit command
    writer
        .write_all(b"exit\r\n")
        .expect("Failed to write exit command to PTY");
    writer.flush().expect("Failed to flush PTY writer");

    // Assert process exits cleanly
    let exit_status = session
        .child
        .wait()
        .expect("Failed to wait on child process");
    assert!(
        exit_status.success(),
        "Expected clean exit (exit code 0), but got: {:?}",
        exit_status
    );
}
