use crate::cli::QueryArgs;
use crate::config;

pub async fn run(args: QueryArgs) -> anyhow::Result<()> {
    let compiled = config::pipeline::read_compiled(&args.input)?;

    let overrides = config::query::parse_overrides(&args.set)?;
    let mut ask = crate::commands::prompt::input;
    let plan = config::query::build_plan(
        &compiled,
        &overrides,
        (!args.headless).then_some(&mut ask as config::query::Ask),
    )?;

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
    config::pipeline::write_json(&args.output, &plan)?;

    tracing::info!(
        input = %args.input.display(),
        output = %args.output.display(),
        headless = args.headless,
        "query requested"
    );
    crate::ui::status(crate::ui::Tone::Good, summary);

    Ok(())
}
