use crate::cli::CompileArgs;
use crate::config;

pub async fn run(args: CompileArgs) -> anyhow::Result<()> {
    let source = config::loader::load_modlist(&args.input)?;
    for warning in config::validator::validate(&source)? {
        tracing::warn!("{warning}");
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

    Ok(())
}
