use unicode_width::UnicodeWidthStr;

/// Represents a parsed token from a command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandToken {
    /// The string text of the token (unquoted/unescaped).
    pub text: String,
    /// Terminal display width of the token text.
    pub width: usize,
    /// Whether this token is complete (i.e. followed by a space, separator, or closed quote).
    pub complete: bool,
    /// Whether this token is an option/flag (starts with `-`).
    pub is_option: bool,
}

impl CommandToken {
    /// Creates a new `CommandToken` with calculated width.
    pub fn new(text: impl Into<String>, complete: bool, is_option: bool) -> Self {
        let text = text.into();
        let width = UnicodeWidthStr::width(text.as_str());
        Self {
            text,
            width,
            complete,
            is_option,
        }
    }
}

/// Internal struct representing a scanned raw token before completion state is finalized.
struct RawToken {
    text: String,
    is_option: bool,
    closed_quote: bool,
}

/// Lexes a PowerShell command line string into a list of `CommandToken`s.
///
/// Delimiters: `|`, `;`, `&&`, `||` outside quotes split the input into segments.
/// Only the active (last) command segment is lexed. If the input ends with a delimiter,
/// an empty token is returned for completing the next command.
pub fn lex_command_line(input: &str) -> Vec<CommandToken> {
    if input.trim().is_empty() {
        return Vec::new();
    }

    let segments = split_segments(input);
    if segments.is_empty() {
        return Vec::new();
    }

    let last_segment = *segments.last().unwrap_or(&"");

    // If input ends with a pipeline or delimiter (e.g. `cat file.txt | ` or `foo; `),
    // the active command segment is the empty/whitespace text after the delimiter.
    if last_segment.trim().is_empty() {
        if segments.len() > 1 {
            return vec![CommandToken::new("", false, false)];
        }
        return Vec::new();
    }

    let mut raw_tokens = lex_segment(last_segment);
    // `& cmd` and `. cmd` invoke `cmd`: the operator is not the command.
    if raw_tokens.len() > 1
        && !raw_tokens[0].closed_quote
        && (raw_tokens[0].text == "&" || raw_tokens[0].text == ".")
    {
        raw_tokens.remove(0);
    }
    if raw_tokens.is_empty() {
        return Vec::new();
    }

    let ends_with_space = last_segment.ends_with(' ') || last_segment.ends_with('\t');
    let mut tokens = Vec::with_capacity(raw_tokens.len() + if ends_with_space { 1 } else { 0 });

    let len = raw_tokens.len();
    for (i, raw) in raw_tokens.into_iter().enumerate() {
        let is_last = i == len - 1;
        let complete = if is_last {
            raw.closed_quote || ends_with_space
        } else {
            true
        };

        tokens.push(CommandToken::new(raw.text, complete, raw.is_option));
    }

    if ends_with_space {
        tokens.push(CommandToken::new("", false, false));
    }

    tokens
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum DelimQuoteState {
    Normal,
    SingleQuote,
    DoubleQuote,
}

/// Splits input into segments separated by unquoted `|`, `;`, `&&`, `||`.
fn split_segments(input: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut state = DelimQuoteState::Normal;

    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let (byte_pos, ch) = chars[i];
        match state {
            DelimQuoteState::Normal => {
                if ch == '\'' {
                    state = DelimQuoteState::SingleQuote;
                    i += 1;
                } else if ch == '"' {
                    state = DelimQuoteState::DoubleQuote;
                    i += 1;
                } else if ch == '`' {
                    // Backtick escapes next character outside quotes in PowerShell
                    i += 2;
                } else if ch == '|' {
                    if i + 1 < len && chars[i + 1].1 == '|' {
                        // || delimiter
                        segments.push(&input[start..byte_pos]);
                        i += 2;
                        start = if i < len { chars[i].0 } else { input.len() };
                    } else {
                        // | delimiter
                        segments.push(&input[start..byte_pos]);
                        i += 1;
                        start = if i < len { chars[i].0 } else { input.len() };
                    }
                } else if ch == ';' {
                    // ; delimiter
                    segments.push(&input[start..byte_pos]);
                    i += 1;
                    start = if i < len { chars[i].0 } else { input.len() };
                } else if ch == '&' && i + 1 < len && chars[i + 1].1 == '&' {
                    // && delimiter
                    segments.push(&input[start..byte_pos]);
                    i += 2;
                    start = if i < len { chars[i].0 } else { input.len() };
                } else if ch == '(' || ch == '{' {
                    // A sub-expression or script block starts a new command.
                    segments.push(&input[start..byte_pos]);
                    i += 1;
                    start = if i < len { chars[i].0 } else { input.len() };
                } else {
                    i += 1;
                }
            }
            DelimQuoteState::SingleQuote => {
                if ch == '\'' {
                    if i + 1 < len && chars[i + 1].1 == '\'' {
                        // Escaped single quote ''
                        i += 2;
                    } else {
                        state = DelimQuoteState::Normal;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            DelimQuoteState::DoubleQuote => {
                if ch == '`' {
                    // Backtick escapes next char inside double quotes
                    i += 2;
                } else if ch == '"' {
                    state = DelimQuoteState::Normal;
                    i += 1;
                } else {
                    i += 1;
                }
            }
        }
    }

    if start <= input.len() {
        segments.push(&input[start..]);
    }

    segments
}

/// Lexes a single command segment into raw tokens.
fn lex_segment(segment: &str) -> Vec<RawToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = segment.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip leading whitespace between tokens
        while i < len && (chars[i] == ' ' || chars[i] == '\t') {
            i += 1;
        }

        if i >= len {
            break;
        }

        let mut current = String::new();
        let mut token_active = false;
        let mut in_single = false;
        let mut in_double = false;
        let mut closed_quote = false;
        let mut is_flag_candidate = chars[i] == '-';

        while i < len {
            let c = chars[i];

            if in_single {
                if c == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        // Two consecutive single quotes inside single quotes -> single quote
                        current.push('\'');
                        i += 2;
                    } else {
                        // Closing single quote
                        in_single = false;
                        closed_quote = true;
                        i += 1;
                    }
                } else {
                    current.push(c);
                    i += 1;
                }
            } else if in_double {
                if c == '`' {
                    if i + 1 < len {
                        current.push(chars[i + 1]);
                        i += 2;
                    } else {
                        current.push('`');
                        i += 1;
                    }
                } else if c == '"' {
                    in_double = false;
                    closed_quote = true;
                    i += 1;
                } else {
                    current.push(c);
                    i += 1;
                }
            } else {
                // Outside quotes
                if c == '\'' {
                    token_active = true;
                    in_single = true;
                    i += 1;
                } else if c == '"' {
                    token_active = true;
                    in_double = true;
                    i += 1;
                } else if c == '`' {
                    token_active = true;
                    if i + 1 < len {
                        current.push(chars[i + 1]);
                        i += 2;
                    } else {
                        current.push('`');
                        i += 1;
                    }
                } else if c == ' ' || c == '\t' {
                    // Unquoted whitespace ends the current word
                    break;
                } else if c == '=' && is_flag_candidate {
                    // Flag with '=': split into flag part and argument value part
                    i += 1; // consume '='

                    let flag_text = std::mem::take(&mut current);
                    let is_option = flag_text.starts_with('-');
                    tokens.push(RawToken {
                        text: flag_text,
                        is_option,
                        closed_quote: false,
                    });

                    // The following part is an argument, not another flag
                    is_flag_candidate = false;
                    token_active = true;
                } else {
                    token_active = true;
                    current.push(c);
                    i += 1;
                }
            }
        }

        if token_active {
            let is_option = current.starts_with('-');
            tokens.push(RawToken {
                text: current,
                is_option,
                closed_quote,
            });
        }
    }

    tokens
}

/// Returns the raw source text (quotes and backtick escapes included) of the token that ends
/// `input`, i.e. exactly what has to be erased to replace it at the prompt.
pub fn active_token_raw(input: &str) -> &str {
    let mut start = 0;
    let mut state = DelimQuoteState::Normal;
    let mut chars = input.char_indices();

    while let Some((i, c)) = chars.next() {
        let next = i + c.len_utf8();
        match state {
            DelimQuoteState::Normal => match c {
                ' ' | '\t' | '\n' | '\r' | '|' | ';' | '&' | '(' | '{' => start = next,
                '\'' => state = DelimQuoteState::SingleQuote,
                '"' => state = DelimQuoteState::DoubleQuote,
                '`' => {
                    chars.next();
                }
                '=' if input[start..i].starts_with('-') => start = next,
                _ => {}
            },
            DelimQuoteState::SingleQuote => {
                if c == '\'' {
                    state = DelimQuoteState::Normal;
                }
            }
            DelimQuoteState::DoubleQuote => match c {
                '`' => {
                    chars.next();
                }
                '"' => state = DelimQuoteState::Normal,
                _ => {}
            },
        }
    }

    &input[start..]
}
