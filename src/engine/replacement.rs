/// Action specifying how to replace the current typed token with the chosen suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementAction {
    /// Number of backspaces to send to erase the current typed input.
    pub backspace_count: usize,
    /// Text to insert into the terminal, including any trailing space.
    pub insert_text: String,
}

/// Computes the terminal key sequence needed to transform `typed` into `suggestion` (with a trailing space).
///
/// Handles:
/// 1. Exact prefix match: zero backspaces, inserts only the remaining suffix + " ".
/// 2. Case mismatch prefix: backspaces the entire typed token and inserts the full suggestion + " ".
/// 3. Non-prefix (fuzzy/full replacement): backspaces the entire typed token and inserts full suggestion + " ".
///
/// Uses character count rather than byte count for safe Unicode (CJK/emojis) backspacing.
pub fn calculate_replacement(typed: &str, suggestion: &str) -> ReplacementAction {
    let trailing_space = if suggestion.ends_with('/') || suggestion.ends_with('\\') {
        ""
    } else {
        " "
    };

    if suggestion.starts_with(typed) {
        ReplacementAction {
            backspace_count: 0,
            insert_text: format!("{}{}", &suggestion[typed.len()..], trailing_space),
        }
    } else {
        ReplacementAction {
            backspace_count: typed.chars().count(),
            insert_text: format!("{}{}", suggestion, trailing_space),
        }
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
