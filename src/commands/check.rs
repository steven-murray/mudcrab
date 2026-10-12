use crate::cli::CheckArgs;
use crate::config;

pub async fn run(args: CheckArgs) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let plan = serde_json::from_str::<config::schema::PersonalizedPlan>(&raw).map_err(|err| {
        anyhow::anyhow!(
            "failed to parse personalized plan {}: {err}",
            args.input.display()
        )
    })?;

    let cache_dir = super::resolve_downloads_dir(args.downloads.clone(), None)?;
    let settings = config::check::CheckSettings {
        cache_dir,
        filter: args.filter.to_mod_filter(),
    };

    let report = config::check::check_all(&plan, &settings)?;

    tracing::info!(
        input = %args.input.display(),
        downloads = %settings.cache_dir.display(),
        mods_checked = report.mods_checked,
        archives_checked = report.archives_checked,
        plugins_checked = report.plugins_checked,
        "check completed"
    );

    Ok(())
}
