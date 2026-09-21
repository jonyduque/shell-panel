# PowerShell Native Cmdlet & Parameter Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement native PowerShell completion for cmdlets (`Get-Process`, `Get-ChildItem`), aliases (`ls`, `gci`, `select`), parameters (`-Path`, `-Recurse`, `-Force`), and PowerShell variables/functions via a persistent, high-performance in-memory `CommandCompletion` worker.

**Architecture:**
1. **PowerShell Completion Bridge (`assets/psWorker.ps1` & `src/engine/providers/powershell.rs`):**
   - Embed a lightweight, persistent PowerShell worker script that reads JSON from stdin, invokes `[System.Management.Automation.CommandCompletion]::CompleteInput(...)`, and outputs single-line JSON responses with `< 1ms` in-memory latency.
   - Rust struct `PowerShellWorker` runs as a supervised background child process with asynchronous stdin/stdout pipes and oneshot request/response dispatch.
   - Map `ResultType` to `SuggestionKind`:
     - `Command` $\to$ `SuggestionKind::PowerShellCmdlet` (`>_ `) or `SuggestionKind::Alias` (`🔗 `).
     - `ParameterName` $\to$ `SuggestionKind::Option` (`🏷️  `).
     - `ProviderContainer` $\to$ `SuggestionKind::Directory` (`📁 `).
     - `ProviderItem` $\to$ `SuggestionKind::File` (`📄 `).
     - `Variable` / `ParameterValue` / `Method` / `Property` $\to$ `SuggestionKind::Other`.
2. **Provider Integration & Priority Arbitrage (`src/core/app.rs`):**
   - Register `PowerShellProvider` alongside `JsonSpecProvider`, `ZoxideProvider`, and `FileProvider`.
   - If `JsonSpecProvider` handles the root command (e.g. `git`, `docker`), Fig specs take precedence.
   - For all PowerShell cmdlets, aliases, and general commands, `PowerShellProvider` supplies native completions and parameters with syntax tooltips (`[string[]] Path`).
   - If PowerShell returns `ParameterName` options, format replacement to complete the parameter name seamlessly.

**Tech Stack:** Rust 1.80+, Tokio async process, serde/serde_json, PowerShell 7 / Windows PowerShell.

---

### Task 1: Persistent PowerShell Completion Worker Script & Protocol

**Files:**
- Create: `assets/psWorker.ps1`
- Create: `src/engine/providers/powershell.rs`
- Modify: `src/engine/providers/mod.rs`
- Test: `tests/powershell_provider_test.rs`

- [ ] **Step 1: Create `assets/psWorker.ps1`**

```powershell
$input_stream = [Console]::In
$output_stream = [Console]::Out

while ($line = $input_stream.ReadLine()) {
    if (-not $line) { continue }
    if ($line -eq "EXIT") { break }
    try {
        $req = $line | ConvertFrom-Json
        $text = $req.text
        $cursor = if ($null -ne $req.cursor) { [int]$req.cursor } else { $text.Length }
        $completions = [System.Management.Automation.CommandCompletion]::CompleteInput($text, $cursor, $null)
        $results = @()
        if ($completions -and $completions.CompletionMatches) {
            foreach ($match in $completions.CompletionMatches) {
                $results += @{
                    name = $match.CompletionText
                    display = $match.ListItemText
                    description = $match.ToolTip
                    type = $match.ResultType.ToString()
                }
            }
        }
        $json = @{
            replacementIndex = $completions.ReplacementIndex
            replacementLength = $completions.ReplacementLength
            matches = $results
        } | ConvertTo-Json -Compress -Depth 5
        $output_stream.WriteLine($json)
        $output_stream.Flush()
    } catch {
        $output_stream.WriteLine('{"matches":[]}')
        $output_stream.Flush()
    }
}
```

- [ ] **Step 2: Write unit test for `PowerShellProvider` JSON parsing and mapping**

Create `tests/powershell_provider_test.rs`:
```rust
use shell_panel::engine::provider::SuggestionKind;
use shell_panel::engine::providers::powershell::parse_powershell_completion_json;

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
```

- [ ] **Step 3: Implement `PowerShellProvider` in `src/engine/providers/powershell.rs`**

- `pub fn parse_powershell_completion_json(json: &str) -> Vec<Suggestion>`:
  Maps `Command` to `PowerShellCmdlet` (or `Alias`), `ParameterName` to `Option`, `ProviderContainer` to `Directory`, `ProviderItem` to `File`.
- `pub struct PowerShellProvider`:
  Spawns background `pwsh.exe` or `powershell.exe` with `assets/psWorker.ps1` (or temp-extracted script).
  Uses a channel to dispatch completion queries asynchronously without blocking the event loop.
  Includes timeout protection (300ms fallback).
  Implements `CompletionProvider`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test powershell_provider_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add assets/psWorker.ps1 src/engine/providers/powershell.rs src/engine/providers/mod.rs tests/powershell_provider_test.rs
git commit -m "feat(engine): implement powershell native command completion provider"
```

---

### Task 2: Wire `PowerShellProvider` into Reactor Loop and Completion Pipeline

**Files:**
- Modify: `src/core/app.rs`
- Test: `tests/tab_trigger_test.rs`

- [x] **Step 1: Add `PowerShellProvider` to `App` in `src/core/app.rs`**

- Initialize `let powershell_provider = PowerShellProvider::new(shell_type);`
- In `ActionKey::AcceptSuggestion`:
  When querying providers on Tab:
  ```rust
  let ps_fut = async {
      powershell_provider.complete(text, &command_state.cwd).await
  };
  let (mut json_sugs, mut zoxide_sugs, mut ps_sugs, mut file_sugs) = tokio::join!(json_fut, zoxide_fut, ps_fut, file_fut);
  ```
- If `json_sugs` has matches (e.g. `git clone`), use `json_sugs`.
- Otherwise include `ps_sugs` (which provides `Get-ChildItem`, `Get-Process`, `-Path`, etc.).
- Sort and deduplicate suggestions.

- [x] **Step 2: Add integration test for PowerShell cmdlet and argument completion**

In `tests/tab_trigger_test.rs`:
```rust
#[tokio::test]
async fn test_powershell_provider_live_completions() {
    let provider = PowerShellProvider::default();
    let sugs = provider.complete("Get-Ch", "").await;
    assert!(sugs.iter().any(|s| s.name.eq_ignore_ascii_case("Get-ChildItem")));

    let param_sugs = provider.complete("Get-ChildItem -", "").await;
    assert!(param_sugs.iter().any(|s| s.name.eq_ignore_ascii_case("-Path")));
}
```

- [x] **Step 3: Run full test suite and verify**

Run: `cargo test`
Expected: All tests PASS.

- [x] **Step 4: Build release binary**

Run: `cargo build --release`

- [x] **Step 5: Commit changes**

```bash
git add src/core/app.rs tests/tab_trigger_test.rs
git commit -m "feat(core): wire powershell native completion provider into tab trigger loop"
```
