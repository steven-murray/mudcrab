use crate::cli::MoveArgs;
use crate::config::add::{Placement, plan_move};

pub fn run(args: MoveArgs) -> anyhow::Result<()> {
    let placement = Placement {
        before_mod: args.before_mod.clone(),
        before_section: (!args.before_sections.is_empty()).then(|| args.before_sections.clone()),
    };
    let mut planned = None;
    let edit = crate::config::edit::plan(&args.input, |text, source| {
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
        let plan = plan_move(text, &source.mods, &args.mod_id, &section, &placement)?;
        let new_text = plan.new_text.clone();
        planned = Some(plan);
        Ok(new_text)
    })
    .map_err(|err| anyhow::anyhow!("moving '{}': {err:#}", args.mod_id))?;
    let plan = planned.expect("a planned edit ran its change");

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
