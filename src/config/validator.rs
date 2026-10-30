use crate::config::condition::Condition;
use crate::config::schema::{
    BainSubpackage, InputType, InputValue, ModAction, ModEntry, ModType, SourceModlist,
};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// Check a modlist, returning anything worth saying that is not fatal.
///
/// The split matters: a modlist that cannot be built is an error, and a modlist
/// that can be built but will not behave as its author expects is a warning.
/// `--strict` is what turns the second into the first.
pub fn validate(modlist: &SourceModlist) -> anyhow::Result<Vec<String>> {
    validate_list(modlist).map_err(|err| locate(err, modlist))
}

/// `err`, saying which included file the mod it is about is written in, so
/// the fix is made in the right file.
fn locate(err: anyhow::Error, modlist: &SourceModlist) -> anyhow::Error {
    let message = err.to_string();
    let base = modlist
        .base_dir
        .as_deref()
        .unwrap_or(std::path::Path::new(""));
    let found = modlist.mods.iter().find_map(|entry| {
        let origin = entry.origin.as_ref()?;
        let id = &entry.id;
        [
            format!("mod {id} "),
            format!("mod {id}:"),
            format!("mod '{id}'"),
        ]
        .iter()
        .any(|named| message.contains(named.as_str()))
        .then(|| (id, crate::config::compiler::origin_name(base, origin)))
    });
    match found {
        Some((id, origin)) => anyhow::anyhow!("{message} (mod {id} is in {origin})"),
        None => err,
    }
}

