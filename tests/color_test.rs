use shell_panel::ui::color::{parse_color_bg, parse_color_fg};

#[test]
fn test_parse_standard_colors_fg() {
    assert_eq!(parse_color_fg("black"), Some("\x1b[30m".to_string()));
    assert_eq!(parse_color_fg("red"), Some("\x1b[31m".to_string()));
    assert_eq!(parse_color_fg("green"), Some("\x1b[32m".to_string()));
    assert_eq!(parse_color_fg("yellow"), Some("\x1b[33m".to_string()));
    assert_eq!(parse_color_fg("blue"), Some("\x1b[34m".to_string()));
    assert_eq!(parse_color_fg("magenta"), Some("\x1b[35m".to_string()));
    assert_eq!(parse_color_fg("purple"), Some("\x1b[35m".to_string()));
    assert_eq!(parse_color_fg("cyan"), Some("\x1b[36m".to_string()));
    assert_eq!(parse_color_fg("white"), Some("\x1b[37m".to_string()));
    assert_eq!(parse_color_fg("gray"), Some("\x1b[90m".to_string()));
    assert_eq!(parse_color_fg("grey"), Some("\x1b[90m".to_string()));
    assert_eq!(parse_color_fg("bright_black"), Some("\x1b[90m".to_string()));
    assert_eq!(parse_color_fg("bright_red"), Some("\x1b[91m".to_string()));
    assert_eq!(parse_color_fg("bright_green"), Some("\x1b[92m".to_string()));
    assert_eq!(parse_color_fg("bright_yellow"), Some("\x1b[93m".to_string()));
    assert_eq!(parse_color_fg("bright_blue"), Some("\x1b[94m".to_string()));
    assert_eq!(parse_color_fg("bright_magenta"), Some("\x1b[95m".to_string()));
    assert_eq!(parse_color_fg("bright_purple"), Some("\x1b[95m".to_string()));
    assert_eq!(parse_color_fg("bright_cyan"), Some("\x1b[96m".to_string()));
    assert_eq!(parse_color_fg("bright_white"), Some("\x1b[97m".to_string()));
}

#[test]
fn test_parse_standard_colors_bg() {
    assert_eq!(parse_color_bg("black"), Some("\x1b[40m".to_string()));
    assert_eq!(parse_color_bg("red"), Some("\x1b[41m".to_string()));
    assert_eq!(parse_color_bg("green"), Some("\x1b[42m".to_string()));
    assert_eq!(parse_color_bg("yellow"), Some("\x1b[43m".to_string()));
    assert_eq!(parse_color_bg("blue"), Some("\x1b[44m".to_string()));
    assert_eq!(parse_color_bg("magenta"), Some("\x1b[45m".to_string()));
    assert_eq!(parse_color_bg("purple"), Some("\x1b[45m".to_string()));
    assert_eq!(parse_color_bg("cyan"), Some("\x1b[46m".to_string()));
    assert_eq!(parse_color_bg("white"), Some("\x1b[47m".to_string()));
    assert_eq!(parse_color_bg("gray"), Some("\x1b[100m".to_string()));
    assert_eq!(parse_color_bg("grey"), Some("\x1b[100m".to_string()));
    assert_eq!(parse_color_bg("bright_black"), Some("\x1b[100m".to_string()));
    assert_eq!(parse_color_bg("bright_red"), Some("\x1b[101m".to_string()));
    assert_eq!(parse_color_bg("bright_green"), Some("\x1b[102m".to_string()));
    assert_eq!(parse_color_bg("bright_yellow"), Some("\x1b[103m".to_string()));
    assert_eq!(parse_color_bg("bright_blue"), Some("\x1b[104m".to_string()));
    assert_eq!(parse_color_bg("bright_magenta"), Some("\x1b[105m".to_string()));
    assert_eq!(parse_color_bg("bright_cyan"), Some("\x1b[106m".to_string()));
    assert_eq!(parse_color_bg("bright_white"), Some("\x1b[107m".to_string()));
}

#[test]
fn test_parse_case_insensitivity_and_variants() {
    assert_eq!(parse_color_fg("CYAN"), Some("\x1b[36m".to_string()));
    assert_eq!(parse_color_bg("Blue"), Some("\x1b[44m".to_string()));
    assert_eq!(parse_color_fg("Bright-Red"), Some("\x1b[91m".to_string()));
    assert_eq!(parse_color_bg("bright red"), Some("\x1b[101m".to_string()));
}

#[test]
fn test_parse_special_modes() {
    assert_eq!(parse_color_fg("invert"), Some("\x1b[7m".to_string()));
    assert_eq!(parse_color_bg("invert"), Some("\x1b[7m".to_string()));
    assert_eq!(parse_color_fg("reverse"), Some("\x1b[7m".to_string()));
    assert_eq!(parse_color_bg("REVERSE"), Some("\x1b[7m".to_string()));

    assert_eq!(parse_color_fg(""), None);
    assert_eq!(parse_color_bg(""), None);
    assert_eq!(parse_color_fg("none"), None);
    assert_eq!(parse_color_bg("NONE"), None);
    assert_eq!(parse_color_fg("default"), None);
    assert_eq!(parse_color_bg("DEFAULT"), None);
}

#[test]
fn test_parse_hex_colors() {
    assert_eq!(parse_color_fg("#ff0000"), Some("\x1b[38;2;255;0;0m".to_string()));
    assert_eq!(parse_color_bg("#00ff00"), Some("\x1b[48;2;0;255;0m".to_string()));
    assert_eq!(parse_color_fg("#0000ff"), Some("\x1b[38;2;0;0;255m".to_string()));
    assert_eq!(parse_color_fg("#123456"), Some("\x1b[38;2;18;52;86m".to_string()));
    assert_eq!(parse_color_fg("#AbCdEf"), Some("\x1b[38;2;171;205;239m".to_string()));

    // 3-digit hex (#RGB expands to #RRGGBB)
    assert_eq!(parse_color_fg("#f0a"), Some("\x1b[38;2;255;0;170m".to_string()));
    assert_eq!(parse_color_bg("#fff"), Some("\x1b[48;2;255;255;255m".to_string()));
    assert_eq!(parse_color_bg("#000"), Some("\x1b[48;2;0;0;0m".to_string()));
}

#[test]
fn test_parse_256_colors() {
    assert_eq!(parse_color_fg("0"), Some("\x1b[38;5;0m".to_string()));
    assert_eq!(parse_color_bg("16"), Some("\x1b[48;5;16m".to_string()));
    assert_eq!(parse_color_fg("244"), Some("\x1b[38;5;244m".to_string()));
    assert_eq!(parse_color_bg("255"), Some("\x1b[48;5;255m".to_string()));
}

#[test]
fn test_parse_invalid_colors() {
    assert_eq!(parse_color_fg("256"), None);
    assert_eq!(parse_color_bg("999"), None);
    assert_eq!(parse_color_fg("-1"), None);
    assert_eq!(parse_color_fg("#12"), None);
    assert_eq!(parse_color_fg("#1234"), None);
    assert_eq!(parse_color_fg("#1234567"), None);
    assert_eq!(parse_color_fg("#gggggg"), None);
    assert_eq!(parse_color_fg("unknown_color"), None);
    assert_eq!(parse_color_bg("notacolor"), None);
}
