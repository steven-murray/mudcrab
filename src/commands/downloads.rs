//! `mudcrab downloads`: housekeeping for the downloads folder.

use crate::cli::{DownloadsCommand, DownloadsLocateArgs, DownloadsRecordArgs, DownloadsTidyArgs};
use crate::config::downloads_tidy::{self, Step};
use std::collections::HashMap;

pub async fn run(command: DownloadsCommand) -> anyhow::Result<()> {
    match command {
        DownloadsCommand::Tidy(args) => tidy(args).await,
        DownloadsCommand::Record(args) => record(args),
        DownloadsCommand::Locate(args) => locate(args),
    }
}

/// Where each archive the plan needs is, as tab-separated lines on stdout.
fn locate(args: DownloadsLocateArgs) -> anyhow::Result<()> {
    use crate::config::archive_store::{ArchiveRef, find};

    let raw = std::fs::read_to_string(&args.plan)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.plan.display()))?;
    let plan: crate::config::schema::PersonalizedPlan = serde_json::from_str(&raw)
        .map_err(|err| anyhow::anyhow!("failed to parse plan {}: {err}", args.plan.display()))?;
    let downloads = super::resolve_downloads_dir(args.downloads.clone(), None)?;
    let search = &args.archive_sources.archive_search_paths;
    let (mut found, mut missing) = (0usize, 0usize);
    for entry in &plan.mods {
        for (index, archive) in entry.archives.iter().enumerate() {
            let mut lines: Vec<(String, Option<std::path::PathBuf>)> = Vec::new();
            if archive.build.is_empty() {
                let reference = ArchiveRef::for_archive(archive);
                lines.push((
                    reference.source.to_string(),
                    find(&downloads, search, &reference),
                ));
            } else {
                for layer in &archive.build {
                    let reference = ArchiveRef::new(&layer.path, layer.file_name.as_deref());
                    lines.push((layer.path.clone(), find(&downloads, search, &reference)));
                }
            }
            for (source, path) in lines {
                let at = match &path {
                    Some(path) => {
                        found += 1;
                        path.display().to_string()
                    }
                    None => {
                        missing += 1;
                        "MISSING".to_string()
                    }
                };
                println!("{}\t{}\t{source}\t{at}", entry.id, index + 1);
            }
        }
    }
    crate::ui::status_err(
        if missing > 0 {
            crate::ui::Tone::Warning
        } else {
            crate::ui::Tone::Good
        },
        format!("{found} found, {missing} missing"),
    );
    Ok(())
}

