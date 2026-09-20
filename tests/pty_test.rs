use shell_panel::pty::conpty::ConPtySession;
use shell_panel::pty::shell::{detect_shell, ShellType};
use std::path::Path;

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
    let mut session = ConPtySession::spawn(shell, 80, 24, script_path)
        .expect("Falha ao criar sessão ConPTY");
    assert!(session.resize(120, 40).is_ok());
    let _ = session.kill();
}
