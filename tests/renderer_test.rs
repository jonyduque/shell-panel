use shell_panel::engine::provider::Suggestion;
use shell_panel::ui::patch::restore_line;
use shell_panel::ui::renderer::{DropdownLayout, Renderer};
use shell_panel::ui::suggestion_state::SuggestionState;
use shell_panel::ui::theme::{Theme, SELECTED_PREFIX, UNSELECTED_PREFIX};
use shell_panel::vt::emulator::HeadlessTerminal;

#[test]
fn test_suggestion_state_navigation_and_wrapping() {
    let mut state = SuggestionState::new(5);
    assert_eq!(state.total_items(), 0);
    assert!(!state.visible);

    // Empty state operations do not panic
    state.move_down();
    state.move_up();
    assert_eq!(state.active_index(), 0);
    assert_eq!(state.active_item(), None);

    let suggestions = (0..3)
        .map(|i| Suggestion::new(format!("cmd{}", i), format!("cmd{}", i), None, 10))
        .collect();

    state.set_suggestions(suggestions);
    assert!(state.visible);
    assert_eq!(state.total_items(), 3);
    assert_eq!(state.active_index(), 0);
    assert_eq!(state.active_item().unwrap().name, "cmd0");

    // Move down: 0 -> 1 -> 2 -> wrap to 0
    state.move_down();
    assert_eq!(state.active_index(), 1);
    state.move_down();
    assert_eq!(state.active_index(), 2);
    state.move_down();
    assert_eq!(state.active_index(), 0);

    // Move up: wrap from 0 to 2 -> 1 -> 0
    state.move_up();
    assert_eq!(state.active_index(), 2);
    state.move_up();
    assert_eq!(state.active_index(), 1);
    state.move_up();
    assert_eq!(state.active_index(), 0);
}

#[test]
fn test_suggestion_state_pagination_and_dismiss() {
    // 7 items with max_rows = 3 => 3 pages (sizes: 3, 3, 1)
    let mut state = SuggestionState::new(3);
    let suggestions = (0..7)
        .map(|i| {
            Suggestion::new(
                format!("item{}", i),
                format!("item{}", i),
                Some(format!("desc{}", i)),
                10,
            )
        })
        .collect();

    state.set_suggestions(suggestions);
    assert!(state.visible);

    // Page 1: items 0, 1, 2
    let page1 = state.visible_page();
    assert_eq!(page1.len(), 3);
    assert_eq!(page1[0].0.name, "item0");
    assert!(page1[0].1); // item 0 is active
    assert!(!page1[1].1);
    assert!(!page1[2].1);

    // Move to item 3 (Page 2)
    state.move_down(); // 1
    state.move_down(); // 2
    state.move_down(); // 3
    assert_eq!(state.active_index(), 3);

    let page2 = state.visible_page();
    assert_eq!(page2.len(), 3);
    assert_eq!(page2[0].0.name, "item3");
    assert!(page2[0].1); // item 3 is active
    assert_eq!(page2[1].0.name, "item4");
    assert!(!page2[1].1);
    assert_eq!(page2[2].0.name, "item5");
    assert!(!page2[2].1);

    // Move to item 6 (Page 3)
    state.move_down(); // 4
    state.move_down(); // 5
    state.move_down(); // 6
    assert_eq!(state.active_index(), 6);

    let page3 = state.visible_page();
    assert_eq!(page3.len(), 1);
    assert_eq!(page3[0].0.name, "item6");
    assert!(page3[0].1);

    // Dismiss
    state.dismiss();
    assert!(!state.visible);
}

