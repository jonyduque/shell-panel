use serde::{Deserialize, Serialize};

/// Category or kind of a completion suggestion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SuggestionKind {
    Directory,
    File,
    Command,
    Subcommand,
    Option,
    PowerShellCmdlet,
    Alias,
    #[default]
    Other,
}

impl SuggestionKind {
    /// Returns the UI icon associated with this suggestion kind.
    pub fn icon(&self) -> &'static str {
        match self {
            SuggestionKind::Directory => "📁 ",
            SuggestionKind::File => "📄 ",
            SuggestionKind::Command => "⚡ ",
            SuggestionKind::Subcommand => "🔹 ",
            SuggestionKind::Option => "🏷️  ",
            SuggestionKind::PowerShellCmdlet => ">_ ",
            SuggestionKind::Alias => "🔗 ",
            SuggestionKind::Other => "  ",
        }
    }
}

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
    /// The category/kind of suggestion (e.g. file, directory, subcommand).
    pub kind: SuggestionKind,
}

impl Suggestion {
    /// Creates a new `Suggestion` instance with default `Other` kind.
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
            kind: SuggestionKind::Other,
        }
    }

    /// Builder method to specify the `SuggestionKind`.
    pub fn with_kind(mut self, kind: SuggestionKind) -> Self {
        self.kind = kind;
        self
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
