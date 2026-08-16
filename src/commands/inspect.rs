use crate::cli::{InspectArgs, InspectFormat};
use crate::config;

pub async fn run(args: InspectArgs) -> anyhow::Result<()> {
    let report = config::inspect::inspect_archive(&args.archive, args.files)?;

    // Straight to stdout, not through tracing: the report is the command's
    // output, and the TOML snippet in it has to survive being piped into an
    // editor or pasted into a modlist.
    match args.format {
        InspectFormat::Text => print!("{}", config::inspect::render_text(&report)),
        InspectFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }

    Ok(())
}
