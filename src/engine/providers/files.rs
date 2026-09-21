use std::path::{Path, PathBuf};

use crate::engine::lexer::lex_command_line;
use crate::engine::provider::{CompletionProvider, Suggestion, SuggestionKind};

/// File and directory completion provider (filesystem fallback).
#[derive(Debug, Default, Clone)]
pub struct FileProvider;

impl FileProvider {
    /// Creates a new `FileProvider`.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl CompletionProvider for FileProvider {
    fn name(&self) -> &'static str {
        "files"
    }

    fn can_handle(&self, _cmd: &str) -> bool {
        true
    }

    async fn complete(&self, cmd_line: &str, cwd: &str) -> Vec<Suggestion> {
        let tokens = lex_command_line(cmd_line);
        let active_token = tokens.last().map(|t| t.text.as_str()).unwrap_or("");

        let (dir_prefix, file_prefix) = match active_token.rfind(|c| c == '/' || c == '\\') {
            Some(idx) => (&active_token[..=idx], &active_token[idx + 1..]),
            None => ("", active_token),
        };

        let target_dir = if dir_prefix.is_empty() {
            PathBuf::from(cwd)
        } else {
            let p = Path::new(dir_prefix);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                Path::new(cwd).join(p)
            }
        };

        let read_dir = match std::fs::read_dir(&target_dir) {
            Ok(rd) => rd,
            Err(_) => return Vec::new(),
        };

        let mut suggestions = Vec::new();
        let file_prefix_lower = file_prefix.to_lowercase();

        for entry in read_dir.flatten() {
            let file_name_os = entry.file_name();
            let file_name = file_name_os.to_string_lossy();

            if !file_prefix_lower.is_empty()
                && !file_name.to_lowercase().starts_with(&file_prefix_lower)
            {
                continue;
            }

            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            if is_dir {
                let name = format!("{}{}/", dir_prefix, file_name);
                let display = format!("{}{}/", dir_prefix, file_name);
                suggestions.push(
                    Suggestion::new(name, display, Some("Directory".into()), 60)
                        .with_kind(SuggestionKind::Directory),
                );
            } else {
                let name = format!("{}{}", dir_prefix, file_name);
                let display = format!("{}{}", dir_prefix, file_name);
                suggestions
                    .push(Suggestion::new(name, display, None, 50).with_kind(SuggestionKind::File));
            }
        }

        suggestions.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        suggestions
    }
}
