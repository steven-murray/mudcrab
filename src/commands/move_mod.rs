use crate::cli::MoveArgs;
use crate::config::add::Placement;

pub fn run(args: MoveArgs) -> anyhow::Result<()> {
    let placement = Placement {
        before_mod: args.before_mod.clone(),
        before_section: (!args.before_sections.is_empty()).then(|| args.before_sections.clone()),
    };
    let sections = (!args.sections.is_empty()).then_some(args.sections.as_slice());
    let (edit, plan) =
        crate::config::edit::plan_mod_move(&args.input, &args.mod_id, sections, &placement)
            .map_err(|err| anyhow::anyhow!("moving '{}': {err:#}", args.mod_id))?;

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

    edit.write()?;
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
