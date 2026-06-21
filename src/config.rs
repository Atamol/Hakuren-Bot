use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_interval")]
    pub poll_interval_secs: u64,
    #[serde(default)]
    pub notify_on_first_run: bool,
    #[serde(default = "default_state_path")]
    pub state_path: String,
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub discord_username: Option<String>,
    pub watch: Vec<Watch>,
}

#[derive(Debug, Deserialize)]
pub struct Watch {
    pub urlname: String,
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub discord_username: Option<String>,
    #[serde(default)]
    pub mention: Option<String>,
}

fn default_interval() -> u64 {
    300
}

fn default_state_path() -> String {
    "state.json".to_string()
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        let mut cfg: Config = toml::from_str(&text).context("failed to parse config TOML")?;

        if non_empty(&cfg.webhook_url).is_none() {
            if let Ok(v) = std::env::var("DISCORD_WEBHOOK_URL") {
                if !v.is_empty() {
                    cfg.webhook_url = Some(v);
                }
            }
        }

        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<()> {
        if self.watch.is_empty() {
            anyhow::bail!("no [[watch]] entries in config");
        }
        for w in &self.watch {
            if w.urlname.trim().is_empty() {
                anyhow::bail!("a [[watch]] entry has an empty urlname");
            }
            if self.webhook_for(w).is_empty() {
                anyhow::bail!(
                    "watch '{}' has no webhook_url and no top-level webhook_url is set",
                    w.urlname
                );
            }
        }
        Ok(())
    }

    pub fn webhook_for<'a>(&'a self, w: &'a Watch) -> &'a str {
        non_empty(&w.webhook_url)
            .or_else(|| non_empty(&self.webhook_url))
            .unwrap_or("")
    }

    pub fn username_for<'a>(&'a self, w: &'a Watch) -> Option<&'a str> {
        non_empty(&w.discord_username).or_else(|| non_empty(&self.discord_username))
    }
}

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|v| !v.is_empty())
}
