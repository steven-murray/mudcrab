use crate::cli::{InspectArgs, InspectFormat};
use crate::config;
use crate::util::fs::eq_ci;

pub async fn run(args: InspectArgs) -> anyhow::Result<()> {
    if !args.archive.exists() {
        anyhow::bail!("archive does not exist: {}", args.archive.display());
    }

    // A BSA is a game archive, not an installer package: no layout to guess and
    // no modlist entry to paste. It gets its own report.
    let is_bsa = args
        .archive
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| eq_ci(ext, "bsa"));

    // Straight to stdout, not through tracing: the report is the command's
    // output, and the TOML snippet in it has to survive being piped into an
    // editor or pasted into a modlist.
    if is_bsa {
        let report = config::inspect::inspect_bsa(&args.archive, args.files)?;
        match args.format {
            InspectFormat::Text => print!("{}", config::inspect::render_bsa_text(&report)),
            InspectFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        }
        return Ok(());
    }

    let report = config::inspect::inspect_archive(&args.archive, args.files)?;
    match args.format {
        InspectFormat::Text => print!("{}", config::inspect::render_text(&report)),
        InspectFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }

    Ok(())
}
