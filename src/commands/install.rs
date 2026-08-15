use crate::cli::InstallArgs;
use crate::config;
use crate::config::tools::ToolsConfig;
use std::path::PathBuf;

pub async fn run(args: InstallArgs) -> anyhow::Result<()> {
    let game_dir = super::require_game_dir(args.game_dir.clone())?;
    let raw = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let plan = serde_json::from_str::<config::schema::PersonalizedPlan>(&raw).map_err(|err| {
        anyhow::anyhow!(
            "failed to parse personalized plan {}: {err}",
            args.input.display()
        )
    })?;

    let cache_dir = args
        .cache
        .clone()
        .unwrap_or_else(|| PathBuf::from(".mudcrab-cache"));
    let game_root_dir = args.game_root_dir.clone().or_else(|| {
        cache_dir.parent().map(|p| p.join("game-root"))
    });
    let mods_dir = match (&args.mods_dir, &args.mo2_instance_dir) {
        (Some(path), _) => path.clone(),
        (None, Some(instance_dir)) => instance_dir.join("mods"),
        (None, None) => anyhow::bail!("install requires either --mods-dir or --mo2-instance-dir"),
    };
    let settings = config::install::InstallSettings {
        cache_dir,
        mods_dir: mods_dir.clone(),
        mo2_instance_dir: args.mo2_instance_dir.clone(),
        profile_name: args.profile_name.clone(),
        game_dir: Some(game_dir.clone()),
        game_root_dir,
        execute_actions: !args.skip_actions,
        dry_run: args.dry_run,
        tools: {
            let path = args
                .tools_config
                .clone()
                .or_else(ToolsConfig::default_path)
                .unwrap_or_else(|| PathBuf::from("tools.toml"));
            ToolsConfig::load(&path)?
        },
    };

    config::install::install_all(&plan, &settings)?;

    tracing::info!(
        input = %args.input.display(),
        cache = ?args.cache.as_ref().map(|p| p.display().to_string()),
        mods_dir = %mods_dir.display(),
        mo2_instance_dir = ?args.mo2_instance_dir.as_ref().map(|p| p.display().to_string()),
        profile_name = %args.profile_name,
        game_dir = %game_dir.display(),
        skip_actions = args.skip_actions,
        dry_run = args.dry_run,
        "install requested"
    );

    Ok(())
}
