use shell_panel::engine::lexer::lex_command_line;
use shell_panel::engine::provider::{CompletionProvider, SuggestionKind};
use shell_panel::engine::providers::carapace::{
    carapace_args, parse_carapace_json, CarapaceProvider,
};
use shell_panel::engine::providers::json_spec::{FigSpec, JsonSpecProvider};
use shell_panel::engine::providers::zoxide::{
    parse_zoxide_output, quote_for_powershell, ZoxideProvider,
};
use shell_panel::engine::replacement::{
    calculate_replacement, replace_range, utf16_to_byte_index, ReplacementAction,
};

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
            delete_count: 0,
            insert_text: "tus ".to_string(),
        }
    );

    // Empty input -> full suggestion
    let action_empty = calculate_replacement("", "status");
    assert_eq!(
        action_empty,
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "status ".to_string(),
        }
    );

    // Exactly typed match -> trailing space
    let action_full = calculate_replacement("status", "status");
    assert_eq!(
        action_full,
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
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
            delete_count: 0,
            insert_text: "status ".to_string(),
        }
    );

    let action2 = calculate_replacement("git", "Git");
    assert_eq!(
        action2,
        ReplacementAction {
            backspace_count: 3,
            delete_count: 0,
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
            delete_count: 0,
            insert_text: "status ".to_string(),
        }
    );

    // Unicode emoji exact prefix
    let action_emoji = calculate_replacement("🚀te", "🚀test");
    assert_eq!(
        action_emoji,
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "st ".to_string(),
        }
    );

    // Unicode emoji case mismatch / replacement (UTF-16 code units, not bytes or scalars)
    // "🚀TE" is 4 UTF-16 units (🚀 is a surrogate pair = 2, T is 1, E is 1)
    let action_emoji_case = calculate_replacement("🚀TE", "🚀test");
    assert_eq!(
        action_emoji_case,
        ReplacementAction {
            backspace_count: 4,
            delete_count: 0,
            insert_text: "🚀test ".to_string(),
        }
    );

    // CJK characters
    // "你好" is 2 chars, each one UTF-16 unit
    let action_cjk = calculate_replacement("你好", "你好世界");
    assert_eq!(
        action_cjk,
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "世界 ".to_string(),
        }
    );

    let action_cjk_replace = calculate_replacement("你好", "こんにちは");
    assert_eq!(
        action_cjk_replace,
        ReplacementAction {
            backspace_count: 2,
            delete_count: 0,
            insert_text: "こんにちは ".to_string(),
        }
    );
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

#[test]
fn test_utf16_to_byte_index() {
    assert_eq!(utf16_to_byte_index("", 0), Some(0));
    assert_eq!(utf16_to_byte_index("ação x", 4), Some(6));
    assert_eq!(utf16_to_byte_index("ação x", 6), Some(8));
    assert_eq!(utf16_to_byte_index("🚀a", 1), None); // inside a surrogate pair
    assert_eq!(utf16_to_byte_index("🚀a", 2), Some(4));
    assert_eq!(utf16_to_byte_index("ab", 3), None);
}

#[test]
fn test_replacement_bytes_and_forward_delete() {
    // Cursor inside `Get-Child|Item`, suggestion `Get-ChildItem`.
    let action = replace_range("Get-Child", "Item", "Get-ChildItem");
    assert_eq!(
        action,
        ReplacementAction {
            backspace_count: 9,
            delete_count: 4,
            insert_text: "Get-ChildItem ".into()
        }
    );
    let mut expected = vec![0x7f; 9];
    for _ in 0..4 {
        expected.extend_from_slice(b"\x1b[3~");
    }
    expected.extend_from_slice(b"Get-ChildItem ");
    assert_eq!(action.to_bytes(), expected);

    // Nothing after the cursor: same as the prefix logic (only the suffix is typed).
    assert_eq!(
        replace_range("System.IO.Fi", "", "System.IO.File"),
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "le ".into()
        }
    );
}

#[test]
fn test_no_trailing_space_after_quoted_directory() {
    assert_eq!(
        calculate_replacement("'My", r"'.\My Documents\'").insert_text,
        r"'.\My Documents\'"
    );
    assert_eq!(
        calculate_replacement("a", "'a b.txt'").insert_text,
        "'a b.txt' "
    );
}

#[test]
fn test_zoxide_paths_are_quoted_for_powershell() {
    assert_eq!(quote_for_powershell(r"C:\src"), r"C:\src");
    assert_eq!(
        quote_for_powershell(r"C:\My Documents"),
        r"'C:\My Documents'"
    );
    assert_eq!(quote_for_powershell(r"C:\it's"), r"'C:\it''s'");

    let sugs = parse_zoxide_output("C:\\My Documents\n", "my");
    assert_eq!(sugs[0].name, r"'C:\My Documents'");
    assert_eq!(sugs[0].display, r"C:\My Documents");
}

