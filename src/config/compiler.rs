use crate::config::schema::{
    ArchiveLayout, CompiledArchive, CompiledMod, CompiledModlist, IniScope, IniSetAction, IniSetFormat, IniValue,
    ModAction, SourceModlist,
};

pub fn compile(source: SourceModlist) -> anyhow::Result<CompiledModlist> {
    let actions = compile_top_level_actions(&source.ini);
    let flattened_mods = source.flatten_mods()?;
    let mo2_modlist_entries = source.mo2_modlist_entries()?;
    let mods: Vec<CompiledMod> = flattened_mods
        .iter()
        .map(|(id, spec)| -> anyhow::Result<CompiledMod> {
            Ok(CompiledMod {
                id: id.clone(),
                oracle_name: spec.oracle_name.clone(),
                section: spec.section.clone(),
                mod_type: spec.mod_type,
                merge: spec.merge.clone(),
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
                        if archive.layout == Some(ArchiveLayout::CustomDataFolder)
                            && archive.data_folder.is_none()
                        {
                            return Err(anyhow::anyhow!(
                                "mod '{}': layout = \"custom-data-folder\" requires data_folder \
                                 to say where the data folder is",
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
                            file_name: archive
                                .file_name
                                .clone()
                                .or_else(|| manual_file_name(archive.path.as_deref())),
                            download_handler: archive.download_handler.clone(),
                            layout: archive.layout,
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
        guide: source.guide.clone(),
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

/// Desugar the top-level `[ini]` table into game-scoped ini_set actions.
///
/// These target Oblivion.ini specifically; game-scoped writes are redirected to
/// the MO2 profile-local copy, never the original in the game directory.
/// The filename a `manual:` descriptor already carries.
///
/// `--archive-search-path` matches an archive by its `file_name`, and for a
/// manual source that name is the whole descriptor: `manual:Feldscar.7z` can
/// only ever be `Feldscar.7z`. Writing it twice is redundant, and redundancy
/// that has to be kept in sync is a defect waiting to happen -- an entry with
/// only the descriptor used to be simply unfindable, reported as "must be
/// downloaded by hand" while sitting in a search path.
///
/// Only `manual:`. Every other scheme names a resource, not a file: a Nexus
/// descriptor is ids, and an HTTP one is a URL whose last segment is a guess.
fn manual_file_name(path: Option<&str>) -> Option<String> {
    let name = path?.strip_prefix("manual:")?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn compile_top_level_actions(ini: &toml::Table) -> Vec<ModAction> {
    ini.iter()
        .map(|(key, value)| {
            ModAction::IniSet(IniSetAction {
                scope: IniScope::Game,
                file: "Oblivion.ini".to_string(),
                // The top-level `[ini]` table is a flat key/value map with no
                // way to name a section. That is fine while every key it sets
                // is unique in the file, and `apply_ini_set_in_section` now
                // errors rather than guessing if one ever is not.
                section: None,
                key: key.clone(),
                value: IniValue(ini_value_to_string(value)),
                format: IniSetFormat::Standard,
            })
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manual_descriptor_supplies_its_own_file_name() {
        assert_eq!(
            manual_file_name(Some("manual:Feldscar_-_VA.7z")).as_deref(),
            Some("Feldscar_-_VA.7z")
        );
        assert_eq!(
            manual_file_name(Some("manual:Sutch Village - VA.7z")).as_deref(),
            Some("Sutch Village - VA.7z"),
            "spaces and dashes are part of the name, not separators"
        );
    }

    #[test]
    fn no_other_scheme_guesses_a_file_name() {
        // A Nexus descriptor is ids; the real filename carries a slug and a
        // timestamp that cannot be derived from them.
        assert_eq!(manual_file_name(Some("nexus:oblivion/52874/1000036033")), None);
        // The last segment of a URL is a plausible guess, and a guess that is
        // wrong looks exactly like an archive nobody has downloaded.
        assert_eq!(manual_file_name(Some("https://example.com/x.7z")), None);
        assert_eq!(manual_file_name(Some("manual:")), None);
        assert_eq!(manual_file_name(None), None);
    }

    /// An explicit `file_name` still wins: the descriptor is a label, and a few
    /// entries deliberately point at a file called something else.
    #[test]
    fn an_explicit_file_name_is_not_overridden() {
        // The same `or_else` chain `compile` uses, with the explicit name set.
        let declared = Some("Actually Called This.7z".to_string());
        let resolved = declared.or_else(|| manual_file_name(Some("manual:Label.7z")));
        assert_eq!(resolved.as_deref(), Some("Actually Called This.7z"));
    }
}
