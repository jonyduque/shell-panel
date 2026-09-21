use serde::Deserialize;

use crate::engine::replacement::utf16_to_byte_index;

/// One PowerShell completion result: completion text, list item text, result type, tooltip.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShellMatch(pub String, pub String, pub String, pub String);

/// What the PSReadLine handler reports: the edited line, the cursor and PowerShell's completions.
/// All indices are UTF-16 code units, the unit .NET strings use.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellReport {
    pub line: String,
    pub cursor: usize,
    #[serde(default)]
    pub replacement_index: i64,
    #[serde(default)]
    pub replacement_length: i64,
    #[serde(default)]
    pub matches: Vec<ShellMatch>,
}

impl ShellReport {
    /// Byte index of the cursor in `line`.
    pub fn cursor_byte(&self) -> Option<usize> {
        utf16_to_byte_index(&self.line, self.cursor)
    }

    /// The line up to the cursor.
    pub fn text_before_cursor(&self) -> Option<&str> {
        self.cursor_byte().map(|end| &self.line[..end])
    }

    /// Byte range PowerShell wants replaced. `None` when it is malformed or does not contain the
    /// cursor (it is applied with Backspace and Delete, which only work around the cursor).
    pub fn replacement_range(&self) -> Option<(usize, usize)> {
        let index = usize::try_from(self.replacement_index).ok()?;
        let length = usize::try_from(self.replacement_length).ok()?;
        let start = utf16_to_byte_index(&self.line, index)?;
        let end = utf16_to_byte_index(&self.line, index + length)?;
        let cursor = self.cursor_byte()?;
        (start <= cursor && cursor <= end).then_some((start, end))
    }
}
