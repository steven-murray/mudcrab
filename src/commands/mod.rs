pub mod add;
pub mod compile;
pub mod check;
pub mod diff;
pub mod download;
pub mod export;
pub mod install;
pub mod merge;
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

    let value = std::env::var("GAME_DIR")
        .map_err(|_| anyhow::anyhow!("GAME_DIR must be defined (or pass --game-dir where supported)"))?;

    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("GAME_DIR is defined but empty");
    }

    Ok(PathBuf::from(trimmed))
}

pub async fn execute(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Add(args) => add::run(args).await,
        Command::Compile(args) => compile::run(args).await,
        Command::Query(args) => query::run(args).await,
        Command::Download(args) => download::run(args).await,
        Command::Check(args) => check::run(args).await,
        Command::Install(args) => install::run(args).await,
        Command::Diff(args) => diff::run(args).await,
        Command::Validate(args) => validate::run(args).await,
        Command::Export(args) => export::run(args).await,
        Command::SetupTools(args) => setup_tools::run(args).await,
        Command::UnhideMerges(args) => unhide_merges::run(args).await,
        Command::Merge(args) => merge::run(args).await,
    }
}
