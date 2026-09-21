use std::fs::{create_dir_all, remove_dir_all, File};
use std::io::Write;

use shell_panel::engine::lexer::lex_command_line;
use shell_panel::engine::provider::{CompletionProvider, SuggestionKind};
use shell_panel::engine::providers::carapace::{
    carapace_args, parse_carapace_json, CarapaceProvider,
};
use shell_panel::engine::providers::files::FileProvider;
use shell_panel::engine::providers::json_spec::{FigSpec, JsonSpecProvider};
use shell_panel::engine::providers::zoxide::{parse_zoxide_output, ZoxideProvider};
use shell_panel::engine::replacement::{calculate_replacement, ReplacementAction};

// =========================================================================
// 1. Replacement Engine Tests
// =========================================================================

#[test]
fn test_calculate_replacement_exact_prefix() {
    // "sta" -> "status"
    let action = calculate_replacement("sta", "status");
    assert_eq!(
        action,
        ReplacementAction {
            backspace_count: 0,
            insert_text: "tus ".to_string(),
        }
    );

    // Empty input -> full suggestion
    let action_empty = calculate_replacement("", "status");
    assert_eq!(
        action_empty,
        ReplacementAction {
            backspace_count: 0,
            insert_text: "status ".to_string(),
        }
    );

    // Exactly typed match -> trailing space
    let action_full = calculate_replacement("status", "status");
    assert_eq!(
        action_full,
        ReplacementAction {
            backspace_count: 0,
            insert_text: " ".to_string(),
        }
    );
}

#[test]
fn test_calculate_replacement_case_mismatch() {
    let action = calculate_replacement("STA", "status");
    assert_eq!(
        action,
        ReplacementAction {
            backspace_count: 3,
            insert_text: "status ".to_string(),
        }
    );

    let action2 = calculate_replacement("git", "Git");
    assert_eq!(
        action2,
        ReplacementAction {
            backspace_count: 3,
            insert_text: "Git ".to_string(),
        }
    );
}

#[test]
fn test_calculate_replacement_fuzzy_and_unicode() {
    // Fuzzy / full replacement
    let action_fuzzy = calculate_replacement("stt", "status");
    assert_eq!(
        action_fuzzy,
        ReplacementAction {
            backspace_count: 3,
            insert_text: "status ".to_string(),
        }
    );

    // Unicode emoji exact prefix
    let action_emoji = calculate_replacement("🚀te", "🚀test");
    assert_eq!(
        action_emoji,
        ReplacementAction {
            backspace_count: 0,
            insert_text: "st ".to_string(),
        }
    );

    // Unicode emoji case mismatch / replacement (char count, not byte count)
    // "🚀TE" is 3 chars (🚀 is 4 bytes, T is 1 byte, E is 1 byte = 6 bytes)
    let action_emoji_case = calculate_replacement("🚀TE", "🚀test");
    assert_eq!(
        action_emoji_case,
        ReplacementAction {
            backspace_count: 3,
            insert_text: "🚀test ".to_string(),
        }
    );

    // CJK characters
    // "你好" is 2 chars (6 bytes)
    let action_cjk = calculate_replacement("你好", "你好世界");
    assert_eq!(
        action_cjk,
        ReplacementAction {
            backspace_count: 0,
            insert_text: "世界 ".to_string(),
        }
    );

    let action_cjk_replace = calculate_replacement("你好", "こんにちは");
    assert_eq!(
        action_cjk_replace,
        ReplacementAction {
            backspace_count: 2,
            insert_text: "こんにちは ".to_string(),
        }
    );
}

// =========================================================================
// 2. FileProvider Tests
// =========================================================================

