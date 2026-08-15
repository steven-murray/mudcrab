use crate::config::schema::{CompiledArchive, CompiledMod, CompiledModlist, SourceModlist};

pub fn compile(source: SourceModlist) -> anyhow::Result<CompiledModlist> {
    let actions = compile_top_level_actions(&source.ini);
    let flattened_mods = source.flatten_mods()?;
    let mo2_modlist_entries = source.mo2_modlist_entries()?;
    let mods: Vec<CompiledMod> = flattened_mods
        .iter()
        .map(|(id, spec)| -> anyhow::Result<CompiledMod> {
            Ok(CompiledMod {
                id: id.clone(),
                mod_type: spec.mod_type.clone(),
                dependencies: spec.dependencies.clone(),
                archives: spec
                    .archives
                    .iter()
                    .map(|archive| {
                        if archive.path.is_some() && !archive.build.is_empty() {
                            return Err(anyhow::anyhow!(
                                "mod '{}': archive cannot have both 'path' and 'build'",
                                id
                            ));
                        }
                        if archive.path.is_none() && archive.build.is_empty() {
                            return Err(anyhow::anyhow!(
                                "mod '{}': archive must have either 'path' or 'build'",
                                id
                            ));
                        }
                        Ok(CompiledArchive {
                            path: archive.path.clone(),
                            download_handler: archive.download_handler.clone(),
                            layout: archive.layout.clone(),
                            data_folder: archive.data_folder.clone(),
                            target_subdir: archive.target_subdir.clone(),
                            bain_subpackages: archive.bain_subpackages.clone(),
                            fomod_selections: archive.fomod_selections.clone(),
                            build: archive.build.clone(),
                            include: archive.include.clone(),
                            exclude: archive.exclude.clone(),
                            game_root_files: archive.game_root_files.clone(),
                        })
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?,
                condition: spec.condition.clone(),
                files: spec.files.clone(),
                actions: spec.actions.clone(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(CompiledModlist {
        schema_version: 1,
        name: source.name,
        input_count: source.inputs.len(),
        mod_count: mods.len(),
        plugin_count: source.plugins.len(),
        inputs: source.inputs,
        actions,
        plugins: source.plugins,
        post_install_actions: source.post_install_actions,
        mo2_modlist_entries,
        mods,
    })
}

fn compile_top_level_actions(ini: &toml::Table) -> toml::Table {
    if ini.is_empty() {
        return toml::Table::new();
    }

    let mut ini_set = Vec::new();
    for (key, value) in ini {
        let mut entry = toml::Table::new();
        entry.insert("scope".to_string(), toml::Value::String("game".to_string()));
        entry.insert("file".to_string(), toml::Value::String("Oblivion.ini".to_string()));
        entry.insert("key".to_string(), toml::Value::String(key.clone()));
        entry.insert("value".to_string(), toml::Value::String(ini_value_to_string(value)));
        ini_set.push(toml::Value::Table(entry));
    }

    let mut actions = toml::Table::new();
    actions.insert("ini_set".to_string(), toml::Value::Array(ini_set));
    actions
}

fn ini_value_to_string(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Integer(number) => number.to_string(),
        toml::Value::Float(number) => number.to_string(),
        toml::Value::Boolean(flag) => {
            if *flag { "1".to_string() } else { "0".to_string() }
        }
        other => other.to_string(),
    }
}
