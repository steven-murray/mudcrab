use crate::cli::UnhideMergesArgs;
use crate::config::install::merge::unhide_merges;

pub async fn run(args: UnhideMergesArgs) -> anyhow::Result<()> {
    let mods_dir = match (&args.mods_dir, &args.mo2_instance_dir) {
        (Some(path), _) => path.clone(),
        (None, Some(instance_dir)) => instance_dir.join("mods"),
        (None, None) => {
            anyhow::bail!("unhide-merges requires either --mods-dir or --mo2-instance-dir")
        }
    };

    let (restored, recorded) = unhide_merges(
        &mods_dir,
        args.mo2_instance_dir.as_deref(),
        &args.profile_name,
    )?;

    tracing::info!(
        mods_dir = %mods_dir.display(),
        profile_name = %args.profile_name,
        restored,
        recorded,
        "unhide-merges completed"
    );

    if recorded == 0 {
        println!("No hidden plugins recorded for profile {}.", args.profile_name);
    } else {
        println!(
            "Restored {restored} of {recorded} recorded plugin(s); \
             the rest were already visible."
        );
    }

    Ok(())
}
