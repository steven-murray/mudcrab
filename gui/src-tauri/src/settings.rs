//! What the GUI remembers between runs: the lists opened, and where each was
//! installed. Kept in `gui.toml` beside `tools.toml`, never in a list, so a
//! list stays the same file however many people install it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Most recent first.
    #[serde(default)]
    pub recent: Vec<PathBuf>,
    /// Per list, by its path.
    #[serde(default)]
    pub lists: BTreeMap<String, ListSettings>,
    /// The tools.toml to use, when not the usual one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools_config: Option<PathBuf>,
}

/// Where one list goes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Other folders that may already hold the list's archives, looked in
    /// before anything is downloaded (`download --archive-search-path`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search: Vec<PathBuf>,
    /// The answers last given to the list's questions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub answers: BTreeMap<String, String>,
}

/// `gui.toml`, beside the usual `tools.toml` (or the one
/// `MUDCRAB_TOOLS_CONFIG` names) -- not beside a tools.toml chosen in the
/// GUI, which would move the GUI's own settings with it.
pub fn path() -> Option<PathBuf> {
    std::env::var_os("MUDCRAB_TOOLS_CONFIG")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(mudcrab::config::tools::ToolsConfig::standard_path)
        .map(|tools| tools.with_file_name("gui.toml"))
}

impl Settings {
    /// What was saved, or nothing; a file that cannot be read is not fatal.
    pub fn load() -> Self {
        path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = path().ok_or_else(|| anyhow::anyhow!("no place for gui.toml"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)
            .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
    }

    /// Put `list` first among the recent lists.
    pub fn opened(&mut self, list: &std::path::Path) {
        self.recent.retain(|p| p != list);
        self.recent.insert(0, list.to_path_buf());
        self.recent.truncate(10);
    }

    pub fn list(&self, list: &std::path::Path) -> ListSettings {
        self.lists
            .get(&list.display().to_string())
            .cloned()
            .unwrap_or_default()
    }

    pub fn list_mut(&mut self, list: &std::path::Path) -> &mut ListSettings {
        self.lists.entry(list.display().to_string()).or_default()
    }
}
