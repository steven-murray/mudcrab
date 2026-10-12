use crate::cli::QueryArgs;
use crate::config;

pub async fn run(args: QueryArgs) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;

    let compiled =
        serde_json::from_str::<config::schema::CompiledModlist>(&raw).map_err(|err| {
            anyhow::anyhow!(
                "failed to parse compiled artifact {}: {err}",
                args.input.display()
            )
        })?;

    let overrides = config::query::parse_overrides(&args.set)?;
    let plan = config::query::build_plan(&compiled, args.headless, &overrides)?;

    if let Some(parent) = args.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| {
            anyhow::anyhow!(
                "failed to create output directory {}: {err}",
                parent.display()
            )
        })?;
    }

    let mut answers: Vec<String> = plan
        .responses
        .iter()
        .map(|(key, value)| match value {
            config::schema::InputValue::Bool(b) => format!("{key}={b}"),
            config::schema::InputValue::Text(t) => format!("{key}={t}"),
        })
        .collect();
    answers.sort();
    let summary = format!(
        "Plan: {} of {} mod(s) selected{} {} {}",
        plan.selected_mods.len(),
        plan.mod_order.len(),
        if answers.is_empty() {
            String::new()
        } else {
            format!(" ({})", answers.join(", "))
        },
        crate::ui::arrow(),
        crate::ui::dim(args.output.display())
    );
    let data = serde_json::to_string_pretty(&plan)
        .map_err(|err| anyhow::anyhow!("failed to serialize query output: {err}"))?;
    std::fs::write(&args.output, data)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", args.output.display()))?;

    tracing::info!(
        input = %args.input.display(),
        output = %args.output.display(),
        headless = args.headless,
        "query requested"
    );
    crate::ui::status(crate::ui::Tone::Good, summary);

    Ok(())
}
