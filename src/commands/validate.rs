use crate::cli::ValidateArgs;
use crate::config;

pub async fn run(args: ValidateArgs) -> anyhow::Result<()> {
    let game_dir = super::require_game_dir(None)?;
    let source = config::loader::load_modlist(&args.input)?;
    config::validator::validate(&source)?;

    tracing::info!(
        input = %args.input.display(),
        game_dir = %game_dir.display(),
        strict = args.strict,
        "validate requested"
    );

    Ok(())
}
