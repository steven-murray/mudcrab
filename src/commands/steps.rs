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
        profile_name: "Default".to_string(),
        game_dir,
        execute_actions: false,
        dry_run: true,
        tools: Default::default(),
        filter: Default::default(),
        force_merges: false,
        force: false,
        redo_actions: Vec::new(),
    };
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
