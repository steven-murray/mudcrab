//! `assets`: what the installed plugins name that the game would not find.

use crate::cli::AssetsArgs;
use crate::config::assets;

/// Shown per plugin unless `--all`.
const EXAMPLES: usize = 5;

pub fn run(args: AssetsArgs) -> anyhow::Result<()> {
    let instance = &args.mo2_instance_dir;
    let game_dir = super::resolve_game_dir(args.game_dir.clone(), Some(instance))?
        .ok_or_else(|| anyhow::anyhow!("no game folder: pass --game-dir"))?;
    let data = assets::VirtualData::of_instance(instance, &args.profile_name, &game_dir)?;
    let active = assets::active_plugins(instance, &args.profile_name)?;
    let report = assets::check(&active, &data)?;

    let missing: usize = report.missing.iter().map(|(_, m)| m.len()).sum();
    crate::ui::say(format!(
        "Read {} plugin(s), naming {} mesh and sound file(s): {missing} not found, in {} \
         plugin(s) ({} more the game's own plugins already lack are left out)",
        report.plugins_read,
        report.references,
        report.missing.len(),
        report.missing_in_game,
    ));
    for (plugin, files) in &report.missing {
        tracing::info!(plugin = %plugin, missing = ?files.iter().map(|m| &m.path).collect::<Vec<_>>(), "assets: missing");
        crate::ui::say(format!("\n{plugin}: {} missing", files.len()));
        let shown = if args.all { files.len() } else { EXAMPLES };
        for file in files.iter().take(shown) {
            crate::ui::say(format!("  {}  ({})", file.path, file.record));
        }
        if files.len() > shown {
            crate::ui::say(format!(
                "  ... and {} more (--all lists them)",
                files.len() - shown
            ));
        }
    }
    let shared = assets::most_named(&report);
    if !shared.is_empty() {
        crate::ui::say("\nNamed by more than one plugin (often one mod left out):");
        for (path, count) in shared.iter().take(10) {
            crate::ui::say(format!("  {path}  ({count} plugins)"));
        }
    }
    Ok(())
}
