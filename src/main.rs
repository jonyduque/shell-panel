use std::path::{Path, PathBuf};

use clap::Parser;
use shell_panel::cli::Cli;
use shell_panel::core;
use shell_panel::pty::conpty::SESSION_ENV;
use shell_panel::pty::shell::is_supported_shell;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Opens (creating if necessary) the log file inside `dir`, returning its path and handle.
/// Split out of [`init_file_logging`] so the fallible I/O can be unit-tested without touching
/// the process-global `tracing` subscriber.
fn open_log_file(dir: &Path) -> anyhow::Result<(PathBuf, std::fs::File)> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("shell-panel.log");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    Ok((path, file))
}

/// Sends `tracing` output to a log file: the terminal is in raw mode and owned by the shell.
fn init_file_logging() -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join("shell-panel");
    let (path, file) = open_log_file(&dir)?;
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("shell_panel=debug"))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(file)),
        )
        .init();
    Ok(path)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.print_default_config {
        println!("{}", core::config::default_sample_toml());
        return Ok(());
    }

    let in_session = std::env::var(SESSION_ENV).as_deref() == Ok("1");
    if cli.check {
        if in_session {
            println!("shell-panel session active.");
            std::process::exit(0);
        }
        println!("Not in a shell-panel session.");
        std::process::exit(1);
    }

    if in_session {
        eprintln!("shell-panel: already running in this terminal ({SESSION_ENV}=1); refusing to start a nested session.");
        std::process::exit(1);
    }

    if cli.verbose {
        match init_file_logging() {
            Ok(path) => eprintln!("shell-panel: writing debug log to {}", path.display()),
            // Logging is diagnostic only: an unwritable log directory must not block startup.
            Err(err) => eprintln!("shell-panel: could not start file logging: {err}"),
        }
    }

    // The shell's output is raw VT. Windows Terminal always interprets it; a legacy console
    // window only does after this call (it enables ENABLE_VIRTUAL_TERMINAL_PROCESSING).
    #[cfg(windows)]
    let _ = crossterm::ansi_support::supports_ansi();

    // Restore the console if we panic while it is in raw mode.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
        default_hook(info);
    }));

    let (config, config_warnings) = core::config::Config::load(cli.config.as_deref());
    for warning in config_warnings {
        eprintln!("shell-panel: {warning}");
    }

    let shell = cli.shell.or_else(|| config.shell.clone());
    if let Some(name) = shell.as_deref() {
        if !is_supported_shell(name) {
            eprintln!("shell-panel: unsupported shell {name:?}; use \"pwsh\" or \"powershell\"");
            std::process::exit(2);
        }
    }

    let mut app = core::app::App::new(config, shell);
    app.no_profile = cli.no_profile;
    let exit_code = app.run().await?;

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_log_file_reports_error_for_unwritable_directory() {
        // A regular file cannot be used as a directory: create_dir_all must fail under it.
        let blocker = std::env::temp_dir().join(format!("sp_log_blocker_{}", std::process::id()));
        std::fs::write(&blocker, b"not a directory").unwrap();
        let dir = blocker.join("shell-panel");

        let result = open_log_file(&dir);

        let _ = std::fs::remove_file(&blocker);
        assert!(result.is_err());
    }
}
