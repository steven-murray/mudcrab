use crate::config::schema::SourceModlist;
use std::path::Path;

pub fn load_modlist(path: &Path) -> anyhow::Result<SourceModlist> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;

    let parsed = toml::from_str::<SourceModlist>(&raw)
        .map_err(|err| anyhow::anyhow!("failed to parse TOML {}: {err}", path.display()))?;

    Ok(parsed)
}
