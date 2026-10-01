use std::path::Path;
use tokio::process::Command;

use crate::engine::lexer::lex_command_line;
use crate::engine::provider::{CompletionProvider, Suggestion, SuggestionKind};

/// Completion provider that queries `zoxide` directory jump history.
#[derive(Debug, Clone)]
pub struct ZoxideProvider {
    pub binary_path: String,
}

impl Default for ZoxideProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ZoxideProvider {
    /// Creates a new `ZoxideProvider` with default `"zoxide"` binary.
    pub fn new() -> Self {
        Self {
            binary_path: "zoxide".to_string(),
        }
    }

    /// Creates a new `ZoxideProvider` with custom binary path.
    pub fn with_binary(binary_path: impl Into<String>) -> Self {
        Self {
            binary_path: binary_path.into(),
        }
    }
}

pub use crate::engine::quote::quote_for_powershell;

/// Parses newline-delimited directory paths from `zoxide query -l`, filtering by prefix.
pub fn parse_zoxide_output(output: &str, prefix: &str) -> Vec<Suggestion> {
    let prefix_clean = prefix.trim();
    let prefix_lower = prefix_clean.to_lowercase();
    let mut suggestions = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let matches = if prefix_clean.is_empty() {
            true
        } else {
            let trimmed_lower = trimmed.to_lowercase();
            if trimmed_lower.starts_with(&prefix_lower) {
                true
            } else if let Some(file_name) = Path::new(trimmed).file_name() {
                file_name
                    .to_string_lossy()
                    .to_lowercase()
                    .starts_with(&prefix_lower)
            } else {
                false
            }
        };

        if matches {
            suggestions.push(
                Suggestion::new(
                    quote_for_powershell(trimmed),
                    trimmed,
                    Some("Zoxide Directory".into()),
                    70,
                )
                .with_kind(SuggestionKind::Directory),
            );
        }
    }

    suggestions
}

#[async_trait::async_trait]
impl CompletionProvider for ZoxideProvider {
    fn name(&self) -> &'static str {
        "zoxide"
    }

    fn can_handle(&self, cmd: &str) -> bool {
        cmd.eq_ignore_ascii_case("cd")
            || cmd.eq_ignore_ascii_case("z")
            || cmd.eq_ignore_ascii_case("zi")
    }

    async fn complete(&self, cmd_line: &str, _cwd: &str) -> Vec<Suggestion> {
        let tokens = lex_command_line(cmd_line);
        if tokens.is_empty() || (tokens.len() == 1 && !tokens[0].complete) {
            return Vec::new();
        }
        let prefix = tokens.last().map(|t| t.text.as_str()).unwrap_or("");

        let query_future = Command::new(&self.binary_path)
            .arg("query")
            .arg("-l")
            .kill_on_drop(true)
            .output();

        let output =
            match tokio::time::timeout(std::time::Duration::from_millis(150), query_future).await {
                Ok(Ok(out)) if out.status.success() => out,
                _ => return Vec::new(),
            };

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        parse_zoxide_output(&stdout_str, prefix)
    }
}
