use shell_panel::core::config::{ColorConfig, Config, IconConfig};
use shell_panel::engine::provider::{Suggestion, SuggestionKind};
use shell_panel::ui::theme::{format_suggestion_line, format_suggestion_line_with_theme, Theme};

#[test]
fn test_format_suggestion_line_with_icons() {
    let dir_sug = Suggestion::new("src/", "src/", Some("Directory".into()), 60)
        .with_kind(SuggestionKind::Directory);
    let formatted = format_suggestion_line(&dir_sug, false, 40);
    assert!(
        formatted.contains("📁"),
        "Must contain folder icon: {}",
        formatted
    );

    let cmd_sug = Suggestion::new("commit", "commit", Some("Commit changes".into()), 80)
        .with_kind(SuggestionKind::Subcommand);
    let formatted_cmd = format_suggestion_line(&cmd_sug, true, 40);
    assert!(
        formatted_cmd.contains("🔹"),
        "Must contain subcommand icon: {}",
        formatted_cmd
    );
}

#[test]
fn test_format_suggestion_line_selected_highlight() {
    let sug = Suggestion::new("checkout", "checkout", Some("Checkout branch".into()), 80)
        .with_kind(SuggestionKind::Subcommand);
    let formatted = format_suggestion_line(&sug, true, 50);

    assert!(formatted.starts_with("\x1b[7m"));
    assert!(formatted.ends_with("\x1b[0m"));
    assert!(formatted.contains("> "));
    assert!(formatted.contains("🔹 "));
    assert!(formatted.contains("checkout"));
    assert!(formatted.contains("Checkout branch"));
}

#[test]
fn test_format_suggestion_line_unselected_description_dim() {
    let sug = Suggestion::new("cargo", "cargo", Some("Rust package manager".into()), 80)
        .with_kind(SuggestionKind::Command);
    let formatted = format_suggestion_line(&sug, false, 50);

    assert!(formatted.contains("  "));
    assert!(formatted.contains("⚡ "));
    assert!(formatted.contains("cargo"));
    // Description should have dim styling \x1b[90m
    assert!(formatted.contains("\x1b[90m"));
}

#[test]
fn test_theme_default_values() {
    let theme = Theme::default();
    assert_eq!(theme.selected_start, "\x1b[7m");
    assert_eq!(theme.selected_end, "\x1b[0m");
    assert_eq!(theme.desc_start, "\x1b[90m");
    assert_eq!(theme.desc_end, "\x1b[0m");
    assert_eq!(theme.unselected_fg_start, "");
    assert_eq!(theme.selected_prefix, "> ");
    assert_eq!(theme.unselected_prefix, "  ");
    assert_eq!(theme.icons, IconConfig::default());
}

#[test]
fn test_theme_from_config_colors() {
    let config = Config {
        colors: ColorConfig {
            selected_bg: "blue".to_string(),
            selected_fg: "white".to_string(),
            unselected_fg: "gray".to_string(),
            description_fg: "yellow".to_string(),
            selected_prefix: "* ".to_string(),
            unselected_prefix: "- ".to_string(),
        },
        ..Default::default()
    };

    let theme = Theme::from_config(&config);
    assert_eq!(theme.selected_start, "\x1b[44m\x1b[37m");
    assert_eq!(theme.desc_start, "\x1b[33m");
    assert_eq!(theme.unselected_fg_start, "\x1b[90m");
    assert_eq!(theme.selected_prefix, "* ");
    assert_eq!(theme.unselected_prefix, "- ");
}

#[test]
fn test_theme_from_config_invert_and_none() {
    let mut config = Config::default();
    config.colors.selected_bg = "invert".to_string();
    config.colors.selected_fg = "white".to_string();
    let theme = Theme::from_config(&config);
    assert_eq!(theme.selected_start, "\x1b[7m");

    config.colors.selected_bg = "".to_string();
    config.colors.selected_fg = "".to_string();
    let theme_empty = Theme::from_config(&config);
    assert_eq!(theme_empty.selected_start, "\x1b[7m");
}

