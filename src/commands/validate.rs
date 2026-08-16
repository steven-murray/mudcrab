use crate::cli::ValidateArgs;
use crate::config;

pub async fn run(args: ValidateArgs) -> anyhow::Result<()> {
    let source = config::loader::load_modlist(&args.input)?;
    config::validator::validate(&source)?;

    tracing::info!(
        input = %args.input.display(),
        strict = args.strict,
        "validate requested"
    );

    Ok(())
}
