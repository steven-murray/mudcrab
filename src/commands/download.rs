use crate::cli::DownloadArgs;
use crate::config;
use std::path::PathBuf;

pub async fn run(args: DownloadArgs) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let plan = serde_json::from_str::<config::schema::PersonalizedPlan>(&raw).map_err(|err| {
        anyhow::anyhow!(
            "failed to parse personalized plan {}: {err}",
            args.input.display()
        )
    })?;

    if args.parallel > 1 {
        tracing::warn!(parallel = args.parallel, "parallel downloads are not implemented yet");
    }

    let cache_dir = args
        .cache
        .clone()
        .unwrap_or_else(|| PathBuf::from(".mudcrab-cache"));
    let settings = config::download::DownloadSettings {
        cache_dir,
        retry: args.retry,
        nexus_api_key: std::env::var("NEXUS_API_KEY").ok(),
        nexus_api_base: std::env::var("NEXUS_API_BASE").ok(),
        filter: args.filter.to_mod_filter(),
        archive_search_paths: args.archive_sources.archive_search_paths.clone(),
    };

    config::download::download_all(&plan, &settings).await?;

    tracing::info!(
        input = %args.input.display(),
        cache = ?args.cache.as_ref().map(|p| p.display().to_string()),
        parallel = args.parallel,
        retry = args.retry,
        "download requested"
    );

    Ok(())
}
