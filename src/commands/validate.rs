use crate::cli::ValidateArgs;
use crate::config;

pub async fn run(args: ValidateArgs) -> anyhow::Result<()> {
    let source = config::loader::load_modlist(&args.input)?;
    let warnings = config::validator::validate(&source)?;

    for warning in &warnings {
        tracing::warn!("{warning}");
    }

    if args.strict && !warnings.is_empty() {
        anyhow::bail!(
            "{} warning{}, and --strict was given",
            warnings.len(),
            if warnings.len() == 1 { "" } else { "s" }
        );
    }

    tracing::info!(
        input = %args.input.display(),
        strict = args.strict,
        warnings = warnings.len(),
        "validate requested"
    );

    Ok(())
}
