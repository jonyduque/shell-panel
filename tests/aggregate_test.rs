use shell_panel::engine::aggregate::{
    merge_suggestions, plan_replacement, shell_suggestions, CompletionEngine,
};
use shell_panel::engine::provider::{Suggestion, SuggestionKind};
use shell_panel::engine::providers::json_spec::{FigSpec, FigSubcommand, JsonSpecProvider};
use shell_panel::engine::replacement::ReplacementAction;
use shell_panel::shell::report::{ShellMatch, ShellReport};

fn report(
    line: &str,
    cursor: usize,
    index: i64,
    length: i64,
    matches: Vec<ShellMatch>,
) -> ShellReport {
    ShellReport {
        line: line.into(),
        cursor,
        replacement_index: index,
        replacement_length: length,
        matches,
    }
}

fn m(text: &str, list: &str, kind: &str, tip: &str) -> ShellMatch {
    ShellMatch(text.into(), list.into(), kind.into(), tip.into())
}

#[test]
fn test_shell_suggestions_kinds_and_descriptions() {
    let sugs = shell_suggestions(&report(
        "x",
        1,
        0,
        1,
        vec![
            m(
                "Get-ChildItem",
                "Get-ChildItem",
                "Command",
                "\r\nGet-ChildItem [[-Path] <string[]>]\r\n",
            ),
            m("git", "git", "Command", "git"),
            m("ls", "ls", "Command", "Get-ChildItem"),
            m("-Path", "Path", "ParameterName", "[string[]] Path"),
            m(r".\src\", "src", "ProviderContainer", r"C:\p\src"),
            m(r".\a.txt", "a.txt", "ProviderItem", r"C:\p\a.txt"),
            m("$x", "x", "Variable", "x"),
        ],
    ));
    let kinds: Vec<SuggestionKind> = sugs.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        vec![
            SuggestionKind::PowerShellCmdlet,
            SuggestionKind::Command, // a native command is not an alias just because it is short
            SuggestionKind::Alias,
            SuggestionKind::Option,
            SuggestionKind::Directory,
            SuggestionKind::File,
            SuggestionKind::Other,
        ]
    );
    assert!(sugs.iter().all(|s| s.uses_shell_range));
    assert_eq!(
        sugs[0].description.as_deref(),
        Some("Get-ChildItem [[-Path] <string[]>]")
    );
    assert_eq!(sugs[1].description, None); // tooltip equal to the text adds nothing
    assert_eq!(sugs[4].display, "src");
}

#[test]
fn test_merge_prefers_specs_hides_files_and_dedupes_path_styles() {
    let external = vec![
        Suggestion::new("status", "status", Some("spec".into()), 80)
            .with_kind(SuggestionKind::Subcommand),
        Suggestion::new("stash", "stash", Some("carapace".into()), 70)
            .with_kind(SuggestionKind::Subcommand),
        Suggestion::new("status", "status", Some("carapace".into()), 70)
            .with_kind(SuggestionKind::Subcommand),
    ];
    let shell = vec![Suggestion::new(r".\stats.txt", "stats.txt", None, 50)
        .with_kind(SuggestionKind::File)
        .with_shell_range()];
    let merged = merge_suggestions("sta", external, shell);
    let names: Vec<&str> = merged.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["status", "stash"]); // one `status` (the spec's), no file
    assert_eq!(merged[0].description.as_deref(), Some("spec"));

    // A path-like token brings files back, and `src/` equals `.\src\` for de-duplication.
    let external =
        vec![Suggestion::new("src/", "src/", None, 70).with_kind(SuggestionKind::Subcommand)];
    let shell = vec![
        Suggestion::new(r".\src\", "src", None, 60)
            .with_kind(SuggestionKind::Directory)
            .with_shell_range(),
        Suggestion::new(r".\srv.txt", "srv.txt", None, 50)
            .with_kind(SuggestionKind::File)
            .with_shell_range(),
    ];
    let names: Vec<String> = merge_suggestions("./sr", external, shell)
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, vec!["src/".to_string(), r".\srv.txt".to_string()]);
}

