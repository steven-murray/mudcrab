//! `includes` and `update`: what a list includes, and moving its git
//! includes on.

use crate::cli::{IncludesArgs, UpdateArgs};
use crate::config::{compiler::origin_name, git, loader};
use crate::ui::{self, Tone};

pub fn list(args: IncludesArgs) -> anyhow::Result<()> {
    let source = loader::load_modlist(&args.input)?;
    if source.includes.is_empty() {
        ui::status(Tone::Note, "The list includes nothing");
        return Ok(());
    }
    let base = source.base_dir.clone().unwrap_or_default();
    for included in &source.includes {
        let mods = source
            .mods
            .iter()
            .filter(|entry| entry.origin.as_ref() == Some(&included.file))
            .count();
        let indent = "  ".repeat(included.depth - 1);
        ui::data(format!(
            "{indent}{}  {}  {}",
            ui::name(&included.id),
            origin_name(&base, &included.file),
            ui::dim(format!("{mods} mod(s)"))
        ));
    }
    for warning in &source.include_warnings {
        ui::status(Tone::Warning, warning);
    }
    Ok(())
}

pub fn update(args: UpdateArgs) -> anyhow::Result<()> {
    let (update, mut lock) = git::plan_update(&args.input, &args.includes)?;
    if update.moved.is_empty() {
        ui::status(Tone::Good, "Every git include is up to date");
        return Ok(());
    }
    let short = |commit: &str| commit[..commit.len().min(7)].to_string();
    for moved in &update.moved {
        ui::data(format!(
            "{}  {}  {} {} {}",
            ui::name(&moved.id),
            ui::dim(&moved.source),
            moved.from.as_deref().map_or("(new)".to_string(), short),
            ui::arrow(),
            short(&moved.to)
        ));
    }
    for (what, ids) in [
        ("added", &update.added),
        ("removed", &update.removed),
        ("changed", &update.changed),
    ] {
        if !ids.is_empty() {
            ui::data(format!("{} mod(s) {what}: {}", ids.len(), ids.join(", ")));
        }
    }
    if args.dry_run {
        ui::say(ui::dim("(--dry-run: the lock file is as it was)"));
        return Ok(());
    }
    lock.save()?;
    ui::status(
        Tone::Good,
        format!(
            "Updated {} include(s) {} {}",
            update.moved.len(),
            ui::arrow(),
            ui::dim(git::lock_path(&args.input).display())
        ),
    );
    Ok(())
}
