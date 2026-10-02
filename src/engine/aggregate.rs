use std::collections::HashSet;

use crate::engine::lexer::{
    active_token_raw, lex_command_line, open_quote, quoted_tail, token_tail,
};
use crate::engine::provider::{CompletionProvider, Suggestion, SuggestionKind};
use crate::engine::providers::carapace::CarapaceProvider;
use crate::engine::providers::json_spec::JsonSpecProvider;
use crate::engine::providers::zoxide::ZoxideProvider;
use crate::engine::replacement::{replace_range, ReplacementAction};
use crate::shell::report::ShellReport;

/// Determines whether file suggestions should be shown next to `existing` suggestions.
///
/// When subcommands, commands, cmdlets or options were found, files are only relevant if the
/// active token looks like a path (contains `/` or `\`, or starts with `.`).
pub fn should_include_files<'a>(
    active_token: &str,
    existing: impl IntoIterator<Item = &'a Suggestion>,
) -> bool {
    let has_gating_suggestion = existing.into_iter().any(|s| {
        matches!(
            s.kind,
            SuggestionKind::Subcommand
                | SuggestionKind::Command
                | SuggestionKind::PowerShellCmdlet
                | SuggestionKind::Option
        )
    });

    if !has_gating_suggestion {
        return true;
    }

    active_token.contains('/') || active_token.contains('\\') || active_token.starts_with('.')
}

/// Aliases PowerShell defines out of the box (completion reports them as plain commands).
const BUILTIN_ALIASES: &[&str] = &[
    "cat", "cd", "chdir", "clc", "clear", "clhy", "cli", "clp", "cls", "clv", "cnsn", "compare",
    "copy", "cp", "cpi", "cpp", "cvpa", "dbp", "del", "diff", "dir", "dnsn", "ebp", "echo", "epal",
    "epcsv", "erase", "etsn", "exsn", "fc", "fhx", "fl", "foreach", "ft", "fw", "gal", "gbp", "gc",
    "gcb", "gci", "gcm", "gcs", "gdr", "gerr", "ghy", "gi", "gin", "gjb", "gl", "gm", "gmo", "gp",
    "gps", "gpv", "group", "gsn", "gsv", "gtz", "gu", "gv", "h", "history", "icm", "iex", "ihy",
    "ii", "ipal", "ipcsv", "ipmo", "irm", "iwr", "kill", "ls", "man", "md", "measure", "mi",
    "mount", "move", "mp", "mv", "nal", "ndr", "ni", "nmo", "nsn", "nv", "ogv", "oh", "popd", "ps",
    "pushd", "pwd", "r", "rbp", "rcjb", "rcsn", "rd", "rdr", "ren", "ri", "rjb", "rm", "rmdir",
    "rmo", "rni", "rnp", "rp", "rsn", "rv", "rvpa", "sajb", "sal", "saps", "sasv", "sbp", "scb",
    "select", "set", "shcm", "si", "sl", "sleep", "sls", "sort", "sp", "spjb", "spps", "spsv",
    "start", "stz", "sv", "tee", "type", "where", "wjb", "write",
];

/// Whether `name` is safe to type into the shell.
///
/// The text is written to the PTY as-is, so a control character would be interpreted as a key:
/// a CR or LF in a match (a multi-line history entry, or a spoofed report) submits the
/// half-inserted line, and an ESC starts an escape sequence.
fn is_insertable(name: &str) -> bool {
    !name.chars().any(|c| c < ' ' || c == '\x7f')
}

/// Turns PowerShell's completion matches into suggestions.
pub fn shell_suggestions(report: &ShellReport) -> Vec<Suggestion> {
    report
        .matches
        .iter()
        .filter(|m| is_insertable(&m.0))
        .map(|m| {
            let (text, list_item, result_type, tooltip) = (&m.0, &m.1, &m.2, &m.3);
            let (kind, priority) = match result_type.as_str() {
                "Command" if text.contains('-') => (SuggestionKind::PowerShellCmdlet, 80),
                "Command" if BUILTIN_ALIASES.contains(&text.to_lowercase().as_str()) => {
                    (SuggestionKind::Alias, 80)
                }
                "Command" => (SuggestionKind::Command, 80),
                "ParameterName" => (SuggestionKind::Option, 75),
                "ProviderContainer" => (SuggestionKind::Directory, 60),
                "ProviderItem" => (SuggestionKind::File, 50),
                _ => (SuggestionKind::Other, 70),
            };
            let description = tooltip
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .filter(|l| l != text && l != list_item)
                .map(str::to_string);
            let display = if list_item.is_empty() {
                text.clone()
            } else {
                list_item.clone()
            };
            Suggestion::new(text.clone(), display, description, priority)
                .with_kind(kind)
                .with_shell_range()
        })
        .collect()
}

/// `src/`, `.\src\` and `'.\src\'` are the same entry for de-duplication.
fn dedupe_key(name: &str) -> String {
    let unquoted = name.trim_matches(['\'', '"']);
    let normalized = unquoted.replace('\\', "/").to_lowercase();
    normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .to_string()
}

