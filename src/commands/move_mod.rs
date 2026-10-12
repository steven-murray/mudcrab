use crate::cli::MoveArgs;
use crate::commands::add::{reload_and_validate, write_atomic};
use crate::config::add::{Placement, plan_move};
use crate::config::loader::load_modlist;

pub fn run(args: MoveArgs) -> anyhow::Result<()> {
    let original = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let source = load_modlist(&args.input)?;
    let current = source
        .mods
        .iter()
        .find(|entry| entry.id == args.mod_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} has no mod with id '{}'",
                args.input.display(),
                args.mod_id
            )
        })?;
    let section = if args.sections.is_empty() {
        current.section.clone()
    } else {
        args.sections.clone()
    };

    let placement = Placement {
        before_mod: args.before_mod.clone(),
        before_section: (!args.before_sections.is_empty()).then(|| args.before_sections.clone()),
    };
    let plan = plan_move(&original, &source.mods, &args.mod_id, &section, &placement)?;

    if args.dry_run {
        crate::ui::status(
            crate::ui::Tone::Note,
            format!(
                "Would move {} to line {}, {}:\n",
                crate::ui::name(&args.mod_id),
                plan.line,
                plan.anchor
            ),
        );
        println!("{}", plan.block);
        crate::ui::say(crate::ui::dim("\n(--dry-run: nothing written)"));
        return Ok(());
    }

    write_atomic(&args.input, &plan.new_text)?;
    if let Err(err) = reload_and_validate(&args.input) {
        write_atomic(&args.input, &original)?;
        anyhow::bail!(
            "moving '{}' produced a modlist that no longer validates: {err}. The original file has \
             been restored unchanged.",
            args.mod_id
        );
    }
    crate::ui::status(
        crate::ui::Tone::Good,
        format!(
            "Moved {} to line {}, {}",
            crate::ui::name(&args.mod_id),
            plan.line,
            plan.anchor
        ),
    );
    Ok(())
}
