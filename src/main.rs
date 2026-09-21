use std::path::PathBuf;

use clap::Parser;
use shell_panel::cli::Cli;
use shell_panel::core;
use shell_panel::pty::conpty::SESSION_ENV;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Sends `tracing` output to a log file: the terminal is in raw mode and owned by the shell.
fn init_file_logging() -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join("shell-panel");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("shell-panel.log");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
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
        let path = init_file_logging()?;
        eprintln!("shell-panel: writing debug log to {}", path.display());
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

    let config = core::config::Config::load_or_default(cli.config.as_deref());
    let shell = cli.shell.or_else(|| config.shell.clone());
    let mut app = core::app::App::new(config, shell);
    app.no_profile = cli.no_profile;
    let exit_code = app.run().await?;

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }

    Ok(())
}
