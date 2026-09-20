use crate::engine::provider::Suggestion;

/// Represents the navigation and pagination state of completion suggestions.
#[derive(Debug, Clone)]
pub struct SuggestionState {
    suggestions: Vec<Suggestion>,
    active_idx: usize,
    pub visible: bool,
    pub max_rows: usize,
}

impl Default for SuggestionState {
    fn default() -> Self {
        Self {
            suggestions: Vec::new(),
            active_idx: 0,
            visible: false,
            max_rows: 5,
        }
    }
}

impl SuggestionState {
    /// Creates a new `SuggestionState` with the specified maximum rows per page.
    pub fn new(max_rows: usize) -> Self {
        Self {
            suggestions: Vec::new(),
            active_idx: 0,
            visible: false,
            max_rows: if max_rows == 0 { 5 } else { max_rows },
        }
    }

    /// Sets the list of suggestions, resetting the active index to 0
    /// and setting `visible` to true if the list is non-empty.
    pub fn set_suggestions(&mut self, list: Vec<Suggestion>) {
        self.active_idx = 0;
        self.visible = !list.is_empty();
        self.suggestions = list;
    }

    /// Returns the currently active (highlighted) index.
    pub fn active_index(&self) -> usize {
        self.active_idx
    }

    /// Returns a reference to the currently active suggestion, if any.
    pub fn active_item(&self) -> Option<&Suggestion> {
        self.suggestions.get(self.active_idx)
    }

    /// Returns the total number of suggestions.
    pub fn total_items(&self) -> usize {
        self.suggestions.len()
    }

    /// Moves the selection down cyclically (wraps from last to first).
    pub fn move_down(&mut self) {
        if !self.suggestions.is_empty() {
            self.active_idx = (self.active_idx + 1) % self.suggestions.len();
        }
    }

    /// Moves the selection up cyclically (wraps from first to last).
    pub fn move_up(&mut self) {
        if !self.suggestions.is_empty() {
            if self.active_idx == 0 {
                self.active_idx = self.suggestions.len() - 1;
            } else {
                self.active_idx -= 1;
            }
        }
    }

    /// Dismisses the suggestions dropdown menu.
    pub fn dismiss(&mut self) {
        self.visible = false;
    }

    /// Returns the slice of suggestions on the current visible page,
    /// each paired with a boolean indicating whether it is the active item.
    pub fn visible_page(&self) -> Vec<(&Suggestion, bool)> {
        if self.suggestions.is_empty() {
            return Vec::new();
        }

        let max_rows = self.max_rows.max(1);
        let page = self.active_idx / max_rows;
        let start = page * max_rows;
        let end = (start + max_rows).min(self.suggestions.len());

        self.suggestions[start..end]
            .iter()
            .enumerate()
            .map(|(offset, item)| (item, (start + offset) == self.active_idx))
            .collect()
    }

    /// Returns `(current_page_1_indexed, total_pages)`.
    pub fn page_info(&self) -> (usize, usize) {
        if self.suggestions.is_empty() {
            return (0, 0);
        }

        let max_rows = self.max_rows.max(1);
        let total_pages = (self.suggestions.len() + max_rows - 1) / max_rows;
        let current_page = (self.active_idx / max_rows) + 1;
        (current_page, total_pages)
    }
}
