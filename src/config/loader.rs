use crate::config::docs::DocsFile;
use crate::config::schema::SourceModlist;
use std::path::Path;

pub fn load_modlist(path: &Path) -> anyhow::Result<SourceModlist> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;

    let mut parsed = toml::from_str::<SourceModlist>(&raw)
        .map_err(|err| anyhow::anyhow!("failed to parse TOML {}: {err}", path.display()))?;

    let base_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    if let Some(docs_file) = parsed.docs_file.clone() {
        let docs = DocsFile::load(&base_dir.join(&docs_file))?;
        merge_docs(&mut parsed, docs).map_err(|err| anyhow::anyhow!("{docs_file}: {err}"))?;
    }
    parsed.base_dir = Some(base_dir);

    Ok(parsed)
}

/// Fold a docs file into the modlist. Anything documented in both is refused:
/// silently preferring one copy would leave the other to be edited in vain.
fn merge_docs(modlist: &mut SourceModlist, file: DocsFile) -> anyhow::Result<()> {
    if let Some(docs) = file.docs {
        anyhow::ensure!(
            modlist.docs.is_empty(),
            "the guide's [docs] are in both the modlist and the docs file"
        );
        modlist.docs = docs;
    }
    for (name, docs) in file.sections {
        anyhow::ensure!(
            !modlist.sections.contains_key(&name),
            "section {name:?} is documented in both the modlist and the docs file"
        );
        modlist.sections.insert(name, docs);
    }
    for (id, docs) in file.mods {
        let entry = modlist
            .mods
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| {
                anyhow::anyhow!("documents mod {id:?}, which the modlist does not have")
            })?;
        anyhow::ensure!(
            entry.docs.is_empty(),
            "mod {id:?} is documented in both the modlist and the docs file"
        );
        entry.docs = docs;
    }
    Ok(())
}
