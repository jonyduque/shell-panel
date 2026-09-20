use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod core;
mod engine;
mod io;
mod pty;
mod shell;
mod vt;
mod ui;

#[derive(Parser, Debug)]
#[command(name = "shell-panel")]
#[command(author = "jonyd")]
#[command(version = "0.1.0")]
#[command(about = "IDE-style command line auto-complete panel for Windows PowerShell", long_about = None)]
pub struct Cli {
    /// Shell to run (pwsh, powershell). Defaults to auto-detect.
    #[arg(short, long)]
    pub shell: Option<String>,

    /// Enable verbose debug logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Check if currently running inside a shell-panel session
    #[arg(short, long)]
    pub check: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let in_session = std::env::var("ISTERM").unwrap_or_default() == "1";
    if cli.check {
        if in_session {
            println!("shell-panel session active.");
            std::process::exit(0);
        } else {
            println!("Not in a shell-panel session.");
            std::process::exit(1);
        }
    }

    // Configure logging
    let filter = if cli.verbose {
        tracing_subscriber::EnvFilter::new("shell_panel=debug")
    } else {
        tracing_subscriber::EnvFilter::new("shell_panel=info")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let config = core::config::Config::default();
    let mut app = core::app::App::new(config);
    app.run().await?;

    Ok(())
}