#[test]
fn test_plan_replacement_uses_shell_range_or_raw_token() {
    // PowerShell's range keeps the `[` in front of the type name. No match is reported here, so
    // there is no result type either: only the range mechanics apply.
    let r = report("[System.IO.Fi", 13, 1, 12, vec![]);
    let shell = Suggestion::new("System.IO.File", "File", None, 70).with_shell_range();
    assert_eq!(
        plan_replacement(&r, &shell),
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "le ".into()
        }
    );

    // Cursor in the middle of the token: the rest of the token is deleted forwards.
    let r = report("Get-ChildItem", 9, 0, 13, vec![]);
    let shell = Suggestion::new("Get-ChildItem", "Get-ChildItem", None, 80).with_shell_range();
    assert_eq!(
        plan_replacement(&r, &shell),
        ReplacementAction {
            backspace_count: 9,
            delete_count: 4,
            insert_text: "Get-ChildItem ".into()
        }
    );

    // Spec/carapace/zoxide suggestions replace the raw token before the cursor, quotes included.
    let r = report("cd 'My Do", 9, 3, 6, vec![]);
    let zoxide = Suggestion::new(r"'C:\My Documents'", r"C:\My Documents", None, 70);
    assert_eq!(
        plan_replacement(&r, &zoxide),
        ReplacementAction {
            backspace_count: 6,
            delete_count: 0,
            insert_text: r"'C:\My Documents' ".into()
        }
    );
}

#[test]
fn test_plan_replacement_closes_a_type_literal() {
    let type_match = vec![m("System.IO.File", "File", "Type", "System.IO.File")];
    let sug = || Suggestion::new("System.IO.File", "File", None, 70).with_shell_range();

    // PowerShell never supplies the `]`: `[System.IO.Fi` must become `[System.IO.File]`.
    let r = report("[System.IO.Fi", 13, 1, 12, type_match.clone());
    assert_eq!(
        plan_replacement(&r, &sug()),
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "le]".into()
        }
    );

    // A bracket that is already closed is left alone.
    let r = report("[System.IO.Fi]", 13, 1, 12, type_match.clone());
    assert_eq!(plan_replacement(&r, &sug()).insert_text, "le ");

    // Outside a type literal the type name is an ordinary argument.
    let r = report("New-Object System.IO.Fi", 23, 11, 12, type_match.clone());
    assert_eq!(plan_replacement(&r, &sug()).insert_text, "le ");

    // A namespace is a step on the way to a type: no bracket and no space, the user keeps typing.
    let r = report(
        "[Sys",
        4,
        1,
        3,
        vec![m("System", "System", "Namespace", "")],
    );
    let namespace = Suggestion::new("System", "System", None, 70).with_shell_range();
    assert_eq!(plan_replacement(&r, &namespace).insert_text, "tem");
}

#[test]
fn test_plan_replacement_survives_hostile_reports() {
    let sug = Suggestion::new("status", "status", None, 80).with_shell_range();

    // A negative replacement index is no range at all: fall back to the token before the cursor.
    assert_eq!(
        plan_replacement(&report("git sta", 7, -1, 3, vec![]), &sug),
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "tus ".into()
        }
    );

    // A cursor past the end of the line: the whole line counts as the text before it.
    assert_eq!(
        plan_replacement(&report("git sta", 99, 4, 3, vec![]), &sug),
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "tus ".into()
        }
    );

    // A cursor inside a surrogate pair ("😀" is two UTF-16 units): no byte index, so no range.
    assert_eq!(
        plan_replacement(&report("a😀", 2, 0, 3, vec![]), &sug),
        ReplacementAction {
            backspace_count: 3, // "a" plus the two units of the emoji
            delete_count: 0,
            insert_text: "status ".into()
        }
    );

    // A range that ends before the cursor cannot be applied with Backspace and Delete:
    // the token before the cursor already is the suggestion, so only the space is typed.
    assert_eq!(
        plan_replacement(&report("git status", 10, 0, 3, vec![]), &sug),
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: " ".into()
        }
    );

    // Absurd indices must not overflow or panic either.
    let r = report("git", 3, i64::MAX, i64::MAX, vec![]);
    assert_eq!(plan_replacement(&r, &sug).insert_text, "status ");
}

#[test]
fn test_shell_suggestions_drop_control_characters() {
    // A multi-line history entry (`#text<Tab>`) would type an Enter and run a half-inserted line.
    let sugs = shell_suggestions(&report(
        "#foo",
        4,
        0,
        4,
        vec![
            m("git status\r\nrm -rf /", "git status…", "Other", ""),
            m("git\tstatus", "git status", "Other", ""),
            m("esc\x1b[1m", "esc", "Other", ""),
            m("del\x7f", "del", "Other", ""),
            m("git status", "git status", "Other", ""),
        ],
    ));
    let names: Vec<&str> = sugs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["git status"]);
}

