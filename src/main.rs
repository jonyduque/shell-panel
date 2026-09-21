use clap::Parser;
use shell_panel::cli::Cli;
use shell_panel::core;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.print_default_config {
        println!("{}", core::config::default_sample_toml());
        std::process::exit(0);
    }

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

    // Register panic hook to restore normal terminal mode and show cursor
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
        default_hook(info);
    }));

    let config = core::config::Config::load_or_default(cli.config.as_deref());
    let shell = cli.shell.or_else(|| config.shell.clone());
    let mut app = core::app::App::new(config, shell);
    let exit_code = app.run().await?;

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }

    Ok(())
}
