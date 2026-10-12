//! `mudcrab downloads`: housekeeping for the downloads folder.

use crate::cli::{DownloadsCommand, DownloadsRecordArgs, DownloadsTidyArgs};
use crate::config::downloads_tidy::{self, Step};
use std::collections::HashMap;

pub async fn run(command: DownloadsCommand) -> anyhow::Result<()> {
    match command {
        DownloadsCommand::Tidy(args) => tidy(args).await,
        DownloadsCommand::Record(args) => record(args),
    }
}

/// Write `size` and `md5` into every `manual:` archive entry that lacks them
/// and whose file is on disk.
fn record(args: DownloadsRecordArgs) -> anyhow::Result<()> {
    use crate::config::archive_store::{ArchiveRef, find};

    let downloads = super::resolve_downloads_dir(args.downloads.clone(), None, None)?;
    let search = &args.archive_sources.archive_search_paths;
    let original = std::fs::read_to_string(&args.modlist)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.modlist.display()))?;
    let source = crate::config::loader::load_modlist(&args.modlist)?;

    let mut text = original.clone();
    let (mut recorded, mut missing, mut already) = (0usize, Vec::new(), 0usize);
    for (mod_index, entry) in source.mods.iter().enumerate() {
        for (archive_index, archive) in entry.archives.iter().enumerate() {
            let Some(path) = archive.path.as_deref().filter(|p| p.starts_with("manual:")) else {
                continue;
            };
            if archive.size.is_some() || archive.md5.is_some() {
                already += 1;
                continue;
            }
            let reference = ArchiveRef::new(path, archive.file_name.as_deref());
            let Some(file) = find(&downloads, search, &reference) else {
                missing.push(format!("{} ({})", entry.id, reference.describe()));
                continue;
            };
            let size = std::fs::metadata(&file)
                .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", file.display()))?
                .len();
            let md5 = crate::config::identify::md5_of_file(&file)?;
            println!(
                "{}: {} -- size {size}, md5 {md5}",
                entry.id,
                reference.describe()
            );
            text = crate::config::add::plan_archive_lines(
                &text,
                &source.mods,
                mod_index,
                archive_index,
                &[format!("size = {size}"), format!("md5 = \"{md5}\"")],
            )?;
            recorded += 1;
        }
    }

    for item in &missing {
        eprintln!("not on disk, so not recorded: {item}");
    }
    eprintln!(
        "{recorded} hand-downloaded archive(s) recorded{}{}",
        if already > 0 {
            format!(", {already} already were")
        } else {
            String::new()
        },
        if missing.is_empty() {
            String::new()
        } else {
            format!(", {} not found", missing.len())
        },
    );
    if args.dry_run || recorded == 0 {
        if args.dry_run {
            eprintln!("(--dry-run: nothing written)");
        }
        return Ok(());
    }
    super::add::write_atomic(&args.modlist, &text)?;
    if let Err(err) = super::add::reload_and_validate(&args.modlist) {
        super::add::write_atomic(&args.modlist, &original)?;
        anyhow::bail!("the edited modlist no longer validates ({err}); the original is restored");
    }
    Ok(())
}

/// Real names for Nexus archives nothing on disk names, from the API's files
/// list: one request per mod. Skipped, with a note, without an API key.
async fn nexus_names(
    dir: &std::path::Path,
    plan: &crate::config::schema::PersonalizedPlan,
) -> anyhow::Result<HashMap<String, String>> {
    let wanted: Vec<String> = downloads_tidy::unnamed_sources(dir, plan)?
        .into_iter()
        .filter(|source| source.starts_with("nexus:"))
        .collect();
    if wanted.is_empty() {
        return Ok(HashMap::new());
    }
    let Ok(key) = std::env::var("NEXUS_API_KEY") else {
        eprintln!(
            "note: {} archive(s) have no recorded name; set NEXUS_API_KEY to look them up",
            wanted.len()
        );
        return Ok(HashMap::new());
    };
    let base = std::env::var("NEXUS_API_BASE").ok();
    let client = reqwest::Client::new();
    let mut by_mod: HashMap<(String, String), HashMap<String, String>> = HashMap::new();
    let mut out = HashMap::new();
    for source in wanted {
        let Some((game, mod_id, file_id)) =
            crate::config::archive_store::ArchiveRef::new(&source, None).nexus_ids()
        else {
            continue;
        };
        let key_pair = (game.to_string(), mod_id.to_string());
        if !by_mod.contains_key(&key_pair) {
            match crate::config::download::nexus_file_names(
                &client,
                base.as_deref(),
                &key,
                game,
                mod_id,
            )
            .await
            {
                Ok(files) => {
                    by_mod.insert(key_pair.clone(), files);
                }
                Err(err) => {
                    eprintln!("note: {source}: {err}");
                    by_mod.insert(key_pair.clone(), HashMap::new());
                }
            }
        }
        if let Some(name) = by_mod[&key_pair].get(file_id) {
            out.insert(source.clone(), name.clone());
        }
    }
    Ok(out)
}

async fn tidy(args: DownloadsTidyArgs) -> anyhow::Result<()> {
    let plan = args
        .plan
        .as_ref()
        .map(|path| -> anyhow::Result<_> {
            let raw = std::fs::read_to_string(path)
                .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
            serde_json::from_str(&raw)
                .map_err(|err| anyhow::anyhow!("failed to parse plan {}: {err}", path.display()))
        })
        .transpose()?;
    let names = match &plan {
        Some(plan) => nexus_names(&args.dir, plan).await?,
        None => HashMap::new(),
    };
    let steps = downloads_tidy::plan_tidy(&args.dir, plan.as_ref(), &names)?;

    let name = |path: &std::path::Path| {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let (mut deletes, mut renames, mut metas, mut sidecars) = (0, 0, 0, 0);
    for step in &steps {
        match step {
            Step::Delete { derived, real } => {
                deletes += 1;
                println!("delete   {}  (same file as {})", name(derived), name(real));
            }
            Step::Rename { derived, real } => {
                renames += 1;
                println!("rename   {}  ->  {}", name(derived), name(real));
            }
            Step::Conflict { derived, real } => {
                println!(
                    "CONFLICT {}  ({} is a different file; left alone)",
                    name(derived),
                    name(real)
                );
            }
            Step::Unknown { derived } => {
                println!(
                    "unknown  {}  (no real name recorded; left alone)",
                    name(derived)
                );
            }
            Step::WriteMeta { archive, source } => {
                metas += 1;
                println!("meta     {}  ({source})", name(archive));
            }
            Step::DeleteSidecar { sidecar } => {
                sidecars += 1;
                println!("delete   {}  (no archive beside it)", name(sidecar));
            }
        }
    }
    let left = steps
        .iter()
        .filter(|s| matches!(s, Step::Conflict { .. } | Step::Unknown { .. }))
        .count();
    eprintln!(
        "{deletes} duplicate name(s) to delete, {renames} to rename, {metas} .meta to write, \
         {sidecars} stray sidecar(s); {left} left for you"
    );

    if args.apply {
        downloads_tidy::apply(&steps)?;
        eprintln!("applied");
    } else if !steps.is_empty() {
        eprintln!("nothing changed; re-run with --apply to do this");
    }
    Ok(())
}