#[test]
fn test_merge_suggestions_drop_control_characters() {
    // No source may bypass the filter, not even an external provider.
    let external = vec![
        Suggestion::new("safe", "safe", None, 80).with_kind(SuggestionKind::Subcommand),
        Suggestion::new("bad\r\n", "bad", None, 90).with_kind(SuggestionKind::Subcommand),
    ];
    let shell = vec![
        Suggestion::new("also\nbad", "also bad", None, 85).with_shell_range(),
        Suggestion::new("fine", "fine", None, 70).with_shell_range(),
    ];
    let names: Vec<String> = merge_suggestions("sa", external, shell)
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, vec!["safe".to_string(), "fine".to_string()]);
}

#[tokio::test]
async fn test_engine_merges_spec_with_shell_matches_off_thread() {
    let mut specs = JsonSpecProvider::new();
    specs.add_spec(FigSpec {
        name: "mytool".into(),
        subcommands: vec![FigSubcommand {
            name: "deploy".into(),
            ..Default::default()
        }],
        ..Default::default()
    });
    let engine = CompletionEngine::new(specs);
    let r = report(
        "mytool de",
        9,
        7,
        2,
        vec![m(r".\demo.txt", "demo.txt", "ProviderItem", "")],
    );

    // Must be usable from a spawned task (Send + 'static).
    let sugs = tokio::spawn(async move { engine.complete(&r, "").await })
        .await
        .unwrap();
    let names: Vec<&str> = sugs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["deploy"]);
}

#[test]
fn test_external_suggestion_mid_word_replaces_the_whole_word() {
    // `git chec|kout`: the spec suggests `checkout`.
    let r = report("git checkout", 8, 4, 8, vec![]);
    let spec = Suggestion::new("checkout", "checkout", None, 90);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 4,
            delete_count: 4,
            insert_text: "checkout ".into()
        }
    );

    // The tail ends at the next word: `git chec|kout --quiet`.
    let r = report("git checkout --quiet", 8, 4, 8, vec![]);
    assert_eq!(plan_replacement(&r, &spec).delete_count, 4);

    // At the end of the word nothing after the cursor is deleted.
    let r = report("git chec", 8, 4, 4, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "kout ".into()
        }
    );
}

#[test]
fn test_external_suggestion_tail_stops_before_a_line_continuation() {
    // `git checkout ma|` + backtick + newline + `  --quiet`: the continuation must survive.
    let r = report(
        "git checkout ma`
  --quiet",
        15,
        13,
        2,
        vec![],
    );
    let spec = Suggestion::new("main", "main", None, 90);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: "in ".into()
        }
    );
}

#[test]
fn test_external_suggestion_tail_stops_before_a_redirection() {
    // `git log --onel|xx>log.txt`: only `xx` is part of the word, the redirection survives.
    let spec = Suggestion::new("--oneline", "--oneline", None, 90);
    let r = report("git log --onelxx>log.txt", 14, 8, 6, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 6,
            delete_count: 2,
            insert_text: "--oneline ".into()
        }
    );
}

#[test]
fn test_external_suggestion_tail_stops_before_a_closing_bracket() {
    // `(cd pro|jects)`: the closing parenthesis must survive.
    let r = report("(cd projects)", 7, 4, 3, vec![]);
    let spec = Suggestion::new("projects", "projects", None, 70);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 3,
            delete_count: 5,
            insert_text: "projects ".into()
        }
    );
}

#[test]
fn test_external_suggestion_inside_quotes_deletes_the_closing_quote() {
    let spec = Suggestion::new(r"'C:\My Documents'", r"C:\My Documents", None, 70);
    // `cd 'My Do|cuments'`: the tail `cuments'` goes with the opening quote.
    let r = report("cd 'My Documents'", 9, 3, 6, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 6,
            delete_count: 8,
            insert_text: r"'C:\My Documents' ".into()
        }
    );
    // The same inside double quotes.
    let spec = Suggestion::new(r#""C:\My Documents""#, r"C:\My Documents", None, 70);
    let r = report(r#"cd "My Documents""#, 9, 3, 6, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 6,
            delete_count: 8,
            insert_text: r#""C:\My Documents" "#.into()
        }
    );
}

#[test]
fn test_plan_replacement_survives_a_range_after_the_cursor() {
    let r = report("abcdef", 2, 3, 2, vec![]);
    let shell = Suggestion::new("xyz", "xyz", None, 70).with_shell_range();
    let _ = plan_replacement(&r, &shell); // must not panic
}

