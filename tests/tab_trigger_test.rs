use shell_panel::engine::aggregate::should_include_files;
use shell_panel::engine::provider::{Suggestion, SuggestionKind};
use shell_panel::engine::replacement::calculate_replacement;

#[test]
fn test_replacement_action_calculation() {
    let rep = calculate_replacement("c", "commit");
    assert_eq!(rep.backspace_count, 0);
    assert_eq!(rep.insert_text, "ommit ");

    let rep2 = calculate_replacement("commit", "commit");
    assert_eq!(rep2.backspace_count, 0);
    assert_eq!(rep2.insert_text, " ");

    let rep3 = calculate_replacement("xyz", "commit");
    assert_eq!(rep3.backspace_count, 3);
    assert_eq!(rep3.insert_text, "commit ");

    let rep_dir = calculate_replacement("src", "src/");
    assert_eq!(rep_dir.backspace_count, 0);
    assert_eq!(rep_dir.insert_text, "/");

    let rep_win_dir = calculate_replacement("src", "src\\");
    assert_eq!(rep_win_dir.backspace_count, 0);
    assert_eq!(rep_win_dir.insert_text, "\\");
}

#[test]
fn test_file_provider_gating_logic_with_subcommands() {
    let subcmd_sugs = vec![
        Suggestion::new("commit", "commit", Some("Commit changes".into()), 80)
            .with_kind(SuggestionKind::Subcommand),
        Suggestion::new("clone", "clone", Some("Clone repo".into()), 80)
            .with_kind(SuggestionKind::Subcommand),
    ];

    // Plain tokens must NOT include files when subcommands exist
    assert!(!should_include_files("c", &subcmd_sugs));
    assert!(!should_include_files("git", &subcmd_sugs));
    assert!(!should_include_files("", &subcmd_sugs));

    // Tokens containing '/' or '\' or starting with '.' MUST include files
    assert!(should_include_files("./file", &subcmd_sugs));
    assert!(should_include_files(".", &subcmd_sugs));
    assert!(should_include_files("..", &subcmd_sugs));
    assert!(should_include_files(".git", &subcmd_sugs));
    assert!(should_include_files("path/to", &subcmd_sugs));
    assert!(should_include_files(r"path\to", &subcmd_sugs));
    assert!(should_include_files("/usr/bin", &subcmd_sugs));
    assert!(should_include_files(r"C:\Windows", &subcmd_sugs));
}

#[test]
fn test_file_provider_gating_logic_with_commands() {
    let cmd_sugs = vec![
        Suggestion::new("cargo", "cargo", Some("Rust package manager".into()), 80)
            .with_kind(SuggestionKind::Command),
    ];

    assert!(!should_include_files("car", &cmd_sugs));
    assert!(should_include_files("./car", &cmd_sugs));
    assert!(should_include_files("dir/", &cmd_sugs));
}

#[test]
fn test_file_provider_gating_logic_with_options() {
    let opt_sugs = vec![
        Suggestion::new("--help", "--help", Some("Show help".into()), 75)
            .with_kind(SuggestionKind::Option),
        Suggestion::new("-v", "-v", Some("Verbose".into()), 75).with_kind(SuggestionKind::Option),
    ];

    // Options must NOT include files for plain argument/option tokens
    assert!(!should_include_files("file_name", &opt_sugs));
    assert!(!should_include_files("-", &opt_sugs));
    assert!(!should_include_files("", &opt_sugs));

    // Tokens containing '/' or '\' or starting with '.' MUST include files
    assert!(should_include_files("./file_name", &opt_sugs));
    assert!(should_include_files(".", &opt_sugs));
    assert!(should_include_files("dir/file", &opt_sugs));
    assert!(should_include_files(r"dir\file", &opt_sugs));
}

#[test]
fn test_file_provider_gating_logic_with_powershell_cmdlets() {
    let cmdlet_sugs =
        vec![
            Suggestion::new("Get-ChildItem", "Get-ChildItem", Some("Cmdlet".into()), 80)
                .with_kind(SuggestionKind::PowerShellCmdlet),
        ];

    // Cmdlets must NOT include files for plain cmdlet query tokens
    assert!(!should_include_files("Get-Ch", &cmdlet_sugs));
    assert!(!should_include_files("Get-ChildItem", &cmdlet_sugs));
    assert!(!should_include_files("", &cmdlet_sugs));

    // Tokens containing '/' or '\' or starting with '.' MUST include files
    assert!(should_include_files("./Get-Ch", &cmdlet_sugs));
    assert!(should_include_files(".", &cmdlet_sugs));
    assert!(should_include_files("dir/cmdlet", &cmdlet_sugs));
    assert!(should_include_files(r"dir\cmdlet", &cmdlet_sugs));
}

#[test]
fn test_file_provider_gating_logic_without_gating_suggestions() {
    // Empty suggestions
    assert!(should_include_files("random", &[]));

    // Other kinds (e.g. Directory / File / Other)
    let file_sugs =
        vec![Suggestion::new("Cargo.toml", "Cargo.toml", None, 50).with_kind(SuggestionKind::File)];
    assert!(should_include_files("Cargo", &file_sugs));

    let other_sugs =
        vec![Suggestion::new("$env:PATH", "$env:PATH", None, 70).with_kind(SuggestionKind::Other)];
    assert!(should_include_files("PATH", &other_sugs));
}
