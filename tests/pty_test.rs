mod common;

use std::io::Write;
use std::time::Duration;

use shell_panel::pty::conpty::{watch_exit, ConPtySession, SpawnOptions};
use shell_panel::pty::shell::{detect_shell, detect_shell_in, is_supported_shell, ShellType};

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
fn test_is_supported_shell() {
    for name in ["pwsh", "PWSH.exe", "powershell", "PowerShell.EXE"] {
        assert!(is_supported_shell(name), "{name}");
    }
    for name in ["cmd", "bash", "C:\\tools\\pwsh.exe", ""] {
        assert!(!is_supported_shell(name), "{name}");
    }
}

#[test]
fn test_detect_shell_prefers_pwsh_on_path() {
    let with = common::TempDir::new("pwsh");
    let without = common::TempDir::new("nopwsh");
    std::fs::write(with.join("pwsh.exe"), b"").unwrap();

    let path = std::env::join_paths([&*without, &*with]).unwrap();
    assert_eq!(detect_shell_in(None, Some(&path)), ShellType::Pwsh);
    let path = std::env::join_paths([&*without]).unwrap();
    assert_eq!(detect_shell_in(None, Some(&path)), ShellType::Powershell);
    assert_eq!(detect_shell_in(None, None), ShellType::Powershell);
    assert_eq!(
        detect_shell_in(Some("powershell"), Some(&path)),
        ShellType::Powershell
    );
}

#[tokio::test]
async fn test_watch_exit_reports_code_although_pty_output_stays_open() {
    let ConPtySession { pair, child, .. } = ConPtySession::spawn(
        detect_shell(None),
        80,
        24,
        SpawnOptions { no_profile: true },
    )
    .unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let exit_rx = watch_exit(child);

    // Input typed before PSReadLine is up stays in the console input buffer.
    tokio::time::sleep(Duration::from_secs(3)).await;
    // The comment is PSReadLine's sensitive-line rule (2.2+): the line is not saved to history.
    writer.write_all(b"exit 3 # apikey\r").unwrap();
    writer.flush().unwrap();

    let code = tokio::time::timeout(Duration::from_secs(30), exit_rx)
        .await
        .expect("shell did not report exit")
        .expect("watcher thread dropped the sender");
    assert_eq!(code, 3);
    drop(pair);
}