/// Merges suggestions of the external sources (specs, carapace, zoxide) with PowerShell's:
/// highest priority first, one entry per name, and PowerShell's files only where files make sense.
pub fn merge_suggestions(
    active_token: &str,
    external: Vec<Suggestion>,
    shell: Vec<Suggestion>,
) -> Vec<Suggestion> {
    let include_files = should_include_files(active_token, external.iter());
    let mut results = external;
    results.extend(shell.into_iter().filter(|s| {
        include_files || !matches!(s.kind, SuggestionKind::File | SuggestionKind::Directory)
    }));
    // No source may hand the PTY a control character, not even an external provider.
    results.retain(|s| is_insertable(&s.name));

    // Stable sort: on equal priority and name the external source (added first) wins.
    results.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.name.cmp(&b.name))
    });
    let mut seen = HashSet::new();
    results.retain(|s| seen.insert(dedupe_key(&s.name)));
    results
}

/// The result type PowerShell reported for the match `suggestion` was built from.
fn shell_result_type<'a>(report: &'a ShellReport, suggestion: &Suggestion) -> Option<&'a str> {
    if !suggestion.uses_shell_range {
        return None;
    }
    report
        .matches
        .iter()
        .find(|m| m.0 == suggestion.name)
        .map(|m| m.2.as_str())
}

/// What a completion needs after it that PowerShell does not supply, or `None` to keep the
/// ordinary trailing space.
///
/// A type literal is reported without the `]` that closes it, so the bracket is added here. A
/// namespace is only a step on the way to a type, so the cursor stays right behind it.
fn missing_suffix(
    report: &ShellReport,
    range: Option<(usize, usize)>,
    result_type: &str,
) -> Option<&'static str> {
    match result_type {
        "Namespace" => Some(""),
        "Type" => {
            let (start, end) = range?;
            let opened = report.line[..start].ends_with('[');
            let closed = report.line[end..].starts_with(']');
            (opened && !closed).then_some("]")
        }
        _ => None,
    }
}

/// Keys that replace the text around the cursor with `suggestion`.
pub fn plan_replacement(report: &ShellReport, suggestion: &Suggestion) -> ReplacementAction {
    let cursor = report.cursor_byte().unwrap_or(report.line.len());
    let range = report.replacement_range();
    // `action` and the text right behind the range it replaces.
    let (mut action, behind) = match range {
        Some((start, end)) if suggestion.uses_shell_range => (
            replace_range(
                &report.line[start..cursor],
                &report.line[cursor..end],
                &suggestion.name,
            ),
            report.line.get(end..).unwrap_or(""),
        ),
        _ => {
            let before = active_token_raw(&report.line[..cursor]);
            let after = &report.line[cursor..];
            let tail = match open_quote(before) {
                Some(quote) => quoted_tail(after, quote),
                None => token_tail(after),
            };
            (
                replace_range(before, tail, &suggestion.name),
                &after[tail.len()..],
            )
        }
    };
    if let Some(suffix) = shell_result_type(report, suggestion)
        .and_then(|result_type| missing_suffix(report, range, result_type))
    {
        action.insert_text = format!("{}{}", action.insert_text.trim_end_matches(' '), suffix);
    }
    // A space already follows the replaced range: a second one would be typed into the line.
    if behind.starts_with([' ', '\t']) {
        action
            .insert_text
            .truncate(action.insert_text.trim_end_matches(' ').len());
    }
    action
}

/// Queries the external providers and merges their results with the shell's own completions.
#[derive(Clone)]
pub struct CompletionEngine {
    json_spec: JsonSpecProvider,
    zoxide: ZoxideProvider,
    carapace: CarapaceProvider,
}

impl CompletionEngine {
    pub fn new(json_spec: JsonSpecProvider) -> Self {
        Self {
            json_spec,
            zoxide: ZoxideProvider::default(),
            carapace: CarapaceProvider::default(),
        }
    }

    /// Suggestions for the text before the cursor of `report`.
    pub async fn complete(&self, report: &ShellReport, cwd: &str) -> Vec<Suggestion> {
        let text = report.text_before_cursor().unwrap_or(&report.line);
        let tokens = lex_command_line(text);
        let root_cmd = tokens.first().map(|t| t.text.as_str()).unwrap_or("");
        let active_token = tokens.last().map(|t| t.text.as_str()).unwrap_or("");

        let (spec, zoxide, carapace) = tokio::join!(
            run_provider(&self.json_spec, root_cmd, text, cwd),
            run_provider(&self.zoxide, root_cmd, text, cwd),
            run_provider(&self.carapace, root_cmd, text, cwd),
        );

        let external = spec.into_iter().chain(zoxide).chain(carapace).collect();
        merge_suggestions(active_token, external, shell_suggestions(report))
    }
}

async fn run_provider(
    provider: &dyn CompletionProvider,
    root_cmd: &str,
    text: &str,
    cwd: &str,
) -> Vec<Suggestion> {
    if provider.can_handle(root_cmd) {
        provider.complete(text, cwd).await
    } else {
        Vec::new()
    }
}
