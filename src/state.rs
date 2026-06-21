use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub creators: HashMap<String, CreatorState>,
    #[serde(skip)]
    path: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CreatorState {
    #[serde(default)]
    pub initialized: bool,
    #[serde(default)]
    pub seen_keys: HashSet<String>,
}

impl State {
    pub fn load(path: &Path) -> Result<Self> {
        let mut state = if path.exists() {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read state {}", path.display()))?;
            serde_json::from_str(&text)
                .with_context(|| format!("failed to parse state {}", path.display()))?
        } else {
            State::default()
        };
        state.path = path.to_path_buf();
        Ok(state)
    }

    pub fn save(&self) -> Result<()> {
        let text = serde_json::to_string_pretty(self).context("failed to serialize state")?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text).with_context(|| format!("failed to write {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("failed to replace {}", self.path.display()))?;
        Ok(())
    }

    pub fn creator_mut(&mut self, urlname: &str) -> &mut CreatorState {
        self.creators.entry(urlname.to_string()).or_default()
    }
}