#[tokio::test]
async fn test_file_provider_listing() {
    let temp_dir = std::env::temp_dir().join(format!(
        "shell_panel_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    create_dir_all(&temp_dir).unwrap();
    create_dir_all(temp_dir.join("subfolder")).unwrap();
    File::create(temp_dir.join("file1.txt"))
        .unwrap()
        .write_all(b"test1")
        .unwrap();
    File::create(temp_dir.join("file2.log"))
        .unwrap()
        .write_all(b"test2")
        .unwrap();
    File::create(temp_dir.join("subfolder").join("nested.rs"))
        .unwrap()
        .write_all(b"fn main() {}")
        .unwrap();

    let provider = FileProvider::new();
    assert_eq!(provider.name(), "files");
    assert!(provider.can_handle("anything"));

    let temp_dir_str = temp_dir.to_str().unwrap();

    // Listing all entries in directory
    let suggestions = provider.complete("cat ", temp_dir_str).await;
    assert_eq!(suggestions.len(), 3);

    // Directory has priority 60 and ends with /
    let subfolder_sug = suggestions.iter().find(|s| s.name.starts_with("subfolder"));
    assert!(subfolder_sug.is_some());
    let subfolder = subfolder_sug.unwrap();
    assert_eq!(subfolder.priority, 60);
    assert_eq!(subfolder.name, "subfolder/");
    assert_eq!(subfolder.description, Some("Directory".into()));
    assert_eq!(subfolder.kind, SuggestionKind::Directory);

    // Files have priority 50
    let file1_sug = suggestions.iter().find(|s| s.name == "file1.txt").unwrap();
    assert_eq!(file1_sug.priority, 50);
    assert_eq!(file1_sug.kind, SuggestionKind::File);

    // Filter by prefix "file"
    let file_filtered = provider.complete("cat file", temp_dir_str).await;
    assert_eq!(file_filtered.len(), 2);
    assert!(file_filtered.iter().any(|s| s.name == "file1.txt"));
    assert!(file_filtered.iter().any(|s| s.name == "file2.log"));

    // Filter subfolder contents with path prefix
    let subfolder_completions = provider.complete("cat subfolder/", temp_dir_str).await;
    assert_eq!(subfolder_completions.len(), 1);
    assert_eq!(subfolder_completions[0].name, "subfolder/nested.rs");

    // Clean up
    let _ = remove_dir_all(&temp_dir);
}

// =========================================================================
// 3. JsonSpecProvider Tests
// =========================================================================

#[tokio::test]
async fn test_json_spec_provider() {
    let spec_json = r#"{
        "name": "git",
        "description": "the stupid content tracker",
        "subcommands": [
            {
                "name": "status",
                "description": "Show the working tree status"
            },
            {
                "name": "commit",
                "description": "Record changes to the repository",
                "options": [
                    {
                        "name": ["-m", "--message"],
                        "description": "Use the given msg as commit message"
                    },
                    {
                        "name": ["-a", "--all"],
                        "description": "Commit all changed files"
                    }
                ]
            }
        ],
        "options": [
            {
                "name": ["-v", "--version"],
                "description": "Show version"
            }
        ]
    }"#;

    let spec: FigSpec = serde_json::from_str(spec_json).unwrap();
    let mut provider = JsonSpecProvider::new();
    provider.add_spec(spec);

    assert_eq!(provider.name(), "json_spec");
    assert!(provider.can_handle("git"));
    assert!(!provider.can_handle("cargo"));

    // Subcommand matching: `git stat`
    let suggestions = provider.complete("git stat", "").await;
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].name, "status");
    assert_eq!(suggestions[0].display, "status");
    assert_eq!(
        suggestions[0].description,
        Some("Show the working tree status".to_string())
    );
    assert_eq!(suggestions[0].priority, 80);
    assert_eq!(suggestions[0].kind, SuggestionKind::Subcommand);

    // Option matching under subcommand: `git commit -m`
    let suggestions_commit_opt = provider.complete("git commit -m", "").await;
    assert_eq!(suggestions_commit_opt.len(), 1);
    assert_eq!(suggestions_commit_opt[0].name, "-m");
    assert_eq!(
        suggestions_commit_opt[0].description,
        Some("Use the given msg as commit message".to_string())
    );
    assert_eq!(suggestions_commit_opt[0].priority, 75);
    assert_eq!(suggestions_commit_opt[0].kind, SuggestionKind::Option);

    // Option matching under subcommand with `-`: all options for commit
    let all_commit_opts = provider.complete("git commit -", "").await;
    let names: Vec<String> = all_commit_opts.into_iter().map(|s| s.name).collect();
    assert!(names.contains(&"-m".to_string()));
    assert!(names.contains(&"--message".to_string()));
    assert!(names.contains(&"-a".to_string()));
    assert!(names.contains(&"--all".to_string()));

    // Root option matching: `git -`
    let root_opts = provider.complete("git -", "").await;
    let root_names: Vec<String> = root_opts.into_iter().map(|s| s.name).collect();
    assert!(root_names.contains(&"-v".to_string()));
    assert!(root_names.contains(&"--version".to_string()));
}

