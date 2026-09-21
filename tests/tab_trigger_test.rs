use std::collections::HashSet;

use shell_panel::core::app::should_include_files;
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
fn test_replacement_del_0x7f_encoding() {
    let rep = calculate_replacement("xyz", "commit");
    let mut write_buf = Vec::new();
    for _ in 0..rep.backspace_count {
        write_buf.push(0x7f);
    }
    write_buf.extend_from_slice(rep.insert_text.as_bytes());

    assert_eq!(
        write_buf,
        vec![0x7f, 0x7f, 0x7f, b'c', b'o', b'm', b'm', b'i', b't', b' ']
    );

    let rep_prefix = calculate_replacement("c", "commit");
    let mut prefix_buf = Vec::new();
    for _ in 0..rep_prefix.backspace_count {
        prefix_buf.push(0x7f);
    }
    prefix_buf.extend_from_slice(rep_prefix.insert_text.as_bytes());

    assert_eq!(prefix_buf, b"ommit ".to_vec());
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
fn test_file_provider_gating_logic_without_subcommands_or_commands() {
    // Only options present
    let opt_sugs = vec![
        Suggestion::new("--help", "--help", Some("Show help".into()), 75)
            .with_kind(SuggestionKind::Option),
        Suggestion::new("-v", "-v", Some("Verbose".into()), 75)
            .with_kind(SuggestionKind::Option),
    ];

    assert!(should_include_files("file_name", &opt_sugs));
    assert!(should_include_files("any_arg", &opt_sugs));

    // Empty suggestions
    assert!(should_include_files("random", &[]));

    // Other kinds (e.g. Directory / File)
    let file_sugs = vec![
        Suggestion::new("Cargo.toml", "Cargo.toml", None, 50)
            .with_kind(SuggestionKind::File),
    ];
    assert!(should_include_files("Cargo", &file_sugs));
}

#[test]
fn test_priority_sorting_and_deduplication() {
    let mut suggestions = vec![
        Suggestion::new("commit", "commit", Some("File called commit".into()), 50)
            .with_kind(SuggestionKind::File),
        Suggestion::new("commit", "commit", Some("Subcommand commit".into()), 80)
            .with_kind(SuggestionKind::Subcommand),
        Suggestion::new("--clone", "--clone", Some("Flag".into()), 75)
            .with_kind(SuggestionKind::Option),
        Suggestion::new("checkout", "checkout", Some("Subcommand".into()), 80)
            .with_kind(SuggestionKind::Subcommand),
    ];

    suggestions.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.name.cmp(&b.name)));
    let mut seen = HashSet::new();
    suggestions.retain(|s| seen.insert(s.name.clone()));

    // Deduplicated list should keep highest priority "commit" (80, Subcommand)
    assert_eq!(suggestions.len(), 3);
    assert_eq!(suggestions[0].name, "checkout");
    assert_eq!(suggestions[0].priority, 80);
    assert_eq!(suggestions[1].name, "commit");
    assert_eq!(suggestions[1].priority, 80);
    assert_eq!(suggestions[1].kind, SuggestionKind::Subcommand);
    assert_eq!(suggestions[2].name, "--clone");
    assert_eq!(suggestions[2].priority, 75);
}
