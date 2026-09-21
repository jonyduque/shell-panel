use shell_panel::core::config::{default_config_path, default_sample_toml, Config};
use std::path::{Path, PathBuf};

#[test]
fn test_default_config_path_location() {
    let path = default_config_path().expect("Should resolve home/userprofile dir");
    let path_str = path.to_string_lossy().replace('\\', "/");
    assert!(
        path_str.ends_with(".config/shell-panel.toml"),
        "Path should end with .config/shell-panel.toml, got: {}",
        path_str
    );
}

#[test]
fn test_parse_custom_toml() {
    let toml_content = r#"
        max_suggestions = 10
        shell = "pwsh"

        [colors]
        selected_bg = "blue"
        selected_fg = "white"
        description_fg = "yellow"

        [icons]
        directory = "📁 "
        subcommand = "⚡ "
    "#;

    let config: Config = toml::from_str(toml_content).expect("Failed to parse TOML");
    assert_eq!(config.max_suggestions, 10);
    assert_eq!(config.shell, Some("pwsh".to_string()));
    assert_eq!(config.colors.selected_bg, "blue");
    assert_eq!(config.colors.selected_fg, "white");
    assert_eq!(config.colors.unselected_fg, ""); // default preserved
    assert_eq!(config.colors.description_fg, "yellow");
    assert_eq!(config.colors.selected_prefix, "> "); // default preserved
    assert_eq!(config.colors.unselected_prefix, "  "); // default preserved
    assert_eq!(config.icons.subcommand, "⚡ ");
    assert_eq!(config.icons.directory, "📁 ");
    assert_eq!(config.icons.file, "📄 "); // default preserved
    assert_eq!(config.icons.command, "⚡ "); // default preserved
    assert_eq!(config.icons.option, "🏷️  "); // default preserved
    assert_eq!(config.icons.powershell_cmdlet, ">_ "); // default preserved
    assert_eq!(config.icons.alias, "🔗 "); // default preserved
    assert_eq!(config.icons.other, "  "); // default preserved
}

#[test]
fn test_load_or_default_non_existent_path() {
    let non_existent = PathBuf::from("non_existent_config_file_12345.toml");
    let config = Config::load_or_default(Some(&non_existent));
    assert_eq!(config, Config::default());
}

#[test]
fn test_load_or_default_valid_file() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("shell_panel_test_{}.toml", std::process::id()));

    let content = r#"
        max_suggestions = 8
        shell = "powershell"

        [colors]
        selected_bg = "magenta"
    "#;

    std::fs::write(&temp_file, content).expect("Failed to write temp config");

    let config = Config::load_or_default(Some(&temp_file));
    let _ = std::fs::remove_file(&temp_file);

    assert_eq!(config.max_suggestions, 8);
    assert_eq!(config.shell, Some("powershell".to_string()));
    assert_eq!(config.colors.selected_bg, "magenta");
    assert_eq!(config.colors.selected_fg, "black"); // default preserved
}

#[test]
fn test_default_sample_toml_validity() {
    let sample = default_sample_toml();
    let parsed: Result<Config, _> = toml::from_str(sample);
    assert!(
        parsed.is_ok(),
        "Sample TOML must parse cleanly: {:?}",
        parsed.err()
    );
}

fn write_temp(tag: &str, content: &str) -> std::path::PathBuf {
    let file = std::env::temp_dir().join(format!("sp_cfg_{}_{}.toml", tag, std::process::id()));
    std::fs::write(&file, content).unwrap();
    file
}

#[test]
fn test_unknown_keys_are_reported_but_the_rest_is_used() {
    let file = write_temp(
        "typo",
        "max_suggestions = 9\ndebounce_ms = 30\n[colors]\nselected_gb = \"red\"\n",
    );
    let (config, warnings) = Config::load(Some(&file));
    let _ = std::fs::remove_file(&file);

    assert_eq!(config.max_suggestions, 9);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("debounce_ms")));
    assert!(warnings.iter().any(|w| w.contains("colors.selected_gb")));
}

#[test]
fn test_invalid_or_missing_explicit_file_is_reported() {
    let file = write_temp("bad", "max_suggestions = \"many\"\n");
    let (config, warnings) = Config::load(Some(&file));
    let _ = std::fs::remove_file(&file);
    assert_eq!(config, Config::default());
    assert_eq!(warnings.len(), 1);

    let (config, warnings) = Config::load(Some(Path::new("non_existent_config_file_12345.toml")));
    assert_eq!(config, Config::default());
    assert_eq!(warnings.len(), 1);
}
