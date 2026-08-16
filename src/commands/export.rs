use crate::cli::{ExportArgs, ExportFormat};

pub async fn run(args: ExportArgs) -> anyhow::Result<()> {
    let format = match args.format {
        ExportFormat::Markdown => "markdown",
        ExportFormat::Html => "html",
        ExportFormat::Json => "json",
    };

    tracing::info!(
        input = %args.input.display(),
        output = %args.output.display(),
        format,
        "export requested"
    );

    anyhow::bail!("export is not implemented yet")
}
