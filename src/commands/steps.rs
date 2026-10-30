//! `steps`: what a list leaves to do by hand, and what of it is done.

use crate::cli::StepsArgs;
use crate::config;
use crate::ui::{self, Tone};

pub fn run(args: StepsArgs) -> anyhow::Result<()> {
    let plan = config::pipeline::read_plan(&args.input)?;
    let game_dir = super::resolve_game_dir(args.game_dir.clone(), Some(&args.mo2_instance_dir))
        .ok()
        .flatten();
    let settings = config::install::InstallSettings {
        cache_dir: Default::default(),
        mods_dir: args.mo2_instance_dir.join("mods"),
        mo2_instance_dir: Some(args.mo2_instance_dir.clone()),
        profile_name: args.profile_name.clone(),
        game_dir,
        execute_actions: false,
        dry_run: false,
        tools: Default::default(),
        filter: Default::default(),
        force_merges: false,
        force: false,
        redo_actions: Vec::new(),
    };
    // A step's plugin built in MO2 lands in overwrite/, which every profile
    // shares: it goes into a mod of this profile's own.
    let collected = config::install::by_hand_mods::collect(&plan, &settings)?;
    for moved in &collected.moved {
        ui::status(
            Tone::Good,
            format!(
                "Moved {} out of overwrite/ into the mod {}, first in the profile's mod list",
                ui::name(&moved.plugin),
                ui::name(&moved.folder)
            ),
        );
    }
    for plugin in &collected.waiting {
        ui::status(
            Tone::Note,
            format!(
                "{} is in overwrite/; close Mod Organizer 2 and run this again to move it into \
                 a mod of this profile's own",
                ui::name(plugin)
            ),
        );
    }
    let steps = config::install::manual_steps(&plan, &settings);
    if steps.is_empty() {
        ui::status(Tone::Good, "The list leaves nothing to do by hand.");
        return Ok(());
    }
    let mut left = 0;
    for (step, done) in &steps {
        let mark = if *done {
            console::style("✔").green().to_string()
        } else {
            left += 1;
            console::style("○").yellow().to_string()
        };
        ui::say(format!("{mark} {}", ui::name(&step.title)));
        if !done {
            for line in step
                .steps
                .lines()
                .map(str::trim_end)
                .filter(|l| !l.trim().is_empty())
            {
                ui::say(format!("    {line}"));
            }
        }
    }
    ui::say("");
    ui::status(
        if left == 0 { Tone::Good } else { Tone::Warning },
        format!("{} of {} step(s) done", steps.len() - left, steps.len()),
    );
    Ok(())
}
