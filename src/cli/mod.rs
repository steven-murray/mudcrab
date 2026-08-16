use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "mudcrab",
    version,
    about = "Declarative modlist compiler and installer for TES4: Oblivion"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate and compile a source modlist into a machine-friendly artifact.
    Compile(CompileArgs),
    /// Resolve interactive inputs into a personalized install plan.
    Query(QueryArgs),
    /// Download archives required by a compiled or personalized modlist.
    Download(DownloadArgs),
    /// Validate cached archives and archive-backed file references without installing.
    Check(CheckArgs),
    /// Install mods from downloaded archives.
    Install(InstallArgs),
    /// Validate a source modlist without producing compiled output.
    Validate(ValidateArgs),
    /// Export documentation or reports from a compiled modlist.
    Export(ExportArgs),
    /// Scan a source modlist and generate a tools.toml configuration template.
    SetupTools(SetupToolsArgs),
    /// Restore plugins that were hidden to make room for a merge.
    UnhideMerges(UnhideMergesArgs),
    /// Build merged plugins from an already-installed mods directory.
    Merge(MergeArgs),
}

#[derive(Debug, Args)]
pub struct MergeArgs {
    /// Modlist TOML declaring the merges to build.
    pub input: PathBuf,
    /// Directory holding the installed mod folders, e.g. an MO2 instance's mods/.
    #[arg(long)]
    pub mods_dir: PathBuf,
    /// Where to write the merged plugins. Nothing outside this directory is
    /// touched: source mods are read only, and no plugin is hidden.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Build only the merge with this mod id.
    #[arg(long)]
    pub only: Option<String>,
}

#[derive(Debug, Args)]
pub struct UnhideMergesArgs {
    /// Installation directory containing the mod folders.
    #[arg(long)]
    pub mods_dir: Option<PathBuf>,
    /// ModOrganizer2 instance root whose profile manifest records the hides.
    #[arg(long)]
    pub mo2_instance_dir: Option<PathBuf>,
    /// Profile name whose manifest to read.
    #[arg(long, default_value = "Default")]
    pub profile_name: String,
}

#[derive(Debug, Args)]
pub struct CompileArgs {
    /// Path to source modlist TOML.
    pub input: PathBuf,
    /// Output path for compiled artifact.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Treat warnings as errors.
    #[arg(long)]
    pub strict: bool,
    /// Skip remote existence checks.
    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct QueryArgs {
    /// Path to compiled modlist artifact.
    pub input: PathBuf,
    /// Output path for personalized install plan.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Use defaults and skip all interactive prompts.
    #[arg(long)]
    pub headless: bool,
}

#[derive(Debug, Args)]
pub struct DownloadArgs {
    /// Path to personalized install plan.
    pub input: PathBuf,
    /// Cache directory for downloaded archives.
    #[arg(long)]
    pub cache: Option<PathBuf>,
    /// Maximum concurrent downloads.
    #[arg(long, default_value_t = 4)]
    pub parallel: usize,
    /// Retry attempts per failed download.
    #[arg(long, default_value_t = 3)]
    pub retry: u32,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Path to personalized install plan.
    pub input: PathBuf,
    /// Cache directory for downloaded archives.
    #[arg(long)]
    pub cache: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    /// Path to personalized install plan.
    pub input: PathBuf,
    /// Source cache directory for archives.
    #[arg(long)]
    pub cache: Option<PathBuf>,
    /// Installation output directory for plain staged installs.
    #[arg(long)]
    pub mods_dir: Option<PathBuf>,
    /// Optional ModOrganizer2 instance root to export into.
    #[arg(long)]
    pub mo2_instance_dir: Option<PathBuf>,
    /// Profile name to use when exporting to ModOrganizer2.
    #[arg(long, default_value = "Default")]
    pub profile_name: String,
    /// Optional game root for game-scoped install actions (e.g. Oblivion.ini edits).
    #[arg(long)]
    pub game_dir: Option<PathBuf>,
    /// Output directory for files declared with game_root_files in the modlist.
    /// Defaults to a 'game-root/' sibling of the cache directory.
    #[arg(long)]
    pub game_root_dir: Option<PathBuf>,
    /// Do not execute post-install actions.
    #[arg(long)]
    pub skip_actions: bool,
    /// Print planned operations without applying them.
    #[arg(long)]
    pub dry_run: bool,
    /// Path to tools.toml machine-local tool configuration.
    /// Defaults to ~/.config/mudcrab/tools.toml (or %APPDATA%\mudcrab\tools.toml on Windows).
    #[arg(long)]
    pub tools_config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Path to source modlist TOML.
    pub input: PathBuf,
    /// Treat warnings as errors.
    #[arg(long)]
    pub strict: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Path to compiled modlist artifact.
    pub input: PathBuf,
    /// Export format.
    #[arg(long, value_enum)]
    pub format: ExportFormat,
    /// Output file path.
    #[arg(short, long)]
    pub output: PathBuf,
}

#[derive(Debug, Copy, Clone, clap::ValueEnum)]
pub enum ExportFormat {
    Markdown,
    Html,
    Json,
}

#[derive(Debug, Args)]
pub struct SetupToolsArgs {
    /// Path to the source modlist TOML to scan for required tools.
    pub input: PathBuf,
    /// Output path for the generated tools.toml template.
    /// Defaults to tools.toml in the same directory as the input file.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Overwrite an existing tools.toml file.
    #[arg(long)]
    pub force: bool,
}
