use crate::cli::CheckArgs;
use crate::config;
use std::path::PathBuf;

pub async fn run(args: CheckArgs) -> anyhow::Result<()> {
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
    let settings = config::check::CheckSettings {
        cache_dir,
        filter: args.filter.to_mod_filter(),
        archive_search_paths: args.archive_sources.archive_search_paths.clone(),
    };

    let report = config::check::check_all(&plan, &settings)?;

    tracing::info!(
        input = %args.input.display(),
        cache = ?args.cache.as_ref().map(|p| p.display().to_string()),
        mods_checked = report.mods_checked,
        archives_checked = report.archives_checked,
        references_checked = report.file_references_checked,
        cached = report.archives_cached,
        resolvable_locally = report.archives_local,
        must_be_downloaded = report.archives_must_download,
        "check completed"
    );

    Ok(())
}
