use crate::config::filter::ModFilter;
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
    /// Add a mod to a source modlist, preserving its comments and formatting.
    Add(AddArgs),
    /// Validate and compile a source modlist into a machine-friendly artifact.
    Compile(CompileArgs),
    /// Resolve interactive inputs into a personalized install plan.
    Query(QueryArgs),
    /// Download archives required by a compiled or personalized modlist.
    Download(DownloadArgs),
    /// Validate cached archives and archive-backed file references without installing.
    Check(CheckArgs),
    /// Report an archive's layout, FOMOD options and plugins, for writing its
    /// modlist entry.
    Inspect(InspectArgs),
    /// Ask Nexus which mod and file an archive is, by its MD5.
    Identify(IdentifyArgs),
    /// Install mods from downloaded archives.
    Install(InstallArgs),
    /// Compare an installed mods directory against a reference MO2 instance.
    Diff(DiffArgs),
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

/// Add one `[[mods]]` block to a source modlist.
///
/// The modlist is edited in place by line-based splice, never by re-emitting
/// parsed TOML: these files carry their reasoning in comments and a
/// parse-and-rewrite would delete all of it.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// Path to the source modlist TOML to edit in place.
    pub input: PathBuf,
    /// Read the mod's details from `<DIR>/<--mod>/meta.ini`, as written by
    /// Mod Organizer 2. Only ever read from.
    #[arg(long, value_name = "ORACLE_MODS_DIR", conflicts_with = "nexus")]
    pub from_oracle: Option<PathBuf>,
    /// Name of the mod folder inside the Oracle mods directory.
    #[arg(long = "mod", value_name = "FOLDER", requires = "from_oracle")]
    pub mod_folder: Option<String>,
    /// Nexus source as `<modid>/<fileid>`, for a mod with no Oracle folder.
    #[arg(long, value_name = "MODID/FILEID")]
    pub nexus: Option<String>,
    /// Mod id, which is also the directory it installs into.
    /// Defaults to the Oracle folder name; required with --nexus.
    #[arg(long, value_name = "MOD_ID")]
    pub id: Option<String>,
    /// Section to file the mod under. Repeat for a nested section path,
    /// outermost first. The block is inserted after that section's last mod.
    #[arg(long = "section", value_name = "NAME")]
    pub sections: Vec<String>,
    /// The archive's own filename. Taken from the Oracle `installationFile=`
    /// when not given.
    #[arg(long = "file-name", value_name = "ARCHIVE")]
    pub file_name: Option<String>,
    /// When --section names a section the file does not have yet, start it
    /// immediately before this existing section instead of at end of file.
    /// Repeat for a nested path, as with --section.
    ///
    /// Section order in the file is section order in MO2, so a section
    /// belonging mid-list cannot simply be appended.
    #[arg(long = "before-section", value_name = "NAME")]
    pub before_sections: Vec<String>,
    /// Insert directly before this existing mod instead of at the end of the
    /// section.
    ///
    /// Order within a section is MO2 priority order, which decides who wins a
    /// file conflict. To put a row first in its section, name the row that
    /// currently comes first.
    #[arg(long = "before-mod", value_name = "MOD_ID")]
    pub before_mod: Option<String>,
    /// Write the block with a `manual:` source instead of a Nexus descriptor.
    ///
    /// For an archive no automated fetch can reach, and for the case the Oracle
    /// leaves half-recorded: a real `modid` with no `fileid`, which is not
    /// enough to build a download URL. The archive still resolves from
    /// `--archive-search-path` by filename, so the build proceeds; a TODO in
    /// the block says what is still missing.
    #[arg(long)]
    pub manual: bool,
    /// Print the block and where it would go, and change nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// Recover a Nexus descriptor for an archive downloaded by hand.
