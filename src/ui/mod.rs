// Floating UI and virtual patching renderer
pub mod color;
pub mod patch;
pub mod renderer;
pub mod suggestion_state;
pub mod theme;

pub use color::{parse_color_bg, parse_color_fg};
pub use patch::{restore_line, LinePatch};
pub use renderer::{clear_dropdown, render_dropdown, DropdownLayout, Renderer};
pub use suggestion_state::SuggestionState;
pub use theme::{
    format_description, format_selected, format_suggestion_line,
    format_suggestion_line_with_min_width, format_suggestion_line_with_theme,
    format_suggestion_line_with_theme_and_min_width, Theme, SELECTED_PREFIX, UNSELECTED_PREFIX,
};

