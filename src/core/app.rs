use crate::core::config::Config;
use anyhow::Result;

pub struct App {
    pub config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("ShellPanel iniciado com sucesso.");
        Ok(())
    }
}
