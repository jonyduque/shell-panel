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
