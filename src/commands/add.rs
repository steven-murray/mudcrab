use crate::cli::AddArgs;
use crate::config;
use crate::config::add::{find_plugins, parse_meta_ini, plan_insertion_before, NewMod, OracleMeta};
use crate::config::install::safe_mod_dir_name;
use std::path::{Path, PathBuf};

pub async fn run(args: AddArgs) -> anyhow::Result<()> {
    let (new_mod, mod_dir) = resolve(&args)?;

    // The id becomes a directory name at install time, so reject here what
    // install would reject there rather than after 600 entries are written.
    safe_mod_dir_name(&new_mod.id)?;

    let original = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let source = config::loader::load_modlist(&args.input)?;

    if let Some(existing) = source.mods.iter().find(|entry| entry.id == new_mod.id) {
        anyhow::bail!(
            "{} already contains a mod with id '{}' (section {}); \
             pass --id to add this one under a different name",
            args.input.display(),
            existing.id,
            if existing.section.is_empty() {
                "<none>".to_string()
            } else {
                existing.section.join(" - ")
            }
        );
    }

    let before: Option<&[String]> = if args.before_sections.is_empty() {
        None
    } else {
        Some(&args.before_sections)
    };
    let plan = plan_insertion_before(&original, &source.mods, &new_mod, before)?;

    if new_mod.section.is_empty() {
        eprintln!(
            "warning: no --section given, so '{}' is being added with no section; \
             it will not sit under any MO2 separator.",
            new_mod.id
        );
    }
    if new_mod.non_nexus {
        eprintln!(
            "warning: '{}' has modid=0 in its meta.ini, so it did not come from Nexus. \
             The block is written with no `path` and a TODO; fill in the source yourself.",
            new_mod.id
        );
    }

    if let Some(dir) = &mod_dir {
        report_plugins(&new_mod.id, dir);
    }

    if args.dry_run {
        println!(
            "would insert into {} at line {}, {}:\n",
            args.input.display(),
            plan.line,
            plan.anchor
        );
        println!("{}", plan.block);
        println!("\n(--dry-run: nothing written)");
        return Ok(());
    }

    write_atomic(&args.input, &plan.new_text)?;

    // A modlist that no longer parses is worse than one missing a mod, so the
    // insert is only kept if the whole file still loads and validates.
    if let Err(err) = reload_and_validate(&args.input) {
        write_atomic(&args.input, &original).map_err(|restore_err| {
            anyhow::anyhow!(
                "adding '{}' produced an invalid modlist ({err}), and restoring the original \
                 also failed ({restore_err}). The file may be damaged.",
                new_mod.id
            )
        })?;
        anyhow::bail!(
            "adding '{}' produced a modlist that no longer validates: {err}. \
             The original file has been restored unchanged.",
            new_mod.id
        );
    }

    println!(
        "added '{}' to {} at line {}, {}",
        new_mod.id,
        args.input.display(),
        plan.line,
        plan.anchor
    );

    Ok(())
}

/// Build the entry to insert, plus the Oracle mod folder when there is one (it
/// is scanned afterwards for plugins).
fn resolve(args: &AddArgs) -> anyhow::Result<(NewMod, Option<PathBuf>)> {
    match (&args.from_oracle, &args.nexus) {
        (Some(oracle_dir), None) => {
            let folder = args.mod_folder.as_ref().ok_or_else(|| {
                anyhow::anyhow!("--from-oracle requires --mod <mod folder name>")
            })?;
            let mod_dir = oracle_dir.join(folder);
            let new_mod = from_oracle(&mod_dir, folder, args)?;
            Ok((new_mod, Some(mod_dir)))
        }
        (None, Some(nexus)) => Ok((from_nexus(nexus, args)?, None)),
        (None, None) => anyhow::bail!(
            "give a source: --from-oracle <ORACLE_MODS_DIR> --mod <folder>, or --nexus <modid>/<fileid>"
        ),
        (Some(_), Some(_)) => unreachable!("clap declares --from-oracle and --nexus as conflicting"),
    }
}

