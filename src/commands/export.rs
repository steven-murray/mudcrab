use crate::cli::ExportArgs;
use crate::config::loader::load_modlist;
use crate::config::schema::PersonalizedPlan;
use crate::config::validator::validate;
use crate::export::build_document;

pub async fn run(args: ExportArgs) -> anyhow::Result<()> {
    let modlist = load_modlist(&args.input)?;
    for warning in validate(&modlist)? {
        tracing::warn!("{warning}");
    }
    let plan = match &args.plan {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .map_err(|err| anyhow::anyhow!("failed to read plan {}: {err}", path.display()))?;
            Some(
                serde_json::from_str::<PersonalizedPlan>(&raw).map_err(|err| {
                    anyhow::anyhow!("failed to parse plan {}: {err}", path.display())
                })?,
            )
        }
        None => None,
    };
    let document = build_document(&modlist, plan.as_ref())?;

    match args.exporter.as_str() {
        "json" => {
            let text = serde_json::to_string_pretty(&document)? + "\n";
            match &args.output {
                Some(path) => {
                    std::fs::write(path, text).map_err(|err| {
                        anyhow::anyhow!("failed to write {}: {err}", path.display())
                    })?;
                    crate::ui::say(format!(
                        "Exported {} mod(s) as json -> {}",
                        document.mod_count,
                        path.display()
                    ));
                }
                None => print!("{text}"),
            }
            Ok(())
        }
        other => anyhow::bail!("no exporter {other:?} yet; `json` is the only one so far"),
    }
}
