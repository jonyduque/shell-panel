/// Keys that turn the text around the cursor into the chosen suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementAction {
    /// Backspaces to erase text before the cursor.
    pub backspace_count: usize,
    /// Forward deletes to erase text after the cursor.
    pub delete_count: usize,
    /// Text to type, including any trailing space.
    pub insert_text: String,
}

impl ReplacementAction {
    /// The byte sequence to write to the PTY.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0x7f; self.backspace_count];
        for _ in 0..self.delete_count {
            bytes.extend_from_slice(b"\x1b[3~");
        }
        bytes.extend_from_slice(self.insert_text.as_bytes());
        bytes
    }
}

/// Directories (also when quoted) keep the cursor right behind them so the path can continue.
fn trailing_space(suggestion: &str) -> &'static str {
    let unquoted = suggestion.trim_end_matches(['\'', '"']);
    if unquoted.ends_with('/') || unquoted.ends_with('\\') {
        ""
    } else {
        " "
    }
}

/// Replaces `typed` (the text right before the cursor) with `suggestion`.
/// When `suggestion` extends `typed` only the missing suffix is typed.
pub fn calculate_replacement(typed: &str, suggestion: &str) -> ReplacementAction {
    let space = trailing_space(suggestion);
    if suggestion.starts_with(typed) {
        ReplacementAction {
            backspace_count: 0,
            delete_count: 0,
            insert_text: format!("{}{}", &suggestion[typed.len()..], space),
        }
    } else {
        ReplacementAction {
            backspace_count: typed.chars().count(),
            delete_count: 0,
            insert_text: format!("{}{}", suggestion, space),
        }
    }
}

/// Replaces `before` + `after` (the text on both sides of the cursor) with `suggestion`.
pub fn replace_range(before: &str, after: &str, suggestion: &str) -> ReplacementAction {
    if after.is_empty() {
        return calculate_replacement(before, suggestion);
    }
    ReplacementAction {
        backspace_count: before.chars().count(),
        delete_count: after.chars().count(),
        insert_text: format!("{}{}", suggestion, trailing_space(suggestion)),
    }
}

/// Converts a UTF-16 code-unit index (the unit PowerShell and .NET use) into a byte index into `s`.
/// Returns `None` when the index is past the end or falls inside a surrogate pair.
pub fn utf16_to_byte_index(s: &str, utf16_idx: usize) -> Option<usize> {
    let mut units = 0;
    for (byte_idx, ch) in s.char_indices() {
        if units == utf16_idx {
            return Some(byte_idx);
        }
        units += ch.len_utf16();
        if units > utf16_idx {
            return None;
        }
    }
    (units == utf16_idx).then_some(s.len())
}