fn from_oracle(mod_dir: &Path, folder: &str, args: &AddArgs) -> anyhow::Result<NewMod> {
    let meta_path = mod_dir.join("meta.ini");
    let text = std::fs::read_to_string(&meta_path).map_err(|err| {
        anyhow::anyhow!("failed to read {}: {err}", meta_path.display())
    })?;
    let meta = parse_meta_ini(&text);

    let id = args.id.clone().unwrap_or_else(|| folder.to_string());
    let file_name = args
        .file_name
        .clone()
        .or_else(|| meta.installation_file.clone());

    let mut new_mod = NewMod {
        id,
        section: args.sections.clone(),
        file_name,
        version: meta.version.clone(),
        ..NewMod::default()
    };

    match nexus_ids(&meta) {
        NexusSource::Nexus { mod_id, file_id } => {
            new_mod.path = Some(format!("nexus:oblivion/{mod_id}/{file_id}"));
            new_mod.download_handler = Some("nexus".to_string());
        }
        NexusSource::NotNexus => new_mod.non_nexus = true,
        NexusSource::MissingFileId { mod_id } => anyhow::bail!(
            "{} has modid={mod_id} but no `1\\fileid` under [installedFiles], so the download \
             URL cannot be built. Add it with --nexus {mod_id}/<fileid> instead.",
            meta_path.display()
        ),
    }

    Ok(new_mod)
}

enum NexusSource {
    Nexus { mod_id: u64, file_id: u64 },
    /// `modid=0`: MO2's marker for a mod installed from somewhere else.
    NotNexus,
    MissingFileId { mod_id: u64 },
}

fn nexus_ids(meta: &OracleMeta) -> NexusSource {
    match (meta.mod_id, meta.file_id) {
        (None | Some(0), _) => NexusSource::NotNexus,
        (Some(mod_id), Some(file_id)) if file_id != 0 => NexusSource::Nexus { mod_id, file_id },
        (Some(mod_id), _) => NexusSource::MissingFileId { mod_id },
    }
}

fn from_nexus(spec: &str, args: &AddArgs) -> anyhow::Result<NewMod> {
    let (mod_id, file_id) = spec
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("--nexus must be <modid>/<fileid>, got '{spec}'"))?;
    let mod_id: u64 = mod_id
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("--nexus mod id must be a number, got '{mod_id}'"))?;
    let file_id: u64 = file_id
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("--nexus file id must be a number, got '{file_id}'"))?;

    let id = args
        .id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("--nexus requires --id <mod id>"))?;
    if args.sections.is_empty() {
        anyhow::bail!("--nexus requires --section <name>");
    }

    Ok(NewMod {
        id,
        section: args.sections.clone(),
        path: Some(format!("nexus:oblivion/{mod_id}/{file_id}")),
        download_handler: Some("nexus".to_string()),
        file_name: args.file_name.clone(),
        version: None,
        non_nexus: false,
    })
}

/// Report plugins the mod folder ships, without touching the load order.
///
/// `validate` hard-fails on a mod declaring a plugin missing from the top-level
/// `plugins` array, and a plugin in the wrong place in load order fails
/// silently in game, which is worse. So the position is the author's call: this
/// only tells them what there is to place.
fn report_plugins(mod_id: &str, mod_dir: &Path) {
    let plugins = find_plugins(mod_dir);
    if plugins.is_empty() {
        return;
    }

    eprintln!(
        "'{mod_id}' ships {} plugin{}:",
        plugins.len(),
        if plugins.len() == 1 { "" } else { "s" }
    );
    for plugin in &plugins {
        eprintln!("  {plugin}");
    }
    eprintln!(
        "Add them to the top-level `plugins` array yourself, at the right point in load order. \
         mudcrab will not guess the position, and the mod block was written without a `plugins` \
         key so the modlist still validates until you do."
    );
}

/// Write via a temp file next to the target, then rename, so an interrupted
/// write cannot leave a half-written modlist behind.
fn write_atomic(path: &Path, content: &str) -> anyhow::Result<()> {
    let mut temp = path.as_os_str().to_os_string();
    temp.push(format!(".mudcrab-add-{}.tmp", std::process::id()));
    let temp = PathBuf::from(temp);

    std::fs::write(&temp, content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", temp.display()))?;

    std::fs::rename(&temp, path).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        anyhow::anyhow!("failed to replace {}: {err}", path.display())
    })
}

fn reload_and_validate(path: &Path) -> anyhow::Result<()> {
    let reloaded = config::loader::load_modlist(path)?;
    config::validator::validate(&reloaded)
}
