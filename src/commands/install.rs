use crate::cli::InstallArgs;
use crate::config;
use crate::config::tools::ToolsConfig;
use std::path::PathBuf;

pub async fn run(args: InstallArgs) -> anyhow::Result<()> {
    if let Some(instance) = &args.mo2_instance_dir
        && !args.dry_run
        && !args.ignore_running_mo2
    {
        let pids = config::mo2::running::running_in(instance);
        if !pids.is_empty() {
            anyhow::bail!(
                "Mod Organizer 2 has {} open (pid {}). Close it first: MO2 writes its own copy of \
                 the profile back when it exits, which would undo this install. \
                 (--ignore-running-mo2 overrides this.)",
                instance.display(),
                pids.iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    let game_dir =
        super::resolve_game_dir(args.game_dir.clone(), args.mo2_instance_dir.as_deref())?;
    let raw = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let plan = serde_json::from_str::<config::schema::PersonalizedPlan>(&raw).map_err(|err| {
        anyhow::anyhow!(
            "failed to parse personalized plan {}: {err}",
            args.input.display()
        )
    })?;

    let cache_dir =
        super::resolve_downloads_dir(args.downloads.clone(), args.mo2_instance_dir.as_deref())?;
    let mods_dir = match (&args.mods_dir, &args.mo2_instance_dir) {
        (Some(path), _) => path.clone(),
        (None, Some(instance_dir)) => instance_dir.join("mods"),
        (None, None) => anyhow::bail!("install requires either --mods-dir or --mo2-instance-dir"),
    };
    // Staging hangs off the mods folder's parent, so a relative path must not
    // reach it as "": that would be `/`, and extraction would fall back to the
    // system temp dir.
    let mods_dir = std::path::absolute(&mods_dir)
        .map_err(|err| anyhow::anyhow!("failed to resolve {}: {err}", mods_dir.display()))?;
    let settings = config::install::InstallSettings {
        cache_dir,
        mods_dir: mods_dir.clone(),
        mo2_instance_dir: args.mo2_instance_dir.clone(),
        profile_name: args.profile_name.clone(),
        game_dir: game_dir.clone(),
        execute_actions: !args.skip_actions,
        dry_run: args.dry_run,
        tools: {
            let path = ToolsConfig::default_path().unwrap_or_else(|| PathBuf::from("tools.toml"));
            ToolsConfig::load(&path)?
        },
        filter: args.filter.to_mod_filter(),
        // Forcing a reinstall without also forcing the merges would rebuild
        // the sources and leave merges built from the old ones.
        force_merges: args.force_merges || args.force,
        force: args.force,
        redo_actions: args.redo_actions.clone(),
    };

    config::install::install_all(&plan, &settings)?;

    tracing::info!(
        input = %args.input.display(),
        downloads = %settings.cache_dir.display(),
        mods_dir = %mods_dir.display(),
        mo2_instance_dir = ?args.mo2_instance_dir.as_ref().map(|p| p.display().to_string()),
        profile_name = %args.profile_name,
        game_dir = ?game_dir,
        skip_actions = args.skip_actions,
        force_merges = args.force_merges,
        force = args.force,
        dry_run = args.dry_run,
        "install requested"
    );

    Ok(())
}
