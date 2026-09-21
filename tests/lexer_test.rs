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
    assert_eq!(tokens[3].complete, false);
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
