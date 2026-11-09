use crate::cli::CheckArgs;
use crate::config;

pub async fn run(args: CheckArgs) -> anyhow::Result<()> {
    let plan = config::pipeline::read_plan(&args.input)?;

    let cache_dir = super::resolve_downloads_dir(args.downloads.clone(), None)?;
    let settings = config::check::CheckSettings {
        search: Vec::new(),
        cache_dir,
        filter: args.filter.to_mod_filter(),
        masters_only: args.masters_only,
        resources: if args.resources {
            Some(
                super::resolve_game_dir(args.game_dir.clone(), None)?.ok_or_else(|| {
                    anyhow::anyhow!("--resources needs the game folder: pass --game-dir")
                })?,
            )
        } else {
            None
        },
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
    if report.problems > 0 {
        anyhow::bail!("check found {} problem(s), listed above", report.problems);
    }

    Ok(())
}