#[test]
fn test_render_dropdown_below_cursor_and_ansi_sequences() {
    let term = HeadlessTerminal::new(80, 24);
    let mut state = SuggestionState::new(5);
    let suggestions = vec![
        Suggestion::new(
            "git status",
            "git status",
            Some("Show working tree status".to_string()),
            100,
        ),
        Suggestion::new(
            "git switch",
            "git switch",
            Some("Switch branches".to_string()),
            90,
        ),
    ];
    state.set_suggestions(suggestions);

    let mut out = Vec::new();
    let cursor_x = 4;
    let cursor_y = 5;

    let layout = Renderer::render_dropdown(
        &state,
        &term,
        &Theme::default(),
        cursor_x,
        cursor_y,
        &mut out,
    )
    .expect("render_dropdown succeeds");

    assert_eq!(
        layout,
        Some(DropdownLayout {
            start_row: 6,
            row_count: 2,
        })
    );

    let rendered = String::from_utf8(out).expect("valid utf-8 output");

    // Verify cursor hiding and saving
    assert!(rendered.contains("\x1b[?25l"), "must hide cursor");
    assert!(rendered.contains("\x1b[s"), "must save cursor position");

    // Verify cursor restoring and unhiding
    assert!(rendered.contains("\x1b[u"), "must restore cursor position");
    assert!(rendered.contains("\x1b[?25h"), "must show cursor");

    // Verify active line inverted highlight (\x1b[7m)
    assert!(
        rendered.contains("\x1b[7m"),
        "must contain highlight sequence for active item"
    );

    // Verify position moves to row 7 (start_row 6 + 1) col 5 (cursor_x 4 + 1)
    assert!(
        rendered.contains("\x1b[7;5H"),
        "must move to start_row + 1 at col"
    );
}

#[test]
fn test_render_dropdown_above_cursor_when_at_bottom() {
    let term = HeadlessTerminal::new(80, 24);
    let mut state = SuggestionState::new(5);
    let suggestions = (0..5)
        .map(|i| Suggestion::new(format!("sug{}", i), format!("sug{}", i), None, 10))
        .collect();
    state.set_suggestions(suggestions);

    let mut out = Vec::new();
    let cursor_x = 0;
    let cursor_y = 23; // Bottom row in 24-row terminal (0-indexed: 0..23)

    let layout = Renderer::render_dropdown(
        &state,
        &term,
        &Theme::default(),
        cursor_x,
        cursor_y,
        &mut out,
    )
    .expect("render_dropdown succeeds");

    // Should render above: start_row = 23 - 5 = 18, row_count = 5
    assert_eq!(
        layout,
        Some(DropdownLayout {
            start_row: 18,
            row_count: 5,
        })
    );

    let rendered = String::from_utf8(out).expect("valid utf-8 output");
    // Target rows: 18 + 1 = 19, 20, 21, 22, 23
    assert!(rendered.contains("\x1b[19;1H"));
    assert!(rendered.contains("\x1b[23;1H"));
}

#[test]
fn test_render_dropdown_hidden_or_empty_returns_none() {
    let term = HeadlessTerminal::new(80, 24);
    let mut state = SuggestionState::new(5);
    let mut out = Vec::new();

    // Empty state
    let res = Renderer::render_dropdown(&state, &term, &Theme::default(), 0, 0, &mut out).unwrap();
    assert_eq!(res, None);
    assert!(out.is_empty());

    // Dismissed state
    state.set_suggestions(vec![Suggestion::new("test", "test", None, 1)]);
    state.dismiss();
    let res = Renderer::render_dropdown(&state, &term, &Theme::default(), 0, 0, &mut out).unwrap();
    assert_eq!(res, None);
    assert!(out.is_empty());
}

