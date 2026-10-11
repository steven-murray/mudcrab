use crate::cli::ExportArgs;
use crate::config::loader::load_modlist;
use crate::config::schema::PersonalizedPlan;
use crate::config::validator::validate;
use crate::export::exporter::{Exporter, OutputMode};
use crate::export::{build_document, builtin};
use std::path::Path;

pub async fn run(args: ExportArgs) -> anyhow::Result<()> {
    if let Some(dir) = &args.eject {
        let count = Exporter::eject(&args.exporter, dir)?;
        crate::ui::say(format!(
            "Copied the {} exporter ({count} file(s)) to {}; export with --to {}",
            args.exporter,
            dir.display(),
            dir.display()
        ));
        return Ok(());
    }
    let input = args
        .input
        .as_deref()
        .expect("clap requires input without --eject");
    let modlist_dir = input.parent().unwrap_or(Path::new("."));

    let exporter = match args.exporter.as_str() {
        builtin::JSON => None,
        spec => Some(Exporter::find(spec, modlist_dir)?),
    };
    if args.list_options {
        match &exporter {
            Some(exporter) => print!("{}", exporter.describe_options()),
            None => println!("json has no options"),
        }
        return Ok(());
    }

    let modlist = load_modlist(input)?;
    for warning in validate(&modlist)? {
        tracing::warn!("{warning}");
    }
    let plan = match &args.plan {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .map_err(|err| anyhow::anyhow!("failed to read plan {}: {err}", path.display()))?;
            Some(
                serde_json::from_str::<PersonalizedPlan>(&raw).map_err(|err| {
                    anyhow::anyhow!("failed to parse plan {}: {err}", path.display())
                })?,
            )
        }
        None => None,
    };
    let archives = args
        .downloads
        .as_deref()
        .map(|downloads| crate::export::Archives {
            downloads,
            search_paths: &args.archive_search_paths,
        });
    let document = build_document(&modlist, plan.as_ref(), archives.as_ref())?;

    let Some(exporter) = exporter else {
        anyhow::ensure!(args.options.is_empty(), "json has no options");
        let text = serde_json::to_string_pretty(&document)? + "\n";
        match &args.output {
            Some(path) => {
                std::fs::write(path, text)
                    .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;
                crate::ui::say(format!(
                    "Exported {} mod(s) as json -> {}",
                    document.mod_count,
                    path.display()
                ));
            }
            None => print!("{text}"),
        }
        return Ok(());
    };

    let options = exporter.resolve_options(modlist.export.get(&exporter.name), &args.options)?;
    let warnings = exporter.render(&document, &options, modlist_dir, args.output.as_deref())?;
    for warning in &warnings {
        tracing::warn!("{warning}");
    }
    if let Some(path) = &args.output {
        let what = match exporter.output_mode() {
            OutputMode::File => "file",
            OutputMode::Site => "site",
        };
        crate::ui::say(format!(
            "Exported {} mod(s) with {} -> {} ({what}){}",
            document.mod_count,
            exporter.name,
            path.display(),
            if warnings.is_empty() {
                String::new()
            } else {
                format!(", {} warning(s)", warnings.len())
            }
        ));
    }
    Ok(())
}
