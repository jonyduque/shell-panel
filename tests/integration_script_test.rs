use shell_panel::core::app::get_shell_integration_path;

#[test]
fn test_shell_integration_path_has_no_verbatim_prefix() {
    let path = get_shell_integration_path().expect("Should resolve path");
    let path_str = path.to_string_lossy();
    assert!(!path_str.starts_with(r"\\?\"), "Path must not contain \\?\\ prefix: {}", path_str);
    assert!(path.is_file(), "File must exist: {:?}", path);
}
