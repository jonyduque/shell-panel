use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// High-level action classified from a terminal key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKey {
    MenuUp,
    MenuDown,
    AcceptSuggestion,
    DismissMenu,
    Passthrough,
}

/// Classify key event into navigation / menu actions or passthrough.
pub fn classify_key(event: &KeyEvent) -> ActionKey {
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        return ActionKey::Passthrough;
    }
    match event.code {
        KeyCode::Up => ActionKey::MenuUp,
        KeyCode::Down => ActionKey::MenuDown,
        KeyCode::Tab => ActionKey::AcceptSuggestion,
        KeyCode::Esc => ActionKey::DismissMenu,
        _ => ActionKey::Passthrough,
    }
}