fn validate_list(modlist: &SourceModlist) -> anyhow::Result<Vec<String>> {
    if modlist.name.trim().is_empty() && !modlist.component {
        anyhow::bail!("modlist name must not be empty");
    }
    for step in &modlist.manual_steps {
        anyhow::ensure!(
            !step.title.trim().is_empty() && !step.steps.trim().is_empty(),
            "a manual step needs a title and its steps"
        );
    }

    for (id, spec) in &modlist.inputs {
        if spec.query.trim().is_empty() {
            anyhow::bail!("input {id} must include a non-empty query");
        }

        if spec.input_type == InputType::Choice && spec.choices.is_empty() {
            anyhow::bail!("input {id} is choice type and must include choices");
        }

        match (&spec.default, spec.input_type) {
            (None, _)
            | (Some(InputValue::Bool(_)), InputType::Bool)
            | (Some(InputValue::Text(_)), InputType::Text) => {}
            (Some(InputValue::Text(value)), InputType::Choice) => {
                if !spec.choices.contains(value) {
                    anyhow::bail!(
                        "input {id}: default {value:?} is not one of its choices {:?}",
                        spec.choices
                    );
                }
            }
            (Some(_), input_type) => {
                anyhow::bail!("input {id}: default does not match its type ({input_type:?})")
            }
        }
    }

    // `key = value`, or `[ini.<Section>]` holding them.
    let scalar = |value: &toml::Value| {
        matches!(
            value,
            toml::Value::String(_)
                | toml::Value::Integer(_)
                | toml::Value::Float(_)
                | toml::Value::Boolean(_)
        )
    };
    for (key, value) in &modlist.ini {
        match value {
            toml::Value::Table(keys) => {
                for (inner, value) in keys {
                    anyhow::ensure!(
                        scalar(value),
                        "ini entry [ini.{key}] {inner} must be a string, integer, float, or boolean"
                    );
                }
            }
            value => anyhow::ensure!(
                scalar(value),
                "top-level ini entry {key} must be a string, integer, float, or boolean, or a \
                 [ini.{key}] section of them"
            ),
        }
    }

    let flattened_mods = modlist.flatten_mods()?;

    // A condition naming an input or mod that does not exist would evaluate
    // to false and silently drop what it guards.
    let mut mod_condition_refs: HashMap<&str, Vec<String>> = HashMap::new();
    for (mod_id, spec) in &flattened_mods {
        let conditions = spec
            .condition
            .iter()
            .map(|expr| (expr, "its `if`".to_string(), true))
            .chain(spec.actions.iter().filter_map(|action| {
                action
                    .condition
                    .as_ref()
                    .map(|expr| (expr, format!("its {} action", action.action.name()), false))
            }))
            .chain(spec.archives.iter().flat_map(|archive| {
                let label = archive.path.as_deref().unwrap_or("(build)").to_string();
                archive
                    .condition
                    .iter()
                    .map({
                        let label = label.clone();
                        move |expr| (expr, format!("its archive {label}"), false)
                    })
                    .chain(archive.fomod_selections.iter().filter_map({
                        let label = label.clone();
                        move |selection| {
                            selection.condition.as_ref().map(|expr| {
                                let site = format!(
                                    "the FOMOD selection {} / {} of {label}",
                                    selection.step, selection.group
                                );
                                (expr, site, false)
                            })
                        }
                    }))
                    .chain(
                        archive
                            .bain_subpackages
                            .iter()
                            .filter_map(move |subpackage| {
                                let BainSubpackage::Conditional { name, condition } = subpackage
                                else {
                                    return None;
                                };
                                Some((
                                    condition,
                                    format!("the BAIN subpackage {name} of {label}"),
                                    false,
                                ))
                            }),
                    )
            }));
        for (expr, site, gates_mod) in conditions {
            let condition = Condition::parse(expr)
                .map_err(|err| anyhow::anyhow!("mod {mod_id}: {site}: {err:#}"))?;
            // A component's conditions may ask the including list's
            // questions, which it cannot see.
            if let Some(input) = condition
                .inputs()
                .into_iter()
                .find(|i| !modlist.inputs.contains_key(*i))
                && !modlist.component
            {
                anyhow::bail!(
                    "mod {mod_id}: {site} asks about input '{input}', which [inputs] does not declare"
                );
            }
            for other in condition.mods() {
                if !flattened_mods.contains_key(other) {
                    anyhow::bail!(
                        "mod {mod_id}: {site} asks about mod '{other}', which is not a mod in this modlist"
                    );
                }
                if gates_mod && other == mod_id {
                    anyhow::bail!("mod {mod_id}: its `if` asks about the mod itself");
                }
            }
            if gates_mod {
                mod_condition_refs.insert(
                    mod_id.as_str(),
                    condition.mods().into_iter().map(str::to_string).collect(),
                );
            }
        }
    }
    reject_condition_cycles(&mod_condition_refs)?;

    validate_docs(modlist, &flattened_mods)?;

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
            // MO2 does not see an unticked mod's plugins, so they have no
            // place in the load order: it adds them at the end, unticked
            // itself, when the player switches the mod on.
            if !spec.enabled {
                if modlist
                    .plugins
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(plugin))
                {
                    anyhow::bail!(
                        "mod {mod_id} is not enabled, but its plugin {plugin} is in the global \
                         plugins load order. MO2 cannot see it there; take it out of `plugins`."
                    );
                }
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

    // An inactive plugin still has a load-order slot, so it must be declared in
    // `plugins` -- otherwise the order lives in two places and they can disagree.
    for plugin in &modlist.inactive_plugins {
        if !modlist
            .plugins
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(plugin))
        {
            anyhow::bail!(
                "inactive-plugins names {plugin}, which is not in the global plugins load \
                 order. An inactive plugin is still ordered; add it to `plugins` too."
            );
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
    // A warning rather than an error: a list mid-build legitimately runs over
    // while the merges that will bring it back under are still unwritten, and
    // refusing to install it would make the intermediate state unreachable.
    // What must not happen is running over *silently*.
    let mut warnings = modlist.include_warnings.clone();
    if modlist.plugins.len() > PLUGIN_LIMIT {
        warnings.push(format!(
            "load order has {} active plugins, {} over Oblivion's limit of {PLUGIN_LIMIT}. \
             The game loads the first {PLUGIN_LIMIT} and silently ignores the rest, so this \
             list cannot be played until merges bring it back under.",
            modlist.plugins.len(),
            modlist.plugins.len() - PLUGIN_LIMIT,
        ));
    }

    validate_merges(modlist, &flattened_mods)?;
    detect_cycles(&flattened_mods)?;

    Ok(warnings)
}

/// Active plugins Oblivion can load. The mod index is one byte, and `0xFF` is
/// reserved for the save's own in-memory records.
const PLUGIN_LIMIT: usize = 255;

/// Every mod id a mod's actions declare a conflict against.
fn conflict_partners(spec: &ModEntry) -> impl Iterator<Item = &String> {
    spec.actions.iter().flat_map(|action| match &action.action {
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

        // An empty mod that declares content is a contradiction, and the
        // contradiction always resolves the wrong way: install writes the
        // folder and silently ignores everything else the entry asked for.
        if spec.mod_type == Some(ModType::Empty)
            && (!spec.archives.is_empty() || !spec.files.is_empty() || !spec.plugins.is_empty())
        {
            anyhow::bail!(
                "mod {mod_id} has type = \"empty\" but declares archives, files or plugins; an \
                 empty mod is the folder and nothing else"
            );
        }

        // `simple` copies the archive root as-is, so a `data_folder` beside it
        // is silently ignored -- and the content lands one folder too deep.
        for archive in &spec.archives {
            if archive.layout == Some(crate::config::schema::ArchiveLayout::Simple)
                && archive.data_folder.is_some()
            {
                anyhow::bail!(
                    "mod {mod_id}: layout = \"simple\" ignores data_folder; use \
                     layout = \"custom-data-folder\" to install from a subfolder"
                );
            }
        }

        // An MO2 plugin is what its archive holds, put in `plugins/`: nothing
        // there loads plugins, and actions assume a mod folder.
        if spec.mod_type == Some(ModType::Mo2Plugin)
            && (spec.archives.is_empty()
                || !spec.files.is_empty()
                || !spec.plugins.is_empty()
                || !spec.actions.is_empty())
        {
            anyhow::bail!(
                "mod {mod_id} has type = \"mo2-plugin\": it needs archives, and takes no files, \
                 plugins or actions"
            );
        }

        // A generated mod is exactly what its actions write.
        if spec.mod_type == Some(ModType::Generated) {
            if !spec.archives.is_empty() || !spec.files.is_empty() {
                anyhow::bail!(
                    "mod {mod_id} has type = \"generated\" but declares archives or files; its \
                     content comes from its actions"
                );
            }
            if spec.actions.is_empty() {
                anyhow::bail!(
                    "mod {mod_id} has type = \"generated\" but no actions, so it would be empty; \
                     use type = \"empty\" for a deliberately empty folder"
                );
            }
        }

        let Some(merge) = &spec.merge else { continue };

        if !spec.archives.is_empty() || !spec.files.is_empty() {
            anyhow::bail!(
                "merge {mod_id} also declares archives or files; a merge mod contains only its \
                 merged output"
            );
        }

        if !is_plugin_filename(&merge.output) {
            anyhow::bail!(
                "merge {mod_id} has invalid output filename {}",
                merge.output
            );
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

/// Refuse mod conditions that ask about each other in a loop: `A if mod:B`
/// and `B if !mod:A` has no answer, and one that happens to be consistent
/// (`A if mod:B`, `B if mod:A`) has two.
fn reject_condition_cycles(refs: &HashMap<&str, Vec<String>>) -> anyhow::Result<()> {
    fn visit<'a>(
        id: &'a str,
        refs: &'a HashMap<&str, Vec<String>>,
        finished: &mut HashSet<&'a str>,
        stack: &mut Vec<&'a str>,
    ) -> anyhow::Result<()> {
        if finished.contains(id) {
            return Ok(());
        }
        if let Some(at) = stack.iter().position(|seen| *seen == id) {
            let mut cycle = stack[at..].to_vec();
            cycle.push(id);
            anyhow::bail!(
                "mod conditions depend on each other: {}",
                cycle.join(" -> ")
            );
        }
        stack.push(id);
        for other in refs.get(id).into_iter().flatten() {
            visit(other, refs, finished, stack)?;
        }
        stack.pop();
        finished.insert(id);
        Ok(())
    }

    let mut ids: Vec<&&str> = refs.keys().collect();
    ids.sort();
    let mut finished = HashSet::new();
    for id in ids {
        visit(id, refs, &mut finished, &mut Vec::new())?;
    }
    Ok(())
}

/// Docs that name a section nobody declares, or a picture that is not there,
/// would vanish from an export without a word.
fn validate_docs(modlist: &SourceModlist, mods: &IndexMap<String, ModEntry>) -> anyhow::Result<()> {
    let sections: HashSet<String> = mods
        .values()
        .flat_map(|spec| (1..=spec.section.len()).map(|depth| spec.section[..depth].join(" - ")))
        .collect();
    for name in modlist.sections.keys() {
        if !sections.contains(name) {
            anyhow::bail!(
                "[sections.{name:?}] documents a section no mod is in (sections are named as MO2 \
                 shows the separator, nested levels joined with \" - \")"
            );
        }
    }

    let documented = std::iter::once(("the guide's [docs]".to_string(), &modlist.docs))
        .chain(
            modlist
                .sections
                .iter()
                .map(|(name, docs)| (format!("section {name:?}"), docs)),
        )
        .chain(
            mods.iter()
                .map(|(id, spec)| (format!("mod {id:?}"), &spec.docs)),
        );
    for (site, docs) in documented {
        for image in &docs.images {
            anyhow::ensure!(
                image.path.is_some() || image.url.is_some(),
                "{site}: an image needs a path, a url, or both"
            );
            if let (Some(path), Some(base)) = (&image.path, &modlist.base_dir)
                && !base.join(path).is_file()
            {
                anyhow::bail!(
                    "{site}: image {path} does not exist (paths are relative to the modlist)"
                );
            }
        }
    }
    Ok(())
}
