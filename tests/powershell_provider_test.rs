use shell_panel::engine::provider::{CompletionProvider, SuggestionKind};
use shell_panel::engine::providers::powershell::parse_powershell_completion_json;
use shell_panel::engine::providers::PowerShellProvider;

#[test]
fn test_parse_powershell_cmdlet_and_parameter_json() {
    let json = r#"{
        "replacementIndex": 0,
        "replacementLength": 6,
        "matches": [
            {
                "name": "Get-ChildItem",
                "display": "Get-ChildItem",
                "type": "Command",
                "description": "Get-ChildItem [[-Path] <string[]>]"
            },
            {
                "name": "-Path",
                "display": "Path",
                "type": "ParameterName",
                "description": "[string[]] Path"
            }
        ]
    }"#;

    let sugs = parse_powershell_completion_json(json);
    assert_eq!(sugs.len(), 2);
    assert_eq!(sugs[0].name, "Get-ChildItem");
    assert_eq!(sugs[0].kind, SuggestionKind::PowerShellCmdlet);
    assert_eq!(sugs[0].priority, 80);

    assert_eq!(sugs[1].name, "-Path");
    assert_eq!(sugs[1].kind, SuggestionKind::Option);
    assert_eq!(sugs[1].priority, 75);
}

#[tokio::test]
async fn test_powershell_provider_live_query() {
    let provider = PowerShellProvider::default();
    let sugs = provider.complete("Get-Ch", "").await;
    assert!(
        sugs.iter().any(|s| s.name.eq_ignore_ascii_case("Get-ChildItem")),
        "Expected Get-ChildItem in completions, got: {:?}",
        sugs
    );

    let param_sugs = provider.complete("Get-ChildItem -", "").await;
    assert!(
        param_sugs.iter().any(|s| s.name.eq_ignore_ascii_case("-Path")),
        "Expected -Path in parameter completions, got: {:?}",
        param_sugs
    );
}
