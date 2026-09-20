use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::engine::lexer::lex_command_line;
use crate::engine::provider::{CompletionProvider, Suggestion};

/// Represents an option/flag in a Fig-converted JSON spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FigOption {
    /// Option flag name(s), e.g. ["-m", "--message"] or ["-v"].
    #[serde(deserialize_with = "deserialize_name_vec")]
    pub name: Vec<String>,
    /// Optional description of the option.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional argument definition for the option.
    #[serde(default)]
    pub args: Option<serde_json::Value>,
}

/// Represents a subcommand in a Fig-converted JSON spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FigSubcommand {
    /// Subcommand name.
    #[serde(deserialize_with = "deserialize_single_string")]
    pub name: String,
    /// Optional description of the subcommand.
    #[serde(default)]
    pub description: Option<String>,
    /// Nested subcommands.
    #[serde(default)]
    pub subcommands: Vec<FigSubcommand>,
    /// Options available for this subcommand.
    #[serde(default)]
    pub options: Vec<FigOption>,
    /// Optional argument definition for this subcommand.
    #[serde(default)]
    pub args: Option<serde_json::Value>,
}

/// Root Fig CLI specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FigSpec {
    /// Root command name (e.g. "git").
    #[serde(deserialize_with = "deserialize_single_string")]
    pub name: String,
    /// Description of the root command.
    #[serde(default)]
    pub description: Option<String>,
    /// Root subcommands.
    #[serde(default)]
    pub subcommands: Vec<FigSubcommand>,
    /// Root options.
    #[serde(default)]
    pub options: Vec<FigOption>,
}

fn deserialize_name_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        Single(String),
        Multiple(Vec<String>),
    }

    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::Single(s) => Ok(vec![s]),
        StringOrVec::Multiple(v) => Ok(v),
    }
}

fn deserialize_single_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        Single(String),
        Multiple(Vec<String>),
    }

    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::Single(s) => Ok(s),
        StringOrVec::Multiple(mut v) => Ok(v.drain(..).next().unwrap_or_default()),
    }
}

/// Completion provider based on Fig JSON CLI specifications.
#[derive(Debug, Default, Clone)]
pub struct JsonSpecProvider {
    pub specs: HashMap<String, FigSpec>,
}

impl JsonSpecProvider {
    /// Creates an empty `JsonSpecProvider`.
    pub fn new() -> Self {
        Self {
            specs: HashMap::new(),
        }
    }

    /// Creates a `JsonSpecProvider` initialized with the given map of specifications.
    pub fn from_specs(specs: HashMap<String, FigSpec>) -> Self {
        Self { specs }
    }

    /// Adds a Fig specification to this provider.
    pub fn add_spec(&mut self, spec: FigSpec) {
        self.specs.insert(spec.name.clone(), spec);
    }
}

#[async_trait::async_trait]
impl CompletionProvider for JsonSpecProvider {
    fn name(&self) -> &'static str {
        "json_spec"
    }

    fn can_handle(&self, cmd: &str) -> bool {
        self.specs.contains_key(cmd)
            || self.specs.values().any(|s| s.name.eq_ignore_ascii_case(cmd))
    }

    async fn complete(&self, cmd_line: &str, _cwd: &str) -> Vec<Suggestion> {
        let tokens = lex_command_line(cmd_line);
        if tokens.is_empty() {
            return Vec::new();
        }

        let root_cmd = &tokens[0].text;
        let spec = match self
            .specs
            .get(root_cmd.as_str())
            .or_else(|| self.specs.values().find(|s| s.name.eq_ignore_ascii_case(root_cmd)))
        {
            Some(s) => s,
            None => return Vec::new(),
        };

        let mut curr_subcommands = &spec.subcommands;
        let mut curr_options = &spec.options;

        if tokens.len() == 1 {
            if !tokens[0].complete {
                return Vec::new();
            }
            // User typed only the root command
            let mut results = Vec::new();
            for subcmd in curr_subcommands {
                results.push(Suggestion::new(
                    &subcmd.name,
                    &subcmd.name,
                    subcmd.description.clone(),
                    80,
                ));
            }
            results.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.name.cmp(&b.name)));
            return results;
        }

        // Traverse intermediate subcommands (between root command and the active token)
        for token in &tokens[1..tokens.len() - 1] {
            if !token.is_option {
                if let Some(matching_sub) = curr_subcommands
                    .iter()
                    .find(|s| s.name.eq_ignore_ascii_case(&token.text))
                {
                    curr_subcommands = &matching_sub.subcommands;
                    curr_options = &matching_sub.options;
                }
            }
        }

        let active_token = &tokens[tokens.len() - 1];
        let active_text = &active_token.text;
        let active_lower = active_text.to_lowercase();
        let mut suggestions = Vec::new();

        if active_token.is_option || active_text.starts_with('-') {
            for opt in curr_options {
                for opt_name in &opt.name {
                    if opt_name.to_lowercase().starts_with(&active_lower) {
                        suggestions.push(Suggestion::new(
                            opt_name,
                            opt_name,
                            opt.description.clone(),
                            75,
                        ));
                    }
                }
            }
        } else {
            for subcmd in curr_subcommands {
                if subcmd.name.to_lowercase().starts_with(&active_lower) {
                    suggestions.push(Suggestion::new(
                        &subcmd.name,
                        &subcmd.name,
                        subcmd.description.clone(),
                        80,
                    ));
                }
            }
        }

        suggestions.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.name.cmp(&b.name)));
        suggestions
    }
}
