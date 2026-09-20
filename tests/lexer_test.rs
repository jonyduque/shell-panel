use shell_panel::engine::lexer::{lex_command_line, CommandToken};

#[test]
fn test_basic_command() {
    let input = r#"git commit -m "feat: test""#;
    let tokens = lex_command_line(input);

    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "git".to_string(),
                width: 3,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "commit".to_string(),
                width: 6,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "-m".to_string(),
                width: 2,
                complete: true,
                is_option: true,
            },
            CommandToken {
                text: "feat: test".to_string(),
                width: 10,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_incomplete_token() {
    let input = "git stat";
    let tokens = lex_command_line(input);

    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "git".to_string(),
                width: 3,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "stat".to_string(),
                width: 4,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_trailing_space() {
    let input = "cargo ";
    let tokens = lex_command_line(input);

    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "cargo".to_string(),
                width: 5,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "".to_string(),
                width: 0,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_trailing_multiple_spaces() {
    let input = "cargo   ";
    let tokens = lex_command_line(input);

    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "cargo".to_string(),
                width: 5,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "".to_string(),
                width: 0,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_flag_with_equals() {
    let input = "docker run --name=my-container";
    let tokens = lex_command_line(input);

    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "docker".to_string(),
                width: 6,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "run".to_string(),
                width: 3,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "--name".to_string(),
                width: 6,
                complete: true,
                is_option: true,
            },
            CommandToken {
                text: "my-container".to_string(),
                width: 12,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_flag_with_equals_and_trailing_space() {
    let input = "docker run --name=my-container ";
    let tokens = lex_command_line(input);

    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "docker".to_string(),
                width: 6,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "run".to_string(),
                width: 3,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "--name".to_string(),
                width: 6,
                complete: true,
                is_option: true,
            },
            CommandToken {
                text: "my-container".to_string(),
                width: 12,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "".to_string(),
                width: 0,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_flag_with_equals_empty_value() {
    let input = "docker run --name=";
    let tokens = lex_command_line(input);

    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "docker".to_string(),
                width: 6,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "run".to_string(),
                width: 3,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "--name".to_string(),
                width: 6,
                complete: true,
                is_option: true,
            },
            CommandToken {
                text: "".to_string(),
                width: 0,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_quotes_and_escapes() {
    let input1 = "hello`\"world";
    let tokens1 = lex_command_line(input1);
    assert_eq!(
        tokens1,
        vec![CommandToken {
            text: "hello\"world".to_string(),
            width: 11,
            complete: false,
            is_option: false,
        }]
    );

    let input1_quoted = "\"hello`\"world\"";
    let tokens1_quoted = lex_command_line(input1_quoted);
    assert_eq!(
        tokens1_quoted,
        vec![CommandToken {
            text: "hello\"world".to_string(),
            width: 11,
            complete: false,
            is_option: false,
        }]
    );

    let input2 = "'literal $var'";
    let tokens2 = lex_command_line(input2);
    assert_eq!(
        tokens2,
        vec![CommandToken {
            text: "literal $var".to_string(),
            width: 12,
            complete: false,
            is_option: false,
        }]
    );

    let input3 = "'don''t'";
    let tokens3 = lex_command_line(input3);
    assert_eq!(
        tokens3,
        vec![CommandToken {
            text: "don't".to_string(),
            width: 5,
            complete: false,
            is_option: false,
        }]
    );
}

#[test]
fn test_pipeline_handling() {
    let input = "cat file.txt | grep -i pattern";
    let tokens = lex_command_line(input);

    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "grep".to_string(),
                width: 4,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "-i".to_string(),
                width: 2,
                complete: true,
                is_option: true,
            },
            CommandToken {
                text: "pattern".to_string(),
                width: 7,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_delimiters_semicolon_and_logical() {
    let input_semi = "echo first; cargo build";
    let tokens_semi = lex_command_line(input_semi);
    assert_eq!(
        tokens_semi,
        vec![
            CommandToken {
                text: "cargo".to_string(),
                width: 5,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "build".to_string(),
                width: 5,
                complete: false,
                is_option: false,
            },
        ]
    );

    let input_and = "test && git push";
    let tokens_and = lex_command_line(input_and);
    assert_eq!(
        tokens_and,
        vec![
            CommandToken {
                text: "git".to_string(),
                width: 3,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "push".to_string(),
                width: 4,
                complete: false,
                is_option: false,
            },
        ]
    );

    let input_or = "test || echo failed";
    let tokens_or = lex_command_line(input_or);
    assert_eq!(
        tokens_or,
        vec![
            CommandToken {
                text: "echo".to_string(),
                width: 4,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "failed".to_string(),
                width: 6,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_delimiters_inside_quotes() {
    let input = "grep \"foo | bar; baz && qux || done\"";
    let tokens = lex_command_line(input);
    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "grep".to_string(),
                width: 4,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "foo | bar; baz && qux || done".to_string(),
                width: 29,
                complete: false,
                is_option: false,
            },
        ]
    );
}

#[test]
fn test_empty_and_whitespace_input() {
    assert_eq!(lex_command_line(""), vec![]);
    assert_eq!(lex_command_line("   "), vec![]);
    assert_eq!(lex_command_line("|"), vec![]);
    assert_eq!(lex_command_line(";"), vec![]);
}

#[test]
fn test_unicode_width() {
    let input = "echo 日本語";
    let tokens = lex_command_line(input);
    assert_eq!(
        tokens,
        vec![
            CommandToken {
                text: "echo".to_string(),
                width: 4,
                complete: true,
                is_option: false,
            },
            CommandToken {
                text: "日本語".to_string(),
                width: 6,
                complete: false,
                is_option: false,
            },
        ]
    );
}