#[test]
fn test_theme_from_config_hex_and_256_colors() {
    let mut config = Config::default();
    config.colors.selected_bg = "#112233".to_string();
    config.colors.selected_fg = "250".to_string();
    config.colors.description_fg = "#abcdef".to_string();

    let theme = Theme::from_config(&config);
    assert_eq!(theme.selected_start, "\x1b[48;2;17;34;51m\x1b[38;5;250m");
    assert_eq!(theme.desc_start, "\x1b[38;2;171;205;239m");
}

#[test]
fn test_theme_from_config_custom_icons() {
    let config = Config {
        icons: IconConfig {
            directory: "[DIR] ".to_string(),
            file: "[FILE] ".to_string(),
            command: "[CMD] ".to_string(),
            subcommand: "[SUB] ".to_string(),
            option: "[OPT] ".to_string(),
            powershell_cmdlet: "[PS] ".to_string(),
            alias: "[AL] ".to_string(),
            other: "[?] ".to_string(),
        },
        ..Default::default()
    };

    let theme = Theme::from_config(&config);
    assert_eq!(theme.icon_for(SuggestionKind::Directory), "[DIR] ");
    assert_eq!(theme.icon_for(SuggestionKind::File), "[FILE] ");
    assert_eq!(theme.icon_for(SuggestionKind::Command), "[CMD] ");
    assert_eq!(theme.icon_for(SuggestionKind::Subcommand), "[SUB] ");
    assert_eq!(theme.icon_for(SuggestionKind::Option), "[OPT] ");
    assert_eq!(theme.icon_for(SuggestionKind::PowerShellCmdlet), "[PS] ");
    assert_eq!(theme.icon_for(SuggestionKind::Alias), "[AL] ");
    assert_eq!(theme.icon_for(SuggestionKind::Other), "[?] ");
}

#[test]
fn test_format_suggestion_line_with_custom_theme() {
    let mut config = Config {
        colors: ColorConfig {
            selected_bg: "magenta".to_string(),
            selected_fg: "white".to_string(),
            unselected_fg: "cyan".to_string(),
            description_fg: "yellow".to_string(),
            selected_prefix: ">> ".to_string(),
            unselected_prefix: "   ".to_string(),
        },
        ..Default::default()
    };
    config.icons.command = "$ ".to_string();

    let theme = Theme::from_config(&config);
    let sug = Suggestion::new("build", "build", Some("Compile project".into()), 100)
        .with_kind(SuggestionKind::Command);

    // Selected line formatting
    let selected_line = format_suggestion_line_with_theme(&sug, true, 40, &theme);
    assert!(selected_line.starts_with("\x1b[45m\x1b[37m"));
    assert!(selected_line.ends_with("\x1b[0m"));
    assert!(selected_line.contains(">> "));
    assert!(selected_line.contains("$ "));
    assert!(selected_line.contains("build"));
    assert!(selected_line.contains("Compile project"));

    // Unselected line formatting
    let unselected_line = format_suggestion_line_with_theme(&sug, false, 40, &theme);
    assert!(unselected_line.contains("   "));
    assert!(unselected_line.contains("$ "));
    assert!(unselected_line.contains("build"));
    assert!(unselected_line.contains("\x1b[36m")); // unselected_fg cyan
    assert!(unselected_line.contains("\x1b[33m")); // description_fg yellow
}

#[test]
fn test_control_characters_in_suggestion_text_are_drawn_inert() {
    let theme = Theme::default();
    let sug = Suggestion::new(
        "a",
        "a\u{1b}[1;1HPWNED",
        Some("x\u{1b}]52;c;aWV4\u{7}y\u{9b}2Jz".to_string()),
        50,
    );
    for selected in [false, true] {
        let line = format_suggestion_line_with_theme(&sug, selected, 80, &theme);
        assert!(!line.contains("\u{1b}[1;1H"), "{line:?}");
        assert!(!line.contains("\u{1b}]52"), "{line:?}");
        assert!(!line.contains('\u{7}'), "{line:?}");
        assert!(!line.contains('\u{9b}'), "{line:?}");
        assert!(line.contains("PWNED"), "{line:?}");
    }
}
