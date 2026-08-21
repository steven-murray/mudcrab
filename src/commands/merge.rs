//! Build merges against an existing install, without installing anything.
//!
//! This is the command the manual verification loop runs: it reads mod folders
//! and writes merged plugins somewhere else. It never renames a source plugin,
//! never writes into the mods directory, and never touches a profile -- so a
//! merge can be built and inspected before anything in a real instance changes.

use crate::cli::MergeArgs;
use crate::config;
use crate::config::schema::MergeSpec;
use crate::merge::{self, MergeRequest, MergeSource};
use crate::plugin::PluginName;
use crate::util::fs::{find_child_case_insensitive, write_text_file};
use std::path::{Path, PathBuf};

pub async fn run(args: MergeArgs) -> anyhow::Result<()> {
    let source = config::loader::load_modlist(&args.input)?;
    for warning in config::validator::validate(&source)? {
        tracing::warn!("{warning}");
    }

    let load_order: Vec<PluginName> = source.plugins.iter().map(PluginName::new).collect();

    let selected: Vec<(&str, &MergeSpec)> = source
        .mods
        .iter()
        .filter_map(|entry| entry.merge.as_ref().map(|merge| (entry.id.as_str(), merge)))
        .filter(|(id, _)| args.only.as_deref().is_none_or(|only| only == *id))
        .collect();

    if selected.is_empty() {
        match &args.only {
            Some(only) => anyhow::bail!(
                "no merge with id '{only}' in {}. Available: {}",
                args.input.display(),
                available(&source)
            ),
            None => anyhow::bail!("{} declares no merges", args.input.display()),
        }
    }

    std::fs::create_dir_all(&args.output)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", args.output.display()))?;

    for (id, spec) in selected {
        let mut sources = Vec::with_capacity(spec.sources.len());
        for entry in &spec.sources {
            let mod_dir = args.mods_dir.join(&entry.mod_id);
            let path = locate(&mod_dir, &entry.plugin).ok_or_else(|| {
                anyhow::anyhow!(
                    "merge '{id}': {} is not in {}",
                    entry.plugin,
                    mod_dir.display()
                )
            })?;
            sources.push(MergeSource {
                plugin: PluginName::new(&entry.plugin),
                path,
            });
        }

        let request = MergeRequest {
            name: id.to_string(),
            output: spec.output.clone(),
            sources,
            load_order: load_order.clone(),
        };

        let built = merge::run(&request)?;
        let report = &built.report;

        let target = args.output.join(id);
        std::fs::create_dir_all(&target)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", target.display()))?;

        let plugin_path = target.join(&spec.output);
        std::fs::write(&plugin_path, built.plugin.to_bytes())
            .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", plugin_path.display()))?;

        let sidecar = target.join(format!("merge - {id}"));
        std::fs::create_dir_all(&sidecar)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", sidecar.display()))?;
        write_text_file(
            &sidecar.join("map.json"),
            &merge::run::map_json(&built.allocation),
        )?;

        println!("{id}");
        println!("  output     {}", plugin_path.display());
        println!("  sources    {}", report.source_count);
        println!("  masters    {} ({})", report.masters.len(), report.masters.join(", "));
        println!("  records    {}", report.record_count);
        println!("  groups     {}", report.group_count);
        println!("  renumbered {}", report.remapped);
        println!("  clobbered  {}", report.clobbered);
        if report.non_canonical_inputs > 0 {
            println!(
                "  note       {} source reference(s) used a mod index past their own",
                report.non_canonical_inputs
            );
            println!("             master list; translated as own-record references");
        }
        println!();

        tracing::info!(
            merge = id,
            output = %plugin_path.display(),
            records = report.record_count,
            remapped = report.remapped,
            clobbered = report.clobbered,
            "merge built"
        );
    }

    Ok(())
}

fn available(source: &config::schema::SourceModlist) -> String {
    let names: Vec<&str> = source
        .mods
        .iter()
        .filter(|entry| entry.merge.is_some())
        .map(|entry| entry.id.as_str())
        .collect();
    if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    }
}

/// Find a source plugin, accepting the `.mohidden` form a previous merge left.
fn locate(mod_dir: &Path, plugin: &str) -> Option<PathBuf> {
    find_child_case_insensitive(mod_dir, plugin)
        .or_else(|| find_child_case_insensitive(mod_dir, &format!("{plugin}.mohidden")))
}
