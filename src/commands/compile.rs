use crate::cli::CompileArgs;
use crate::config;

pub async fn run(args: CompileArgs) -> anyhow::Result<()> {
    let config::pipeline::Compiled {
        modlist: compiled,
        warnings,
    } = config::pipeline::compile(&args.input, args.strict)?;
    for warning in &warnings {
        tracing::warn!("{warning}");
    }
    config::pipeline::write_json(&args.output, &compiled)?;
    let summary = format!(
        "Compiled {} mod(s), {} plugin(s), {} input(s) {} {}",
        compiled.mod_count,
        compiled.plugin_count,
        compiled.input_count,
        crate::ui::arrow(),
        crate::ui::dim(args.output.display())
    );

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