#[test]
fn test_merge_tie_keeps_the_external_item_and_drops_the_shell_duplicate() {
    // M8: same priority and name, so the sort cannot separate them; the stable order keeps the
    // external suggestion (with its description and without the shell range).
    let external = vec![Suggestion::new("main", "main", Some("carapace".into()), 70)];
    let shell = vec![Suggestion::new("main", "main", None, 70).with_shell_range()];
    let merged = merge_suggestions("ma", external, shell);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].description.as_deref(), Some("carapace"));
    assert!(!merged[0].uses_shell_range);
}

#[test]
fn test_merge_dedupes_a_quoted_external_path_against_the_shell_one() {
    // M9: carapace says `'My Dir/'`, PowerShell says `'.\My Dir\'`: one entry.
    let external = vec![
        Suggestion::new("'My Dir/'", "My Dir/", None, 70).with_kind(SuggestionKind::Directory)
    ];
    let shell = vec![Suggestion::new(r"'.\My Dir\'", "My Dir", None, 60)
        .with_kind(SuggestionKind::Directory)
        .with_shell_range()];
    let merged = merge_suggestions("My", external, shell);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].name, "'My Dir/'");
}

#[test]
fn test_open_quote_in_the_middle_of_a_word_is_detected_with_powershell_rules() {
    // `cd C:\'My Do|cs'`: the quote does not start the word, but it is open at the cursor.
    let spec = Suggestion::new(r"'C:\My Documents'", r"C:\My Documents", None, 70);
    let r = report(r"cd C:\'My Docs'", 12, 0, 0, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 9,
            delete_count: 3,
            insert_text: r"'C:\My Documents' ".into()
        }
    );
    // `cd 'it''s|x'`: the doubled quote is an escape, so the string is still open.
    let spec = Suggestion::new("'it''s dir'", "it's dir", None, 70);
    let r = report("cd 'it''sx'", 9, 0, 0, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 6,
            delete_count: 2,
            insert_text: "'it''s dir' ".into()
        }
    );
}

#[test]
fn test_unterminated_quote_swallows_the_rest_of_the_line() {
    // Closed without change: in PowerShell's grammar everything after an unclosed quote is
    // inside the string, so the tail is the whole rest of the line.
    let spec = Suggestion::new("'abcdef'", "abcdef", None, 70);
    let r = report("cd 'abcdef ghi", 7, 0, 0, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 4,
            delete_count: 7,
            insert_text: "'abcdef' ".into()
        }
    );
}

#[test]
fn test_no_double_space_when_a_space_already_follows_the_replaced_range() {
    let spec = Suggestion::new(r"'C:\My Documents'", r"C:\My Documents", None, 70);
    // `cd 'My Do|cuments' | sort`
    let r = report("cd 'My Documents' | sort", 9, 0, 0, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 6,
            delete_count: 8,
            insert_text: r"'C:\My Documents'".into()
        }
    );
    // A tab counts too: `git chec|kout<TAB>--quiet`.
    let spec = Suggestion::new("checkout", "checkout", None, 90);
    let r = report("git checkout\t--quiet", 8, 0, 0, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 4,
            delete_count: 4,
            insert_text: "checkout".into()
        }
    );
    // The shell-range branch: `Get-Ch|ildItem | sort`.
    let shell = Suggestion::new("Get-ChildItem", "Get-ChildItem", None, 80).with_shell_range();
    let r = report("Get-ChildItem | sort", 6, 0, 13, vec![]);
    assert_eq!(
        plan_replacement(&r, &shell),
        ReplacementAction {
            backspace_count: 6,
            delete_count: 7,
            insert_text: "Get-ChildItem".into()
        }
    );
    // Without anything after the word the space stays.
    let r = report("Get-ChildItem", 6, 0, 13, vec![]);
    assert_eq!(plan_replacement(&r, &shell).insert_text, "Get-ChildItem ");
}

#[test]
fn test_delete_count_is_in_utf16_units_for_a_multibyte_tail() {
    // `cd ab|😀c`: the emoji costs two Delete keys.
    let spec = Suggestion::new("abxyz", "abxyz", None, 70);
    let r = report("cd ab😀c", 5, 0, 0, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 2,
            delete_count: 3,
            insert_text: "abxyz ".into()
        }
    );
}

#[test]
fn test_quoted_suggestion_replaces_an_unquoted_word_around_the_cursor() {
    // `cd Proje|cts-new`: the whole word goes, the quoted zoxide path comes in.
    let spec = Suggestion::new("'Projects new'", "Projects new", None, 70);
    let r = report("cd Projects-new", 8, 0, 0, vec![]);
    assert_eq!(
        plan_replacement(&r, &spec),
        ReplacementAction {
            backspace_count: 5,
            delete_count: 7,
            insert_text: "'Projects new' ".into()
        }
    );
}
