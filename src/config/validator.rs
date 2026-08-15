use crate::config::schema::{InputType, ModEntry, SourceModlist};
use indexmap::IndexMap;
use std::collections::HashSet;

pub fn validate(modlist: &SourceModlist) -> anyhow::Result<()> {
    if modlist.name.trim().is_empty() {
        anyhow::bail!("modlist name must not be empty");
    }

    for (id, spec) in &modlist.inputs {
        if spec.query.trim().is_empty() {
            anyhow::bail!("input {id} must include a non-empty query");
        }

        if spec.input_type == InputType::Choice && spec.choices.is_empty() {
            anyhow::bail!("input {id} is choice type and must include choices");
        }
    }

    for (key, value) in &modlist.ini {
        match value {
            toml::Value::String(_)
            | toml::Value::Integer(_)
            | toml::Value::Float(_)
            | toml::Value::Boolean(_) => {}
            _ => anyhow::bail!(
                "top-level ini entry {key} must be a string, integer, float, or boolean"
            ),
        }
    }

    let flattened_mods = modlist.flatten_mods()?;

    for (mod_id, spec) in &flattened_mods {
        for dep in &spec.dependencies {
            if !flattened_mods.contains_key(dep) {
                anyhow::bail!("mod {mod_id} has unknown dependency {dep}");
            }
        }

        for plugin in &spec.plugins {
            if !is_plugin_filename(plugin) {
                anyhow::bail!("mod {mod_id} declares invalid plugin filename {plugin}");
            }
            if !modlist.plugins.iter().any(|entry| entry == plugin) {
                anyhow::bail!(
                    "mod {mod_id} declares plugin {plugin} that is missing from global plugins load order"
                );
            }
        }
    }

    let mut seen_plugins = HashSet::new();
    for plugin in &modlist.plugins {
        if !is_plugin_filename(plugin) {
            anyhow::bail!("global plugins load order contains invalid plugin filename {plugin}");
        }
        if !seen_plugins.insert(plugin.to_ascii_lowercase()) {
            anyhow::bail!("global plugins load order contains duplicate plugin {plugin}");
        }
    }

    detect_cycles(&flattened_mods)?;

    Ok(())
}

fn is_plugin_filename(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.ends_with(".esp") || lower.ends_with(".esm")
}

fn detect_cycles(mods: &IndexMap<String, ModEntry>) -> anyhow::Result<()> {
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();

    for mod_id in mods.keys() {
        visit(mods, mod_id, &mut visiting, &mut visited)?;
    }

    Ok(())
}

fn visit(
    mods: &IndexMap<String, ModEntry>,
    mod_id: &str,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> anyhow::Result<()> {
    if visited.contains(mod_id) {
        return Ok(());
    }

    if !visiting.insert(mod_id.to_string()) {
        anyhow::bail!("dependency cycle detected at mod {mod_id}");
    }

    if let Some(spec) = mods.get(mod_id) {
        for dep in &spec.dependencies {
            visit(mods, dep, visiting, visited)?;
        }
    }

    visiting.remove(mod_id);
    visited.insert(mod_id.to_string());

    Ok(())
}
