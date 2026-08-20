use crate::cli::{DiffArgs, DiffFormat};
use crate::config;
use crate::config::diff::{DiffSettings, GuideEra, PlanIndex};
use crate::config::schema::GuideProvenance;

pub async fn run(args: DiffArgs) -> anyhow::Result<()> {
    let mut plan_guide: Option<GuideProvenance> = None;
    let plan = match &args.plan {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
            let plan = serde_json::from_str::<config::schema::PersonalizedPlan>(&raw).map_err(
                |err| {
                    anyhow::anyhow!("failed to parse personalized plan {}: {err}", path.display())
                },
            )?;
            plan_guide = plan.guide.clone();
            Some(PlanIndex::from_plan(&plan))
        }
        None => None,
    };

    let filter = args.filter.to_mod_filter();
    // A section filter is only meaningful against a plan, because a mod folder
    // on disk does not record which section it belongs to. Failing loudly beats
    // silently comparing nothing and reporting a clean section.
    if plan.is_none() && !args.filter.sections.is_empty() {
        anyhow::bail!("--section needs --plan: a mods directory does not record section paths");
    }

    // The command line wins over the plan, so a one-off comparison can say what
    // the plan does not -- or correct it.
    let era = match &args.guide_date {
        Some(published) => GuideEra::from_provenance(Some(&GuideProvenance {
            published: published.clone(),
            file_id: args.guide_file_id,
        }))?,
        None => GuideEra::from_provenance(plan_guide.as_ref())?,
    };

    let settings = DiffSettings {
        mods_dir: args.mods_dir.clone(),
        oracle_dir: args.oracle.clone(),
        filter,
        plan,
        era,
    };

    let report = config::diff::diff_all(&settings)?;

    // Straight to stdout, not through tracing: this report is the command's
    // output, and it has to survive being piped or pasted somewhere.
    match args.format {
        DiffFormat::Text => print!("{}", config::diff::render_text(&report)),
        DiffFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }

    if report.has_differences() {
        // Non-zero so a section can be gated on it. The report is already on
        // stdout, so this only has to say why the exit code is what it is.
        anyhow::bail!(
            "{} differing, {} missing from ours, {} extra in ours",
            report.summary.differing,
            report.summary.missing,
            report.summary.extra
        );
    }

    Ok(())
}
