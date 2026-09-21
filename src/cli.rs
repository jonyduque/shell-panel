use std::path::PathBuf;

use clap::Parser;

/// Command-line arguments for shell-panel.
#[derive(Parser, Debug)]
#[command(name = "shell-panel", version, author, about, long_about = None)]
pub struct Cli {
    /// Shell to run (pwsh or powershell). Defaults to pwsh.exe when it is on PATH.
    #[arg(short, long)]
    pub shell: Option<String>,

    /// Enable verbose debug logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Check if currently running inside a shell-panel session
    #[arg(short, long)]
    pub check: bool,

    /// Path to TOML configuration file (defaults to ~/.config/shell-panel.toml)
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Print default sample configuration in TOML format
    #[arg(long)]
    pub print_default_config: bool,
}
