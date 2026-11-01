//! `assets`: what the installed plugins name that the game would not find.

use crate::cli::AssetsArgs;
use crate::config::assets;
use crate::ui::Tone;

/// Shown per plugin unless `--all`.
const EXAMPLES: usize = 5;

pub fn run(args: AssetsArgs) -> anyhow::Result<()> {
    let instance = &crate::config::mo2::layout::find_instance(&args.mo2_instance_dir)?;
    let game_dir = super::resolve_game_dir(args.game_dir.clone(), Some(instance))?
        .ok_or_else(|| anyhow::anyhow!("no game folder: pass --game-dir"))?;
    let data = assets::VirtualData::of_instance(instance, &args.profile_name, &game_dir)?;
    let active = assets::active_plugins(instance, &args.profile_name)?;
    let report = assets::check(&active, &data)?;

    let missing: usize = report.missing.iter().map(|(_, m)| m.len()).sum();
    for (plugin, files) in &report.missing {
        tracing::info!(plugin = %plugin, missing = ?files.iter().map(|m| &m.path).collect::<Vec<_>>(), "assets: missing");
        let shown = if args.all { files.len() } else { EXAMPLES };
        let mut items: Vec<String> = files
            .iter()
            .take(shown)
            .map(|file| format!("{}  {}", file.path, crate::ui::dim(&file.record)))
            .collect();
        if files.len() > shown {
            items.push(crate::ui::dim(format!(
                "... and {} more (--all lists them)",
                files.len() - shown
            )));
        }
        crate::ui::heading(
            Tone::Warning,
            format!(
                "{} {}",
                crate::ui::name(plugin),
                crate::ui::dim(format!("({} missing)", files.len()))
            ),
        );
        for item in items {
            crate::ui::bullet(Tone::Warning, item);
        }
    }
    let shared: Vec<String> = assets::most_named(&report)
        .into_iter()
        .take(10)
        .map(|(path, count)| format!("{path}  {}", crate::ui::dim(format!("({count} plugins)"))))
        .collect();
    crate::ui::section(
        Tone::Note,
        "Named by more than one plugin (often one mod left out)",
        &shared,
    );
    crate::ui::say("");
    crate::ui::status(
        if missing > 0 {
            Tone::Warning
        } else {
            Tone::Good
        },
        format!(
            "Read {} plugin(s), naming {} mesh and sound file(s): {missing} not found, in {} \
             plugin(s) {}",
            report.plugins_read,
            report.references,
            report.missing.len(),
            crate::ui::dim(format!(
                "({} more the game's own plugins already lack are left out)",
                report.missing_in_game
            )),
        ),
    );
    Ok(())
}
