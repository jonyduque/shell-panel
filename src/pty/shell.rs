use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Pwsh,
    Powershell,
}

impl ShellType {
    /// Returns the executable name for the shell.
    pub fn executable_name(&self) -> &'static str {
        match self {
            ShellType::Pwsh => "pwsh.exe",
            ShellType::Powershell => "powershell.exe",
        }
    }
}

/// Detects the PowerShell shell type to use.
///
/// If `override_shell` is provided:
/// - Returns `ShellType::Powershell` if "powershell" (case-insensitive).
/// - Otherwise returns `ShellType::Pwsh`.
///
/// If `override_shell` is None:
/// - Searches PATH for `pwsh.exe`. If found, returns `ShellType::Pwsh`.
/// - Otherwise returns `ShellType::Powershell`.
pub fn detect_shell(override_shell: Option<&str>) -> ShellType {
    if let Some(s) = override_shell {
        if s.eq_ignore_ascii_case("powershell") {
            ShellType::Powershell
        } else {
            ShellType::Pwsh
        }
    } else {
        if let Some(path_var) = env::var_os("PATH") {
            for dir in env::split_paths(&path_var) {
                let candidate = dir.join("pwsh.exe");
                if candidate.is_file() {
                    return ShellType::Pwsh;
                }
            }
        }
        ShellType::Powershell
    }
}
