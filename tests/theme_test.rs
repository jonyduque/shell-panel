use shell_panel::engine::provider::{Suggestion, SuggestionKind};
use shell_panel::ui::theme::format_suggestion_line;

#[test]
fn test_format_suggestion_line_with_icons() {
    let dir_sug = Suggestion::new("src/", "src/", Some("Directory".into()), 60)
        .with_kind(SuggestionKind::Directory);
    let formatted = format_suggestion_line(&dir_sug, false, 40);
    assert!(formatted.contains("📁"), "Must contain folder icon: {}", formatted);

    let cmd_sug = Suggestion::new("commit", "commit", Some("Commit changes".into()), 80)
        .with_kind(SuggestionKind::Subcommand);
    let formatted_cmd = format_suggestion_line(&cmd_sug, true, 40);
    assert!(formatted_cmd.contains("🔹"), "Must contain subcommand icon: {}", formatted_cmd);
}

#[test]
fn test_all_suggestion_kind_icons() {
    assert_eq!(SuggestionKind::Directory.icon(), "📁 ");
    assert_eq!(SuggestionKind::File.icon(), "📄 ");
    assert_eq!(SuggestionKind::Command.icon(), "⚡ ");
    assert_eq!(SuggestionKind::Subcommand.icon(), "🔹 ");
    assert_eq!(SuggestionKind::Option.icon(), "🏷️  ");
    assert_eq!(SuggestionKind::PowerShellCmdlet.icon(), ">_ ");
    assert_eq!(SuggestionKind::Alias.icon(), "🔗 ");
    assert_eq!(SuggestionKind::Other.icon(), "  ");
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
