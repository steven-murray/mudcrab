pub mod add;
mod bsa;
mod dds;
mod downloads;
pub mod check;
pub mod compile;
pub mod conflicts;
pub mod diff;
pub mod download;
pub mod export;
pub mod identify;
pub mod inspect;
pub mod install;
pub mod merge;
mod new_merge;
pub mod query;
pub mod setup_tools;
pub mod unhide_merges;
pub mod validate;

use crate::cli::{Cli, Command};
use std::path::PathBuf;

pub fn require_game_dir(explicit: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }

    let value = std::env::var("GAME_DIR").map_err(|_| {
        anyhow::anyhow!("GAME_DIR must be defined (or pass --game-dir where supported)")
    })?;

    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("GAME_DIR is defined but empty");
    }

    Ok(PathBuf::from(trimmed))
}

/// The downloads folder: `--downloads`, else `downloads_dir` in tools.toml,
/// else an MO2 instance's own `downloads/`.
pub fn resolve_downloads_dir(
    explicit: Option<PathBuf>,
    tools_config: Option<&std::path::Path>,
    mo2_instance: Option<&std::path::Path>,
) -> anyhow::Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(dir);
    }
    let tools_path = tools_config
        .map(std::path::Path::to_path_buf)
        .or_else(crate::config::tools::ToolsConfig::default_path);
    if let Some(path) = tools_path
        && let Some(dir) = crate::config::tools::ToolsConfig::load(&path)?.downloads_dir
    {
        return Ok(dir);
    }
    if let Some(instance) = mo2_instance {
        return Ok(instance.join("downloads"));
    }
    anyhow::bail!(
        "no downloads folder: pass --downloads <DIR>, or set downloads_dir in tools.toml"
    )
}

pub async fn execute(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Add(args) => add::run(args).await,
        Command::Compile(args) => compile::run(args).await,
        Command::Conflicts(args) => conflicts::run(args).await,
        Command::Query(args) => query::run(args).await,
        Command::Download(args) => download::run(args).await,
        Command::Check(args) => check::run(args).await,
        Command::Inspect(args) => inspect::run(args).await,
        Command::Identify(args) => identify::run(args).await,
        Command::Install(args) => install::run(args).await,
        Command::Diff(args) => diff::run(args).await,
        Command::Validate(args) => validate::run(args).await,
        Command::Export(args) => export::run(args).await,
        Command::SetupTools(args) => setup_tools::run(args).await,
        Command::UnhideMerges(args) => unhide_merges::run(args).await,
        Command::Merge(args) => merge::run(args).await,
        Command::NewMerge(args) => new_merge::run(args).await,
        Command::Bsa(command) => bsa::run(command),
        Command::Dds(command) => dds::run(command),
        Command::Downloads(command) => downloads::run(command).await,
    }
}