/// Fill in what lets an archive be recognised wherever it turns up: the file
/// name of a Nexus or URL archive that has none, and the size and MD5 of a
/// `manual:` one. Only for archives whose file is on disk.
fn record(args: DownloadsRecordArgs) -> anyhow::Result<()> {
    use crate::config::archive_store::{ArchiveRef, find};

    let downloads = super::resolve_downloads_dir(args.downloads.clone(), None)?;
    let search = &args.archive_sources.archive_search_paths;
    let original = std::fs::read_to_string(&args.modlist)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.modlist.display()))?;
    let source = crate::config::loader::load_modlist(&args.modlist)?;

    let mut text = original.clone();
    let (mut recorded, mut missing, mut already) = (0usize, Vec::new(), 0usize);
    for (mod_index, entry) in source.mods.iter().enumerate() {
        for (archive_index, archive) in entry.archives.iter().enumerate() {
            let Some(path) = archive.path.as_deref() else {
                continue;
            };
            let manual = path.starts_with("manual:");
            // A file name lets a copy with no `.meta` -- a browser download, a
            // file from another machine -- be recognised; a `manual:` source
            // already carries one.
            let needs_name = archive.file_name.is_none()
                && (path.starts_with("nexus:")
                    || path.starts_with("http://")
                    || path.starts_with("https://"));
            let needs_content = manual && archive.size.is_none() && archive.md5.is_none();
            if !needs_name && !needs_content {
                if manual || archive.file_name.is_some() {
                    already += 1;
                }
                continue;
            }
            let reference = ArchiveRef::new(path, archive.file_name.as_deref());
            let Some(file) = find(&downloads, search, &reference) else {
                missing.push(format!("{} ({})", entry.id, reference.describe()));
                continue;
            };
            let mut lines = Vec::new();
            if needs_name {
                let name = file
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                println!("{}: {path} -- file_name {name}", entry.id);
                lines.push(format!(
                    "file_name = \"{}\"",
                    name.replace('\\', "\\\\").replace('"', "\\\"")
                ));
            }
            if needs_content {
                let size = std::fs::metadata(&file)
                    .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", file.display()))?
                    .len();
                let md5 = crate::config::identify::md5_of_file(&file)?;
                println!(
                    "{}: {} -- size {size}, md5 {md5}",
                    entry.id,
                    reference.describe()
                );
                lines.push(format!("size = {size}"));
                lines.push(format!("md5 = \"{md5}\""));
            }
            text = crate::config::add::plan_archive_lines(
                &text,
                &source.mods,
                mod_index,
                archive_index,
                &lines,
            )?;
            recorded += 1;
        }
    }

    // Build layers: a file name each, as for an archive.
    for (mod_index, entry) in source.mods.iter().enumerate() {
        let mut nth = 0usize;
        for archive in &entry.archives {
            for layer in &archive.build {
                let this = nth;
                nth += 1;
                if layer.file_name.is_some() {
                    continue;
                }
                let reference = ArchiveRef::new(&layer.path, None);
                let Some(file) = find(&downloads, search, &reference) else {
                    missing.push(format!("{} ({})", entry.id, layer.path));
                    continue;
                };
                let name = file
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                println!("{}: {} -- file_name {name}", entry.id, layer.path);
                text = crate::config::add::plan_table_lines(
                    &text,
                    &source.mods,
                    mod_index,
                    "[[mods.archives.build]]",
                    this,
                    &[format!(
                        "file_name = \"{}\"",
                        name.replace('\\', "\\\\").replace('"', "\\\"")
                    )],
                )?;
                recorded += 1;
            }
        }
    }

    for item in &missing {
        crate::ui::status_err(
            crate::ui::Tone::Warning,
            format!("not on disk, so not recorded: {item}"),
        );
    }
    crate::ui::status_err(
        crate::ui::Tone::Good,
        format!(
            "{recorded} archive(s) recorded{}{}",
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
        ),
    );
    if args.dry_run || recorded == 0 {
        if args.dry_run {
            eprintln!("{}", crate::ui::dim("(--dry-run: nothing written)"));
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
    let Some(key) = crate::config::tools::ToolsConfig::nexus_api_key() else {
        crate::ui::status_err(
            crate::ui::Tone::Note,
            format!(
                "{} archive(s) have no recorded name; set a Nexus API key \
             (NEXUS_API_KEY, or nexus_api_key in tools.toml) to look them up",
                wanted.len()
            ),
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
                    crate::ui::status_err(crate::ui::Tone::Note, format!("{source}: {err}"));
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
                println!(
                    "{} {}  {}",
                    console::style("delete  ").red(),
                    name(derived),
                    crate::ui::dim(format!("(same file as {})", name(real)))
                );
            }
            Step::Rename { derived, real } => {
                renames += 1;
                println!(
                    "{} {}  {}  {}",
                    console::style("rename  ").cyan(),
                    name(derived),
                    crate::ui::arrow(),
                    name(real)
                );
            }
            Step::Conflict { derived, real } => {
                println!(
                    "{} {}  {}",
                    console::style("CONFLICT").yellow().bold(),
                    name(derived),
                    crate::ui::dim(format!("({} is a different file; left alone)", name(real)))
                );
            }
            Step::Unknown { derived } => {
                println!(
                    "{} {}  {}",
                    crate::ui::dim("unknown "),
                    name(derived),
                    crate::ui::dim("(no real name recorded; left alone)")
                );
            }
            Step::WriteMeta { archive, source } => {
                metas += 1;
                println!(
                    "{} {}  {}",
                    console::style("meta    ").green(),
                    name(archive),
                    crate::ui::dim(format!("({source})"))
                );
            }
            Step::DeleteSidecar { sidecar } => {
                sidecars += 1;
                println!(
                    "{} {}  {}",
                    console::style("delete  ").red(),
                    name(sidecar),
                    crate::ui::dim("(no archive beside it)")
                );
            }
        }
    }
    let left = steps
        .iter()
        .filter(|s| matches!(s, Step::Conflict { .. } | Step::Unknown { .. }))
        .count();
    crate::ui::status_err(
        if left > 0 {
            crate::ui::Tone::Warning
        } else {
            crate::ui::Tone::Good
        },
        format!(
            "{deletes} duplicate name(s) to delete, {renames} to rename, {metas} .meta to \
             write, {sidecars} stray sidecar(s); {left} left for you"
        ),
    );

    if args.apply {
        downloads_tidy::apply(&steps)?;
        crate::ui::status_err(crate::ui::Tone::Good, "Applied.");
    } else if !steps.is_empty() {
        crate::ui::status_err(
            crate::ui::Tone::Note,
            "Nothing changed; run again with --apply to do this.",
        );
    }
    Ok(())
}
