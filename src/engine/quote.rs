/// Wraps `path` in single quotes when PowerShell would otherwise split or interpret it.
pub fn quote_for_powershell(path: &str) -> String {
    let needs_quotes = path
        .chars()
        .any(|c| c.is_whitespace() || "'\"`$(){};,&@#|<>".contains(c));
    if needs_quotes {
        format!("'{}'", path.replace('\'', "''"))
    } else {
        path.to_string()
    }
}
