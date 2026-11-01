use crate::cli::UnhideMergesArgs;
use crate::config::install::merge::unhide_merges;

pub async fn run(mut args: UnhideMergesArgs) -> anyhow::Result<()> {
    args.mo2_instance_dir = args
        .mo2_instance_dir
        .as_deref()
        .map(crate::config::mo2::layout::find_instance)
        .transpose()?;
    let mods_dir = match (&args.mods_dir, &args.mo2_instance_dir) {
        (Some(path), _) => path.clone(),
        (None, Some(instance_dir)) => crate::config::mo2::layout::mods_dir(instance_dir),
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
        crate::ui::status(
            crate::ui::Tone::Note,
            format!(
                "No hidden plugins recorded for profile {}.",
                args.profile_name
            ),
        );
    } else {
        crate::ui::status(
            crate::ui::Tone::Good,
            format!(
                "Restored {restored} of {recorded} recorded plugin(s); the rest were already \
                 visible."
            ),
        );
    }

    Ok(())
}
