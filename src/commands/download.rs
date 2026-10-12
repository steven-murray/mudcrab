use crate::cli::DownloadArgs;
use crate::config;

pub async fn run(args: DownloadArgs) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let plan = serde_json::from_str::<config::schema::PersonalizedPlan>(&raw).map_err(|err| {
        anyhow::anyhow!(
            "failed to parse personalized plan {}: {err}",
            args.input.display()
        )
    })?;

    let cache_dir = super::resolve_downloads_dir(args.downloads.clone(), None, None)?;
    let settings = config::download::DownloadSettings {
        cache_dir,
        retry: args.retry,
        nexus_api_key: crate::config::tools::ToolsConfig::nexus_api_key(),
        nexus_api_base: std::env::var("NEXUS_API_BASE").ok(),
        filter: args.filter.to_mod_filter(),
        archive_search_paths: args.archive_sources.archive_search_paths.clone(),
        parallel: args.parallel,
    };

    // Pages are opened for what cannot be fetched directly, unless told not
    // to or there is nobody at a terminal, or no browser, to click.
    let browser = args.browser || (!args.no_browser && super::download_browser::available());
    if browser {
        let unfetched = config::download::download_report(&plan, &settings).await?;
        super::download_browser::run(
            unfetched,
            &settings.cache_dir,
            args.batch,
            args.watch.clone(),
            args.browser_downloads,
        )
        .await?;
    } else {
        config::download::download_all(&plan, &settings).await?;
    }

    tracing::info!(
        input = %args.input.display(),
        downloads = %settings.cache_dir.display(),
        parallel = args.parallel,
        retry = args.retry,
        "download requested"
    );

    Ok(())
}
