use crate::cli::NewMergeArgs;
use crate::config::scaffold;

pub async fn run(args: NewMergeArgs) -> anyhow::Result<()> {
    let mut requested = args.plugin.clone();
    if let Some(path) = &args.plugins_from {
        let text = std::fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
        requested.extend(
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(ToString::to_string),
        );
    }

    let sources = scaffold::resolve_sources(&args.mods_dir, &requested)?;

    let load_order_path = match &args.load_order {
        Some(path) => path.clone(),
        None => {
            let found = scaffold::discover_load_orders(&args.mods_dir);
            match found.len() {
                1 => found.into_iter().next().expect("length checked"),
                0 => anyhow::bail!(
                    "no profile loadorder.txt found next to {}. Pass --load-order <file>: \
                     the merge orders its masters by the load order, so it cannot be guessed.",
                    args.mods_dir.display()
                ),
                _ => anyhow::bail!(
                    "several profiles have a loadorder.txt; pass --load-order to choose one:\n  {}",
                    found
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join("\n  ")
                ),
            }
        }
    };
    let load_order = scaffold::read_load_order(&load_order_path)?;

    let output_plugin = args
        .output_plugin
        .clone()
        .unwrap_or_else(|| format!("{}.esp", args.name));
    let plugins = scaffold::post_merge_load_order(&load_order, &sources, &output_plugin);

    // Every source must be in the load order, or the merge is being built
    // against an order that does not describe the instance -- masters would be
    // sorted by a list missing half the picture.
    let missing: Vec<&str> = sources
        .iter()
        .filter(|source| {
            !load_order
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&source.plugin))
        })
        .map(|source| source.plugin.as_str())
        .collect();
    if !missing.is_empty() {
        anyhow::bail!(
            "{} is not in {}: {}.\nA plugin absent from the load order is not active, so merging \
             it would change nothing in game.",
            if missing.len() == 1 { "a source plugin" } else { "some source plugins" },
            load_order_path.display(),
            missing.join(", ")
        );
    }

    let rendered = scaffold::render(&args.name, &output_plugin, &sources, &plugins);

    match &args.output {
        Some(path) => {
            if path.exists() && !args.force {
                anyhow::bail!(
                    "{} already exists; pass --force to overwrite it",
                    path.display()
                );
            }
            std::fs::write(path, &rendered)
                .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;
            tracing::info!(
                output = %path.display(),
                sources = sources.len(),
                load_order = %load_order_path.display(),
                "wrote a merge modlist"
            );
            println!(
                "Wrote {} — {} sources, load order from {}.\n\nBuild it with:\n  mudcrab merge {} \\\n    --mods-dir {} \\\n    --output <somewhere new>",
                path.display(),
                sources.len(),
                load_order_path.display(),
                path.display(),
                args.mods_dir.display(),
            );
        }
        None => print!("{rendered}"),
    }
    Ok(())
}
