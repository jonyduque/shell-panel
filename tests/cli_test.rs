use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use shell_panel::cli::Cli;

#[test]
fn test_cli_definition_is_valid() {
    // clap's debug assertions are what panicked at startup when two args shared `-c`.
    Cli::command().debug_assert();
}

#[test]
fn test_check_keeps_short_flag_and_config_is_long_only() {
    let cli = Cli::try_parse_from(["shell-panel", "-c", "--config", "x.toml"]).unwrap();
    assert!(cli.check);
    assert_eq!(cli.config, Some(PathBuf::from("x.toml")));
}

#[test]
fn test_version_comes_from_cargo_manifest() {
    assert_eq!(
        Cli::command().get_version(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

fn run_with_session_env(args: &[&str], in_session: bool) -> std::process::Output {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_shell-panel"));
    cmd.args(args).env_remove("SHELL_PANEL_SESSION");
    if in_session {
        cmd.env("SHELL_PANEL_SESSION", "1");
    }
    cmd.output().unwrap()
}

#[test]
fn test_refuses_to_start_inside_existing_session() {
    let out = run_with_session_env(&[], true);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("already running"));
}

#[test]
fn test_check_reports_session_state() {
    assert_eq!(
        run_with_session_env(&["--check"], true).status.code(),
        Some(0)
    );
    assert_eq!(
        run_with_session_env(&["--check"], false).status.code(),
        Some(1)
    );
}

#[test]
fn test_unsupported_shell_exits_with_code_2() {
    let out = run_with_session_env(&["--shell", "bash"], false);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unsupported shell \"bash\""));

    let config = std::env::temp_dir().join(format!("sp_bash_{}.toml", std::process::id()));
    std::fs::write(&config, "shell = \"bash\"\n").unwrap();
    let out = run_with_session_env(&["--config", config.to_str().unwrap()], false);
    let _ = std::fs::remove_file(&config);
    assert_eq!(out.status.code(), Some(2));
}
