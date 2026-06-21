mod config;
mod note;
mod notifier;
mod state;
mod watcher;

use std::path::PathBuf;

use anyhow::Result;

use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config_path = std::env::var("HAKUREN_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
    let path = PathBuf::from(&config_path);
    if !path.exists() {
        anyhow::bail!(
            "config not found: {}\ncopy config.example.toml to {} and fill it in",
            path.display(),
            config_path
        );
    }

    let config = Config::load(&path)?;
    watcher::run(config).await
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("hakuren_bot=info,warn"));
    fmt().with_env_filter(filter).with_target(false).init();
}
