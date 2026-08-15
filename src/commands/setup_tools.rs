use crate::config::schema::ModAction;
use crate::cli::SetupToolsArgs;
use crate::config::loader::load_modlist;
use crate::config::schema::PostInstallAction;
use crate::config::tools::{generate_tools_toml, RequiredTools};
use std::path::PathBuf;

pub async fn run(args: SetupToolsArgs) -> anyhow::Result<()> {
    let source = load_modlist(&args.input).map_err(|err| {
        anyhow::anyhow!("failed to parse {}: {err}", args.input.display())
    })?;

    // Detect which tools are referenced anywhere in the modlist.
    let mut required = RequiredTools::default();

    if source
        .post_install_actions
        .iter()
        .any(|a| matches!(a, PostInstallAction::LootSort))
    {
        required.needs_loot = true;
    }

    let flat = source.flatten_mods().map_err(|err| {
        anyhow::anyhow!("failed to flatten mods while scanning for required tools: {err}")
    })?;
    for (_id, spec) in &flat {
        for action in &spec.actions {
            if matches!(action, ModAction::Qac(_)) {
                required.needs_tes4edit = true;
            }
        }
    }

    // Report what was found.
    tracing::info!(
        needs_loot = required.needs_loot,
        needs_tes4edit = required.needs_tes4edit,
        "setup-tools: tool scan complete"
    );

    if !required.needs_loot && !required.needs_tes4edit {
        tracing::info!("setup-tools: no external tools required by this modlist");
    }

    let content = generate_tools_toml(&required);

    let output_path = args
        .output
        .clone()
        .or_else(|| {
            // Default: tools.toml alongside the input file.
            args.input.parent().map(|p| p.join("tools.toml"))
        })
        .unwrap_or_else(|| PathBuf::from("tools.toml"));

    if output_path.exists() && !args.force {
        anyhow::bail!(
            "{} already exists; pass --force to overwrite",
            output_path.display()
        );
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            anyhow::anyhow!("failed to create output directory {}: {err}", parent.display())
        })?;
    }

    std::fs::write(&output_path, &content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", output_path.display()))?;

    tracing::info!(path = %output_path.display(), "setup-tools: wrote tools.toml template");

    println!("Wrote tools configuration template to: {}", output_path.display());
    println!("Edit the file and fill in the placeholder paths before running 'mudcrab install'.");

    Ok(())
}