#[test]
fn test_clear_dropdown_restores_terminal_lines() {
    let mut term = HeadlessTerminal::new(80, 24);
    // Write some lines into the terminal
    term.process(b"\x1b[1;1HLine 1 content\r\n\x1b[2;1HLine 2 content\r\n\x1b[3;1HLine 3 content");

    let layout = DropdownLayout {
        start_row: 1, // 0-indexed line 1 corresponds to "Line 2 content"
        row_count: 2, // lines 1 and 2
    };

    let mut out = Vec::new();
    Renderer::clear_dropdown(&layout, &term, &mut out).expect("clear_dropdown succeeds");

    let cleared = String::from_utf8(out).expect("valid utf-8 output");

    // Check cursor hide/save/restore/show
    assert!(cleared.contains("\x1b[?25l"));
    assert!(cleared.contains("\x1b[s"));
    assert!(cleared.contains("\x1b[u"));
    assert!(cleared.contains("\x1b[?25h"));

    // Check that restore_line ANSI sequences for row 1 and row 2 were output
    // Row 1 (0-indexed) is 2 in 1-indexed ANSI
    let expected_row1 = restore_line(1, &term);
    let expected_row2 = restore_line(2, &term);

    assert!(cleared.contains(&expected_row1));
    assert!(cleared.contains(&expected_row2));
    assert!(expected_row1.contains("Line 2 content"));
    assert!(expected_row2.contains("Line 3 content"));
}

#[test]
fn test_theme_constants_and_helpers() {
    assert_eq!(SELECTED_PREFIX, "> ");
    assert_eq!(UNSELECTED_PREFIX, "  ");
    assert_eq!(Theme::SELECTED_PREFIX, "> ");
    assert_eq!(Theme::UNSELECTED_PREFIX, "  ");

    let sel = Theme::format_selected("hello");
    assert!(sel.starts_with("\x1b[7m"));
    assert!(sel.ends_with("\x1b[0m"));
    assert!(sel.contains("hello"));

    let desc = Theme::format_description("help text");
    assert!(desc.starts_with("\x1b[90m"));
    assert!(desc.ends_with("\x1b[0m"));
    assert!(desc.contains("help text"));
}

fn many(n: usize) -> Vec<Suggestion> {
    (0..n)
        .map(|i| Suggestion::new(format!("item{i}"), format!("item{i}"), None, 50))
        .collect()
}

#[test]
fn test_render_dropdown_never_writes_past_right_edge() {
    let term = HeadlessTerminal::new(80, 24);
    let mut state = SuggestionState::new(5);
    state.set_suggestions(vec![
        Suggestion::new(
            "a-very-long-suggestion-name",
            "a-very-long-suggestion-name",
            Some("with a long description".into()),
            80,
        ),
        Suggestion::new("second", "second", None, 70),
    ]);

    let mut out = Vec::new();
    Renderer::render_dropdown(&state, &term, &Theme::default(), 70, 5, &mut out).unwrap();

    // Replay on a blank screen: the panel starts at column 70, anything left of it wrapped.
    let mut screen = HeadlessTerminal::new(80, 24);
    screen.process(&out);
    for row in 0..24 {
        for col in 0..70 {
            let cell = screen.screen().cell(row, col).unwrap();
            assert!(
                cell.contents().is_empty(),
                "overflow wrote {:?} at row {} col {}",
                cell.contents(),
                row,
                col
            );
        }
    }
}

#[test]
fn test_dropdown_never_covers_the_cursor_row() {
    // 8 rows, cursor on row 4, 5 suggestions wanted: 3 rows below, 4 above -> 4 rows above.
    let term = HeadlessTerminal::new(80, 8);
    let mut state = SuggestionState::new(5);
    state.set_suggestions(many(12));

    let mut out = Vec::new();
    let layout = Renderer::render_dropdown(&state, &term, &Theme::default(), 0, 4, &mut out)
        .unwrap()
        .expect("rendered");

    assert_eq!(
        layout,
        DropdownLayout {
            start_row: 0,
            row_count: 4
        }
    );
}

#[test]
fn test_dropdown_in_one_row_terminal_is_not_rendered() {
    let term = HeadlessTerminal::new(80, 1);
    let mut state = SuggestionState::new(5);
    state.set_suggestions(many(3));
    let mut out = Vec::new();
    assert_eq!(
        Renderer::render_dropdown(&state, &term, &Theme::default(), 0, 0, &mut out).unwrap(),
        None
    );
}

