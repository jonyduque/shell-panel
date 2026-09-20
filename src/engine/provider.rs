use serde::{Deserialize, Serialize};

/// Represents a completion candidate shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    /// The actual text value to be inserted/completed.
    pub name: String,
    /// The display label shown in the dropdown panel.
    pub display: String,
    /// Optional documentation/description explaining the suggestion.
    pub description: Option<String>,
    /// Priority score for ranking suggestions (higher values rank first).
    pub priority: u32,
}

impl Suggestion {
    /// Creates a new `Suggestion` instance.
    pub fn new(
        name: impl Into<String>,
        display: impl Into<String>,
        description: Option<String>,
        priority: u32,
    ) -> Self {
        Self {
            name: name.into(),
            display: display.into(),
            description,
            priority,
        }
    }
}

/// Trait defining a completion provider capable of suggesting completions.
#[async_trait::async_trait]
pub trait CompletionProvider: Send + Sync {
    /// Returns the unique name of this provider.
    fn name(&self) -> &'static str;

    /// Checks whether this provider can handle the given root command (e.g. "git", "cd").
    fn can_handle(&self, cmd: &str) -> bool;

    /// Produces a list of completion suggestions for the given command line and current working directory.
    async fn complete(&self, cmd_line: &str, cwd: &str) -> Vec<Suggestion>;
}
