use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum ActionKey {
    MenuUp,
    MenuDown,
    AcceptSuggestion,
    DismissMenu,
    Passthrough,
}

pub fn classify_key(event: &KeyEvent) -> ActionKey {
    // Ignore key release events on Windows to prevent duplicate menu navigation
    if event.kind == KeyEventKind::Release {
        return ActionKey::Passthrough;
    }

    // Do not hijack Ctrl or Alt key combinations
    if event.modifiers.contains(KeyModifiers::CONTROL) || event.modifiers.contains(KeyModifiers::ALT) {
        return ActionKey::Passthrough;
    }

    match event.code {
        KeyCode::Up => ActionKey::MenuUp,
        KeyCode::Down => ActionKey::MenuDown,
        KeyCode::Tab => ActionKey::AcceptSuggestion,
        KeyCode::BackTab => ActionKey::MenuUp, // Shift+Tab cycles backwards
        KeyCode::Esc => ActionKey::DismissMenu,
        _ => ActionKey::Passthrough,
    }
}