#[test]
fn test_visible_page_with_smaller_page() {
    let mut state = SuggestionState::new(5);
    state.set_suggestions(many(7));
    for _ in 0..4 {
        state.move_down(); // active index 4 -> second page of size 4
    }
    let page = state.visible_page_with(4);
    assert_eq!(page.len(), 3);
    assert!(page[0].1);
    assert_eq!(page[0].0.name, "item4");
}

#[test]
fn test_render_dropdown_with_custom_theme() {
    use shell_panel::core::config::{ColorConfig, Config, IconConfig};
    use shell_panel::engine::provider::SuggestionKind;

    let term = HeadlessTerminal::new(80, 24);
    let mut state = SuggestionState::new(5);
    let suggestions = vec![
        Suggestion::new(
            "git status",
            "git status",
            Some("Show status".to_string()),
            100,
        )
        .with_kind(SuggestionKind::Command),
        Suggestion::new(
            "git switch",
            "git switch",
            Some("Switch branches".to_string()),
            90,
        )
        .with_kind(SuggestionKind::Subcommand),
    ];
    state.set_suggestions(suggestions);

    let config = Config {
        colors: ColorConfig {
            selected_bg: "magenta".to_string(),
            selected_fg: "white".to_string(),
            unselected_fg: "cyan".to_string(),
            description_fg: "yellow".to_string(),
            selected_prefix: ">> ".to_string(),
            unselected_prefix: "   ".to_string(),
        },
        icons: IconConfig {
            command: "CMD: ".to_string(),
            subcommand: "SUB: ".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let theme = Theme::from_config(&config);
    let mut out = Vec::new();
    let layout = Renderer::render_dropdown(&state, &term, &theme, 2, 3, &mut out)
        .expect("render_dropdown succeeds");

    assert!(layout.is_some());
    let rendered = String::from_utf8(out).expect("valid utf-8 output");

    // Check custom theme colors & prefixes & icons
    assert!(
        rendered.contains("\x1b[45m\x1b[37m"),
        "must have magenta bg and white fg for selected"
    );
    assert!(
        rendered.contains(">> "),
        "must contain custom selected prefix"
    );
    assert!(
        rendered.contains("CMD: "),
        "must contain custom command icon"
    );
    assert!(rendered.contains("git status"), "must contain command text");
    assert!(
        rendered.contains("SUB: "),
        "must contain custom subcommand icon"
    );
    assert!(
        rendered.contains("\x1b[36m"),
        "must have unselected cyan fg"
    );
    assert!(
        rendered.contains("\x1b[33m"),
        "must have description yellow fg"
    );
}

#[test]
fn test_restore_line_keeps_colors() {
    let mut term = HeadlessTerminal::new(80, 24);
    term.process(b"\x1b[31mRED\x1b[0m plain");

    let restored = restore_line(0, &term);

    assert!(restored.starts_with("\x1b[1;1H"));
    assert!(restored.contains("31m"), "color lost: {:?}", restored);
    assert!(restored.contains("RED"));
    assert!(restored.contains("plain"));

    // Replaying the restore on a blank screen reproduces the colored cell.
    let mut replay = HeadlessTerminal::new(80, 24);
    replay.process(restored.as_bytes());
    assert_eq!(
        replay.screen().cell(0, 0).unwrap().fgcolor(),
        vt100::Color::Idx(1)
    );
    assert_eq!(
        replay.screen().cell(0, 4).unwrap().fgcolor(),
        vt100::Color::Default
    );
}

#[test]
fn test_zero_max_suggestions_means_five_and_still_renders() {
    assert_eq!(SuggestionState::new(0).max_rows, 5);
    let mut state = SuggestionState::new(0);
    state.set_suggestions(many(3));
    let term = HeadlessTerminal::new(80, 24);
    let mut out = Vec::new();
    let layout =
        Renderer::render_dropdown(&state, &term, &Theme::default(), 0, 0, &mut out).unwrap();
    assert_eq!(layout.map(|l| l.row_count), Some(3));
}
