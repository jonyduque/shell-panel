use shell_panel::engine::lexer::{active_token_raw, lex_command_line, CommandToken};

#[test]
fn test_lex_command_line_basic() {
    let tokens = lex_command_line("git commit -m \"feat: test\"");
    assert_eq!(tokens.len(), 4);
    assert_eq!(
        tokens[0],
        CommandToken {
            text: "git".to_string(),
            width: 3,
            complete: true,
            is_option: false,
        }
    );
    assert_eq!(
        tokens[1],
        CommandToken {
            text: "commit".to_string(),
            width: 6,
            complete: true,
            is_option: false,
        }
    );
    assert_eq!(
        tokens[2],
        CommandToken {
            text: "-m".to_string(),
            width: 2,
            complete: true,
            is_option: true,
        }
    );
    // Closed quote must be marked complete: true
    assert_eq!(
        tokens[3],
        CommandToken {
            text: "feat: test".to_string(),
            width: 10,
            complete: true,
            is_option: false,
        }
    );
}

#[test]
fn test_lex_command_line_unclosed_quote() {
    let tokens = lex_command_line("git commit -m \"feat: test");
    assert_eq!(tokens.len(), 4);
    // Unclosed quote must be complete: false
    assert!(!tokens[3].complete);
}

#[test]
fn test_lex_command_line_incomplete_last_token() {
    let tokens = lex_command_line("git stat");
    assert_eq!(tokens.len(), 2);
    assert_eq!(
        tokens[0],
        CommandToken {
            text: "git".to_string(),
            width: 3,
            complete: true,
            is_option: false,
        }
    );
    assert_eq!(
        tokens[1],
        CommandToken {
            text: "stat".to_string(),
            width: 4,
            complete: false,
            is_option: false,
        }
    );
}

#[test]
fn test_lex_command_line_trailing_space() {
    let tokens = lex_command_line("cargo ");
    assert_eq!(tokens.len(), 2);
    assert_eq!(
        tokens[0],
        CommandToken {
            text: "cargo".to_string(),
            width: 5,
            complete: true,
            is_option: false,
        }
    );
    assert_eq!(
        tokens[1],
        CommandToken {
            text: "".to_string(),
            width: 0,
            complete: false,
            is_option: false,
        }
    );
}

#[test]
fn test_lex_command_line_flag_with_equals() {
    let tokens = lex_command_line("docker run --name=my-container");
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0].text, "docker");
    assert_eq!(tokens[1].text, "run");
    assert_eq!(tokens[2].text, "--name");
    assert!(tokens[2].is_option);
    assert!(tokens[2].complete);
    assert_eq!(tokens[3].text, "my-container");
    assert!(!tokens[3].complete);
}

#[test]
fn test_lex_command_line_quotes_and_escapes() {
    let tokens = lex_command_line(r#"echo "hello`"world""#);
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1].text, "hello\"world");
    assert!(tokens[1].complete);

    let tokens2 = lex_command_line("echo 'literal $var'");
    assert_eq!(tokens2.len(), 2);
    assert_eq!(tokens2[1].text, "literal $var");
    assert!(tokens2[1].complete);

    let tokens3 = lex_command_line("echo 'don''t'");
    assert_eq!(tokens3.len(), 2);
    assert_eq!(tokens3[1].text, "don't");
    assert!(tokens3[1].complete);
}

#[test]
fn test_lex_command_line_pipeline_and_delimiters() {
    // Only the last command segment after pipe is lexed
    let tokens = lex_command_line("cat file.txt | grep -i pattern");
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].text, "grep");
    assert_eq!(tokens[1].text, "-i");
    assert_eq!(tokens[2].text, "pattern");
}

#[test]
fn test_lex_command_line_trailing_delimiter() {
    // Pipeline with trailing space: ready for command after pipe!
    let tokens = lex_command_line("cat file.txt | ");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, "");
    assert!(!tokens[0].complete);

    // Semicolon with trailing space
    let tokens2 = lex_command_line("echo foo; ");
    assert_eq!(tokens2.len(), 1);
    assert_eq!(tokens2[0].text, "");
    assert!(!tokens2[0].complete);
}

