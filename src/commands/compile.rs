use crate::cli::CompileArgs;
use crate::config;

pub async fn run(args: CompileArgs) -> anyhow::Result<()> {
    let source = config::loader::load_modlist(&args.input)?;
    let warnings = config::validator::validate(&source)?;
    for warning in &warnings {
        tracing::warn!("{warning}");
    }
    if args.strict && !warnings.is_empty() {
        anyhow::bail!(
            "{} warning(s), and --strict makes them errors:\n{}",
            warnings.len(),
            warnings.join("\n")
        );
    }
    let compiled = config::compiler::compile(source)?;

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

    let summary = format!(
        "Compiled {} mod(s), {} plugin(s), {} input(s) {} {}",
        compiled.mod_count,
        compiled.plugin_count,
        compiled.input_count,
        crate::ui::arrow(),
        crate::ui::dim(args.output.display())
    );
    let data = serde_json::to_string_pretty(&compiled)
        .map_err(|err| anyhow::anyhow!("failed to serialize compiled artifact: {err}"))?;

    std::fs::write(&args.output, data)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", args.output.display()))?;

    tracing::info!(
        input = %args.input.display(),
        output = %args.output.display(),
        strict = args.strict,
        offline = args.offline,
        "compile requested"
    );
    crate::ui::status(crate::ui::Tone::Good, summary);

    Ok(())
}