// =========================================================================
// 4. CarapaceProvider Tests
// =========================================================================

#[test]
fn test_carapace_provider_parse() {
    let provider = CarapaceProvider::new();
    assert_eq!(provider.name(), "carapace");
    assert!(provider.can_handle("git"));
    assert!(!provider.can_handle(""));

    // Array format
    let json_array = r#"[
        { "value": "status", "display": "status", "description": "Show the working tree status", "style": "blue" },
        { "value": "commit", "display": "commit", "description": "Record changes to the repository" }
    ]"#;

    let parsed = parse_carapace_json(json_array);
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].name, "status");
    assert_eq!(parsed[0].display, "status");
    assert_eq!(
        parsed[0].description,
        Some("Show the working tree status".to_string())
    );
    assert_eq!(parsed[0].priority, 70);
    assert_eq!(parsed[0].kind, SuggestionKind::Subcommand);

    assert_eq!(parsed[1].name, "commit");
    assert_eq!(parsed[1].display, "commit");
    assert_eq!(
        parsed[1].description,
        Some("Record changes to the repository".to_string())
    );
    assert_eq!(parsed[1].kind, SuggestionKind::Subcommand);

    // Object format with option
    let json_object = r#"{
        "values": [
            { "value": "checkout", "display": "checkout", "description": "Switch branches" },
            { "value": "--help", "display": "--help", "description": "Show help" }
        ]
    }"#;

    let parsed_obj = parse_carapace_json(json_object);
    assert_eq!(parsed_obj.len(), 2);
    assert_eq!(parsed_obj[0].name, "checkout");
    assert_eq!(parsed_obj[0].kind, SuggestionKind::Subcommand);
    assert_eq!(parsed_obj[1].name, "--help");
    assert_eq!(parsed_obj[1].kind, SuggestionKind::Option);

    // Invalid JSON returns empty vec
    let invalid = parse_carapace_json("not json");
    assert!(invalid.is_empty());
}

#[test]
fn test_carapace_args_use_completer_export_form() {
    // `carapace <completer> export <completer> <args...>`; `_carapace` would complete carapace itself.
    assert_eq!(
        carapace_args(&lex_command_line("git sta")),
        vec!["git", "export", "git", "sta"]
    );
    assert_eq!(
        carapace_args(&lex_command_line("npm ")),
        vec!["npm", "export", "npm", ""]
    );
    assert!(carapace_args(&lex_command_line("")).is_empty());
}

// =========================================================================
// 5. ZoxideProvider Tests
// =========================================================================

#[test]
fn test_zoxide_provider() {
    let provider = ZoxideProvider::new();
    assert_eq!(provider.name(), "zoxide");
    assert!(provider.can_handle("cd"));
    assert!(provider.can_handle("z"));
    assert!(provider.can_handle("zi"));
    assert!(!provider.can_handle("ls"));
    assert!(!provider.can_handle("dir"));

    let zoxide_output = "C:\\Users\\jonyd\\Projetos\\inshellisense\nC:\\Users\\jonyd\\Documents\nD:\\Data\\Archive\n";

    // All paths with empty prefix
    let all = parse_zoxide_output(zoxide_output, "");
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].name, "C:\\Users\\jonyd\\Projetos\\inshellisense");
    assert_eq!(all[0].description, Some("Zoxide Directory".to_string()));
    assert_eq!(all[0].priority, 70);
    assert_eq!(all[0].kind, SuggestionKind::Directory);

    // Filter by path prefix "C:\Users"
    let users = parse_zoxide_output(zoxide_output, "C:\\Users");
    assert_eq!(users.len(), 2);

    // Filter by directory leaf name "Archive"
    let archive = parse_zoxide_output(zoxide_output, "archive");
    assert_eq!(archive.len(), 1);
    assert_eq!(archive[0].name, "D:\\Data\\Archive");

    // Filter with no matches
    let none = parse_zoxide_output(zoxide_output, "nonexistent");
    assert!(none.is_empty());
}
