// Floating UI and virtual patching renderer
pub mod patch;
pub mod renderer;
pub mod suggestion_state;
pub mod theme;

pub use patch::{restore_line, LinePatch};
pub use renderer::{clear_dropdown, render_dropdown, DropdownLayout, Renderer};
pub use suggestion_state::SuggestionState;
pub use theme::{format_description, format_selected, Theme, SELECTED_PREFIX, UNSELECTED_PREFIX};