///
/// Nexus indexes every file it serves by MD5, which is how Mod Organizer 2
/// recognises a file you fetched yourself. Same API call, so an archive with no
/// `.meta` sidecar does not have to be round-tripped through MO2's UI before it
/// can become a modlist entry.
#[derive(Debug, Args)]
pub struct IdentifyArgs {
    /// Archives to identify.
    #[arg(required = true)]
    pub archives: Vec<PathBuf>,
    /// Nexus game domain.
    #[arg(long, default_value = "oblivion")]
    pub game: String,
    /// Mod id to search when the MD5 index has no entry for the archive.
    ///
    /// Nexus does not index every file it serves by hash -- a download moved to
    /// a mod's OLD FILES section typically has none. The fallback lists that
    /// mod's published files and matches on filename. Read from the filename
    /// automatically when it looks like `<title>-<mod id>-...`, which is how
    /// Nexus names downloads, so this is rarely needed.
    #[arg(long)]
    pub mod_id: Option<u64>,
    /// Also write the `.meta` sidecar MO2 would have written, so later runs
    /// need no lookup.
    #[arg(long)]
    pub write_meta: bool,
    /// Override the API base, for testing against a stub.
    #[arg(long, hide = true)]
    pub api_base: Option<String>,
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

/// Narrow a command to part of the modlist.
///
/// Shared by `download`, `check` and `install` so the three always agree on
/// what a given pair of flags selects -- you can download a section, check it,
/// then install it with the same arguments. `compile` and `query` deliberately
/// have no filter: they are cheap, and a partial compiled artifact would make
/// every later stage operate on a modlist that no longer describes the list.
#[derive(Debug, Default, Args)]
pub struct FilterArgs {
    /// Only act on mods in this section. Matches any level of a mod's section
    /// path, case-insensitively, so a parent section selects everything nested
    /// under it. Repeatable.
    #[arg(long = "section", value_name = "NAME")]
    pub sections: Vec<String>,
    /// Only act on this mod id, matched exactly. Repeatable, and unions with
    /// --section rather than intersecting it.
    #[arg(long = "only", value_name = "MOD_ID")]
    pub only: Vec<String>,
}

impl FilterArgs {
    pub fn to_mod_filter(&self) -> ModFilter {
        ModFilter::new(&self.sections, &self.only)
    }
}

/// Point a command at archives this machine already has.
///
/// Shared by `download`, `check` and `install` for the same reason as
/// `FilterArgs`: the three have to agree on what is available, or `check` would
/// report a download that `install` then does not need.
#[derive(Debug, Default, Args)]
pub struct ArchiveSourceArgs {
    /// Directory to search for archives that are already downloaded, before
    /// fetching anything. An archive is matched by its `file_name` from the
    /// modlist, compared case-insensitively, and a hit is hard-linked into the
    /// cache rather than copied. Repeatable and tried in the order given, first
    /// hit wins. These directories are only ever read from.
    #[arg(long = "archive-search-path", value_name = "DIR")]
    pub archive_search_paths: Vec<PathBuf>,
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
    #[command(flatten)]
    pub filter: FilterArgs,
    #[command(flatten)]
    pub archive_sources: ArchiveSourceArgs,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Path to personalized install plan.
    pub input: PathBuf,
    /// Cache directory for downloaded archives.
    #[arg(long)]
    pub cache: Option<PathBuf>,
    #[command(flatten)]
    pub filter: FilterArgs,
    #[command(flatten)]
    pub archive_sources: ArchiveSourceArgs,
}

/// Read an archive and print what its `[[mods.archives]]` block needs to say.
///
/// Configuring a mod otherwise means extracting the archive by hand and
/// transcribing `fomod/ModuleConfig.xml` into TOML, where a typo only surfaces
/// at install time. Nothing is written and no plan is involved: this takes an
/// archive path so it can be run on a download before the modlist mentions it.
#[derive(Debug, Args)]
pub struct InspectArgs {
    /// Path to the archive to read. Only ever read from.
    pub archive: PathBuf,
    /// List every file in the archive. Off by default, because a texture pack
    /// would otherwise bury the layout and FOMOD report under 4000 lines.
    #[arg(long)]
    pub files: bool,
    /// Report format.
    #[arg(long, value_enum, default_value_t = InspectFormat::Text)]
    pub format: InspectFormat,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum InspectFormat {
    /// Report meant to be read, with a TOML snippet to paste.
    Text,
    /// The same data, structured.
    Json,
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
    /// Rebuild every merge in scope, even one whose recorded inputs are
    /// unchanged. Merges are otherwise skipped when the spec, the load order
    /// and every source plugin are exactly as they were at the last build.
    #[arg(long)]
    pub force_merges: bool,
    /// Reinstall every mod in scope, even one whose recorded definition is
    /// unchanged.
    ///
    /// The skip fingerprint covers the plan, not mudcrab, so a mod is
    /// otherwise skipped even when the code that installs it has changed --
    /// a new layout, a fixed action. Implies --force-merges.
    #[arg(long)]
    pub force: bool,
    /// Print planned operations without applying them.
    #[arg(long)]
    pub dry_run: bool,
    /// Path to tools.toml machine-local tool configuration.
    /// Defaults to ~/.config/mudcrab/tools.toml (or %APPDATA%\mudcrab\tools.toml on Windows).
    #[arg(long)]
    pub tools_config: Option<PathBuf>,
    #[command(flatten)]
    pub filter: FilterArgs,
    #[command(flatten)]
    pub archive_sources: ArchiveSourceArgs,
}

/// Compare what we installed against a reference ("Oracle") MO2 instance.
///
/// Verification is per section, not per list: a 700-mod list is only
/// reproducible if each section can be signed off as it is built, while the
/// decisions that produced it are still fresh.
#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Installation directory holding the mod folders we produced.
    #[arg(long)]
    pub mods_dir: PathBuf,
    /// The reference instance's mods directory. Only ever read from.
    #[arg(long)]
    pub oracle: PathBuf,
    /// Personalized install plan, which supplies each mod's section and the
    /// archive it was meant to come from. Without it every directory present in
    /// either tree is compared, and `--section` has nothing to match against.
    #[arg(long)]
    pub plan: Option<PathBuf>,
    /// Report format.
    #[arg(long, value_enum, default_value_t = DiffFormat::Text)]
    pub format: DiffFormat,
    #[command(flatten)]
    pub filter: FilterArgs,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum DiffFormat {
    /// Compact per-section report meant to be read, or pasted into a review.
    Text,
    /// The same data, structured.
    Json,
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
