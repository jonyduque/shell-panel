use std::io::Write;
use std::path::Path;
use std::time::Duration;

use shell_panel::core::app::get_shell_integration_path;
use shell_panel::pty::conpty::{watch_exit, ConPtySession};
use shell_panel::pty::shell::{detect_shell, ShellType};

#[test]
fn test_shell_type_executable_name() {
    assert_eq!(ShellType::Pwsh.executable_name(), "pwsh.exe");
    assert_eq!(ShellType::Powershell.executable_name(), "powershell.exe");
}

#[test]
fn test_detect_shell_override_powershell() {
    assert_eq!(detect_shell(Some("powershell")), ShellType::Powershell);
    assert_eq!(detect_shell(Some("PowerShell")), ShellType::Powershell);
    assert_eq!(detect_shell(Some("powershell.exe")), ShellType::Powershell);
    assert_eq!(detect_shell(Some("PowerShell.EXE")), ShellType::Powershell);
}

#[test]
fn test_detect_shell_override_pwsh_or_other() {
    assert_eq!(detect_shell(Some("pwsh")), ShellType::Pwsh);
    assert_eq!(detect_shell(Some("pwsh.exe")), ShellType::Pwsh);
    assert_eq!(detect_shell(Some("anything_else")), ShellType::Pwsh);
}

#[test]
fn test_detect_shell_auto() {
    let shell = detect_shell(None);
    assert!(shell == ShellType::Pwsh || shell == ShellType::Powershell);
}

#[test]
fn test_conpty_session_spawn_and_resize() {
    let script_path = Path::new("assets/shellIntegration.ps1");
    let shell = detect_shell(None);
    let mut session =
        ConPtySession::spawn(shell, 80, 24, script_path).expect("Falha ao criar sessão ConPTY");
    assert!(session.resize(120, 40).is_ok());
    let _ = session.kill();
}

#[tokio::test]
async fn test_watch_exit_reports_code_although_pty_output_stays_open() {
    let script = get_shell_integration_path().unwrap();
    let ConPtySession { pair, child } =
        ConPtySession::spawn(detect_shell(None), 80, 24, &script).unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let exit_rx = watch_exit(child);

    // Input typed before PSReadLine is up stays in the console input buffer.
    tokio::time::sleep(Duration::from_secs(3)).await;
    writer.write_all(b"exit 3\r").unwrap();
    writer.flush().unwrap();

    let code = tokio::time::timeout(Duration::from_secs(30), exit_rx)
        .await
        .expect("shell did not report exit")
        .expect("watcher thread dropped the sender");
    assert_eq!(code, 3);
    drop(pair);
}