#[test]
fn test_deletion_counts_are_utf16_code_units() {
    // PSReadLine removes one UTF-16 code unit per key, so a surrogate pair costs two keys on
    // both sides of the cursor. `a🚀b` and `c🚀d` are 4 code units each (3 scalars each).
    let action = replace_range("a🚀b", "c🚀d", "plain");
    assert_eq!(
        action,
        ReplacementAction {
            backspace_count: 4,
            delete_count: 4,
            insert_text: "plain ".to_string(),
        }
    );

    let mut expected = vec![0x7f; 4];
    for _ in 0..4 {
        expected.extend_from_slice(b"\x1b[3~");
    }
    expected.extend_from_slice(b"plain ");
    assert_eq!(action.to_bytes(), expected);

    // Same rule when only text before the cursor is replaced.
    assert_eq!(
        calculate_replacement("🚀🚀", "done").backspace_count,
        4,
        "two surrogate pairs need four backspaces"
    );
}

#[tokio::test]
async fn test_load_dir_adds_user_specs_and_reports_bad_files() {
    let dir = std::env::temp_dir().join(format!("sp_specs_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mytool.json"),
        r#"{"name":"mytool","subcommands":[{"name":"deploy","description":"Ship it"}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("git.json"),
        r#"{"name":"git","subcommands":[{"name":"onlymine"}]}"#,
    )
    .unwrap();
    std::fs::write(dir.join("broken.json"), "{").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();

    let mut provider = JsonSpecProvider::with_embedded_specs();
    let warnings = provider.load_dir(&dir);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(warnings.len(), 1, "{:?}", warnings);
    assert!(warnings[0].contains("broken.json"));
    let sugs = provider.complete("mytool dep", "").await;
    assert_eq!(sugs[0].name, "deploy");
    let names: Vec<String> = provider
        .complete("git ", "")
        .await
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, vec!["onlymine".to_string()]);
}

#[test]
fn test_carapace_values_are_quoted_for_powershell() {
    let sugs = parse_carapace_json(
        r#"{"values":[{"value":"My Dir/","display":"My Dir/"},{"value":"--force"},{"value":"main"}]}"#,
    );
    let names: Vec<&str> = sugs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["'My Dir/'", "--force", "main"]);
    assert_eq!(sugs[0].display, "My Dir/");
}

#[test]
fn test_every_powershell_metacharacter_is_quoted() {
    for path in [
        r"C:\a;b", r"C:\x$y", r"C:\a&b", r"C:\p(1)", r"C:\a b", r"C:\a,b", r"C:\a|b",
    ] {
        let quoted = quote_for_powershell(path);
        assert!(
            quoted.starts_with('\'') && quoted.ends_with('\''),
            "{path} -> {quoted}"
        );
    }
}

#[test]
fn test_double_quoted_directory_gets_no_trailing_space() {
    // M10: a double-quoted directory keeps the cursor behind the backslash.
    let action = calculate_replacement("\"My", r#"".\My Documents\""#);
    assert_eq!(action.insert_text, r#"".\My Documents\""#);
    assert!(!action.insert_text.ends_with(' '));
}

#[test]
fn test_carapace_flag_value_with_a_space_is_quoted_after_the_equals() {
    let sugs = parse_carapace_json(
        r#"{"values":[{"value":"--name=a b"},{"value":"--force"},{"value":"--k=v"}]}"#,
    );
    let names: Vec<&str> = sugs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["--name='a b'", "--force", "--k=v"]);
    assert!(sugs.iter().all(|s| s.kind == SuggestionKind::Option));
}

/// Writes a `.cmd` helper that sleeps about five seconds.
fn slow_helper() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shell-panel-slow-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("slow.cmd");
    std::fs::write(&path, "@ping -n 6 127.0.0.1 >nul\r\n").unwrap();
    path
}

#[tokio::test]
async fn test_slow_carapace_helper_times_out_empty_and_fast() {
    let helper = slow_helper();
    let provider = CarapaceProvider::with_binary(helper.to_string_lossy());
    let started = std::time::Instant::now();
    let sugs = provider.complete("git sta", "").await;
    assert!(sugs.is_empty());
    assert!(started.elapsed() < std::time::Duration::from_millis(1000));
}

#[tokio::test]
async fn test_slow_zoxide_helper_times_out_empty_and_fast() {
    let helper = slow_helper();
    let provider = ZoxideProvider::with_binary(helper.to_string_lossy());
    let started = std::time::Instant::now();
    let sugs = provider.complete("cd x", "").await;
    assert!(sugs.is_empty());
    assert!(started.elapsed() < std::time::Duration::from_millis(1000));
}
