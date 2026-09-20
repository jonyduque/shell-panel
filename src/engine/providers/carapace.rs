use serde::Deserialize;
use tokio::process::Command;

use crate::engine::lexer::lex_command_line;
use crate::engine::provider::{CompletionProvider, Suggestion};

/// Represents an entry in Carapace's export JSON format.
#[derive(Debug, Deserialize)]
struct CarapaceItem {
    #[serde(default)]
    value: String,
    #[serde(default)]
    display: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    style: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CarapaceFormat {
    Array(Vec<CarapaceItem>),
    Object {
        #[serde(default)]
        values: Vec<CarapaceItem>,
    },
}

/// Completion provider that queries the external `carapace` CLI.
#[derive(Debug, Clone)]
pub struct CarapaceProvider {
    pub binary_path: String,
}

impl Default for CarapaceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CarapaceProvider {
    /// Creates a new `CarapaceProvider` using default `"carapace"` binary.
    pub fn new() -> Self {
        Self {
            binary_path: "carapace".to_string(),
        }
    }

    /// Creates a new `CarapaceProvider` with a custom binary path.
    pub fn with_binary(binary_path: impl Into<String>) -> Self {
        Self {
            binary_path: binary_path.into(),
        }
    }
}

/// Parses Carapace JSON export output into a list of `Suggestion`s.
pub fn parse_carapace_json(json_str: &str) -> Vec<Suggestion> {
    let items: Vec<CarapaceItem> = match serde_json::from_str::<CarapaceFormat>(json_str) {
        Ok(CarapaceFormat::Array(arr)) => arr,
        Ok(CarapaceFormat::Object { values }) => values,
        Err(_) => return Vec::new(),
    };

    items
        .into_iter()
        .filter(|item| !item.value.is_empty())
        .map(|item| {
            let display = item
                .display
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| item.value.clone());
            let description = item.description.filter(|d| !d.is_empty());
            Suggestion::new(item.value, display, description, 70)
        })
        .collect()
}

#[async_trait::async_trait]
impl CompletionProvider for CarapaceProvider {
    fn name(&self) -> &'static str {
        "carapace"
    }

    fn can_handle(&self, cmd: &str) -> bool {
        !cmd.trim().is_empty()
    }

    async fn complete(&self, cmd_line: &str, cwd: &str) -> Vec<Suggestion> {
        let tokens = lex_command_line(cmd_line);
        if tokens.is_empty() {
            return Vec::new();
        }

        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("_carapace").arg("export");
        if !cwd.is_empty() {
            cmd.current_dir(cwd);
        }

        for token in &tokens {
            cmd.arg(&token.text);
        }

        let output = match tokio::time::timeout(std::time::Duration::from_millis(200), cmd.output()).await {
            Ok(Ok(out)) if out.status.success() => out,
            _ => return Vec::new(),
        };

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        parse_carapace_json(&stdout_str)
    }
}