#[test]
fn test_lex_command_line_delimiters_inside_quotes() {
    let tokens = lex_command_line("grep \"foo | bar; baz && qux || done\"");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].text, "grep");
    assert_eq!(tokens[1].text, "foo | bar; baz && qux || done");
}

#[test]
fn test_lex_command_line_empty() {
    assert_eq!(lex_command_line(""), Vec::<CommandToken>::new());
    assert_eq!(lex_command_line("   "), Vec::<CommandToken>::new());
}

#[test]
fn test_lex_command_line_unicode_width() {
    let tokens = lex_command_line("echo 你好 🚀");
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[1].text, "你好");
    assert_eq!(tokens[1].width, 4); // CJK characters width = 2 each
    assert_eq!(tokens[2].text, "🚀");
    assert_eq!(tokens[2].width, 2); // Emoji width = 2
}

#[test]
fn test_active_token_raw_keeps_quotes_and_escapes() {
    assert_eq!(active_token_raw("git sta"), "sta");
    assert_eq!(active_token_raw("cd 'My Do"), "'My Do");
    assert_eq!(active_token_raw("git commit -m \"hello wor"), "\"hello wor");
    assert_eq!(active_token_raw("ls "), "");
    assert_eq!(active_token_raw("a | b"), "b");
    assert_eq!(active_token_raw("cmd --name=va"), "va");
    assert_eq!(active_token_raw("echo a` b"), "a` b");
}

#[test]
fn test_active_token_raw_stops_at_line_breaks() {
    // A multi-line PSReadLine buffer: the token must not span the previous physical line,
    // otherwise the planned backspaces would erase across the newline.
    assert_eq!(active_token_raw("if ($x) {\nGet-Ch"), "Get-Ch");
    assert_eq!(active_token_raw("git commit\r\nsr"), "sr");
    assert_eq!(active_token_raw("echo hi\n"), "");
}

fn texts(input: &str) -> Vec<String> {
    lex_command_line(input)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

#[test]
fn test_command_inside_script_block_or_parentheses() {
    assert_eq!(texts("if ($x) { git sta"), vec!["git", "sta"]);
    assert_eq!(texts("$r = (git sta"), vec!["git", "sta"]);
    assert_eq!(
        texts("foreach ($f in $files) { docker "),
        vec!["docker", ""]
    );
    // Quoted braces are text.
    assert_eq!(texts("echo '{ git' sta"), vec!["echo", "{ git", "sta"]);
}

#[test]
fn test_call_operators_are_not_the_command() {
    assert_eq!(texts("& git sta"), vec!["git", "sta"]);
    assert_eq!(texts(". git sta"), vec!["git", "sta"]);
    assert_eq!(
        texts("& 'C:\\Program Files\\Git\\cmd\\git.exe' sta"),
        vec!["C:\\Program Files\\Git\\cmd\\git.exe", "sta"]
    );
    assert_eq!(texts("./build.ps1 -Fa"), vec!["./build.ps1", "-Fa"]);
}

#[test]
fn test_line_breaks_separate_commands() {
    assert_eq!(texts("if ($x) {\n  git sta"), vec!["git", "sta"]);
    assert_eq!(texts("Get-Location\ngit sta"), vec!["git", "sta"]);
    assert_eq!(texts("Get-Location\r\ngit sta"), vec!["git", "sta"]);
    assert_eq!(texts("git status\n"), vec![""]);
}

#[test]
fn test_single_ampersand_starts_a_command() {
    assert_eq!(texts("git status & git ch"), vec!["git", "ch"]);
    assert_eq!(texts("git status && git ch"), vec!["git", "ch"]);
    assert_eq!(active_token_raw("git status & git ch"), "ch");
}

#[test]
fn test_backtick_line_continuation_is_whitespace() {
    assert_eq!(texts("git status `\n  --sh"), vec!["git", "status", "--sh"]);
    assert_eq!(
        texts("git status `\r\n  --sh"),
        vec!["git", "status", "--sh"]
    );
}

#[test]
fn test_lexer_and_active_token_agree_on_separators() {
    for input in [
        "if ($x) {\n  git sta",
        "Get-Location\r\ngit sta",
        "git status & git ch",
        "a | b; c && git ch",
        "$r = (git sta",
    ] {
        let last = texts(input).pop().unwrap();
        assert_eq!(active_token_raw(input), last, "input: {input:?}");
    }
}
