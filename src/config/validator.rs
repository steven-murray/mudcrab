use crate::config::schema::{InputType, ModAction, ModEntry, ModType, SourceModlist};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

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

    // Plugins some merge with `hide_sources` swallows. Collected first because
    // the per-mod check below has to know about a merge declared further down
    // the file than the mod whose plugin it takes.
    let merged_away: HashSet<String> = flattened_mods
        .values()
        .filter_map(|spec| spec.merge.as_ref())
        .filter(|merge| merge.hide_sources)
        .flat_map(|merge| {
            merge
                .sources
                .iter()
                .map(|source| source.plugin.to_ascii_lowercase())
        })
        .collect();

    for (mod_id, spec) in &flattened_mods {
        for dep in &spec.dependencies {
            if !flattened_mods.contains_key(dep) {
                anyhow::bail!("mod {mod_id} has unknown dependency {dep}");
            }
        }

        // A conflict relationship names another mod, and naming it wrong is the
        // failure that cost this list three rebuilds: the guide's prose names
        // for the mods in one row were wrong three separate ways. Caught here,
        // where a typo is a validation error rather than a conflict list that
        // comes back empty two hours into a build.
        for named in conflict_partners(spec) {
            if !flattened_mods.contains_key(named) {
                anyhow::bail!(
                    "mod {mod_id} has a conflicts_with naming {named}, which is not a mod in this \
                     modlist"
                );
            }
            if named == mod_id {
                anyhow::bail!("mod {mod_id} declares a conflicts_with against itself");
            }
        }

        for plugin in &spec.plugins {
            if !is_plugin_filename(plugin) {
                anyhow::bail!("mod {mod_id} declares invalid plugin filename {plugin}");
            }
            // A plugin a merge consumes is deliberately absent from the load
            // order -- `validate_merges` rejects it for being there. The mod
            // still declares it, because the mod really does ship it, and that
            // is worth recording whether or not it survives to the load order.
            if merged_away.contains(&plugin.to_ascii_lowercase()) {
                continue;
            }
            if !modlist.plugins.iter().any(|entry| entry == plugin) {
                anyhow::bail!(
                    "mod {mod_id} declares plugin {plugin} that is missing from global plugins \
                     load order, and no merge consumes it"
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

    // Oblivion indexes a plugin's FormIDs by a single byte, so a load order
    // cannot hold more than 255 active plugins. Past that the game does not
    // complain: it loads the first 255 and the rest are simply not there, which
    // looks like a mod that failed to install rather than a list that is too
    // long. Merges are how a list gets back under -- so the number to report is
    // the one that names them.
    if modlist.plugins.len() > PLUGIN_LIMIT {
        anyhow::bail!(
            "load order has {} active plugins, {} over Oblivion's limit of {PLUGIN_LIMIT}. \
             The game loads the first {PLUGIN_LIMIT} and silently ignores the rest. Merge some \
             plugins, or drop them from the `plugins` array.",
            modlist.plugins.len(),
            modlist.plugins.len() - PLUGIN_LIMIT,
        );
    }

    validate_merges(modlist, &flattened_mods)?;
    detect_cycles(&flattened_mods)?;

    Ok(())
}

/// Active plugins Oblivion can load. The mod index is one byte, and `0xFF` is
/// reserved for the save's own in-memory records.
const PLUGIN_LIMIT: usize = 255;

/// Every mod id a mod's actions declare a conflict against.
fn conflict_partners(spec: &ModEntry) -> impl Iterator<Item = &String> {
    spec.actions.iter().flat_map(|action| match action {
        ModAction::FilePrune(prune) => prune.conflicts_with.iter(),
        ModAction::FileHide(hide) => hide.conflicts_with.iter(),
        _ => [].iter(),
    })
}

/// Check every merge against the rest of the modlist.
///
/// A merge is only correct in the context of the whole list -- its sources must
/// exist, its output must be in the load order, and the plugins it consumes
/// must *not* be, because hiding them is what makes the merge take effect. All
/// of those are cheap to check here and expensive to discover in game.
fn validate_merges(
    modlist: &SourceModlist,
    mods: &IndexMap<String, ModEntry>,
) -> anyhow::Result<()> {
    let in_load_order = |plugin: &str| {
        modlist
            .plugins
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(plugin))
    };

    // Plugin -> the merge that already consumes it. Merging one plugin twice
    // would duplicate every record it contains.
    let mut claimed: HashMap<String, String> = HashMap::new();

    for (mod_id, spec) in mods {
        match (spec.mod_type, &spec.merge) {
            (Some(ModType::Merge), None) => anyhow::bail!(
                "mod {mod_id} has type = \"merge\" but no [mods.merge] section saying what to merge"
            ),
            (mod_type, Some(_)) if mod_type != Some(ModType::Merge) => anyhow::bail!(
                "mod {mod_id} has a [mods.merge] section but is not type = \"merge\", so the \
                 merge would never run"
            ),
            _ => {}
        }

        let Some(merge) = &spec.merge else { continue };

        if !spec.archives.is_empty() || !spec.files.is_empty() {
            anyhow::bail!(
                "merge {mod_id} also declares archives or files; a merge mod contains only its \
                 merged output"
            );
        }

        if !is_plugin_filename(&merge.output) {
            anyhow::bail!("merge {mod_id} has invalid output filename {}", merge.output);
        }
        if !in_load_order(&merge.output) {
            anyhow::bail!(
                "merge {mod_id} produces {} which is missing from the global plugins load order",
                merge.output
            );
        }

        if merge.sources.is_empty() {
            anyhow::bail!("merge {mod_id} lists no source plugins");
        }

        for source in &merge.sources {
            if !mods.contains_key(&source.mod_id) {
                anyhow::bail!(
                    "merge {mod_id} sources plugin {} from unknown mod {}",
                    source.plugin,
                    source.mod_id
                );
            }
            if !is_plugin_filename(&source.plugin) {
                anyhow::bail!(
                    "merge {mod_id} sources invalid plugin filename {}",
                    source.plugin
                );
            }
            if merge.hide_sources && in_load_order(&source.plugin) {
                anyhow::bail!(
                    "merge {mod_id} sources {}, which is still in the global plugins load order. \
                     Merged sources are hidden from the plugin list; remove it from `plugins`.",
                    source.plugin
                );
            }

            let key = source.plugin.to_ascii_lowercase();
            if let Some(other) = claimed.insert(key, mod_id.clone()) {
                anyhow::bail!(
                    "plugin {} is merged by both {other} and {mod_id}; every record in it would \
                     appear twice",
                    source.plugin
                );
            }
        }
    }

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
