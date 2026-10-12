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
    /// Show the full log on the terminal instead of progress bars. The log
    /// file is written either way.
    #[arg(short, long, global = true)]
    pub verbose: bool,
    /// Write the full log here instead of the default
    /// (~/.local/state/mudcrab/logs/<command>-<time>.log).
    #[arg(long, global = true, value_name = "FILE")]
    pub log_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add a mod to a source modlist, preserving its comments and formatting.
    Add(AddArgs),
    /// Move a mod to another section or position, comments and all.
    Move(MoveArgs),
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
    /// List the files a Nexus mod publishes, with the descriptor for each.
    NexusFiles(NexusFilesArgs),
    /// List the files two mods both provide, which is what `conflicts_with`
    /// acts on.
    Conflicts(ConflictsArgs),
    /// Install mods from downloaded archives.
    Install(InstallArgs),
    /// Compare an installed mods directory against a reference MO2 instance.
    Diff(DiffArgs),
    /// Validate a source modlist without producing compiled output.
    Validate(ValidateArgs),
    /// Render a modlist as a guide: JSON, or through an exporter's templates.
    Export(ExportArgs),
    /// Scan a source modlist and generate a tools.toml configuration template.
    SetupTools(SetupToolsArgs),
    /// Restore plugins that were hidden to make room for a merge.
    UnhideMerges(UnhideMergesArgs),
    /// Build merged plugins from an already-installed mods directory.
    Merge(MergeArgs),
    /// Write a merge modlist from a bare list of plugin names.
    NewMerge(NewMergeArgs),
    /// List or extract the files inside a BSA archive.
    #[command(subcommand)]
    Bsa(BsaCommand),
    /// Inspect DDS textures.
    #[command(subcommand)]
    Dds(DdsCommand),
    /// Manage the downloads folder.
    #[command(subcommand)]
    Downloads(DownloadsCommand),
}

/// Add one `[[mods]]` block to a source modlist.
///
/// The modlist is edited in place by line-based splice, never by re-emitting
/// parsed TOML: these files carry their reasoning in comments and a
/// parse-and-rewrite would delete all of it.
/// Move a mod's block within a source modlist: to another section, or before
/// another mod. Its comments, archives and actions go with it; only its
/// `section` line changes. Validated afterwards, and undone if that fails.
#[derive(Debug, Args)]
pub struct MoveArgs {
    /// Path to the source modlist TOML to edit in place.
    pub input: PathBuf,
    /// The mod to move.
    #[arg(long = "mod", value_name = "MOD_ID")]
    pub mod_id: String,
    /// The section to move it to; repeat for a nested path. Defaults to the
    /// section it is in.
    #[arg(long = "section", value_name = "NAME")]
    pub sections: Vec<String>,
    /// Put it directly before this mod rather than at the end of the section.
    #[arg(long = "before-mod", value_name = "MOD_ID")]
    pub before_mod: Option<String>,
    /// When the section does not exist yet, start it before this one.
    #[arg(long = "before-section", value_name = "NAME")]
    pub before_sections: Vec<String>,
    /// Print where it would go, and change nothing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Path to the source modlist TOML to edit in place.
    pub input: PathBuf,
    /// Take the mod from an MO2 instance where it is already installed:
    /// `<MODS_DIR>/<--mod>/meta.ini`, as MO2 writes it, gives the Nexus ids,
    /// archive name and version. For moving a list built by hand in MO2 into
    /// a modlist. Only ever read from.
    #[arg(
        long = "from-mo2",
        alias = "from-oracle",
        value_name = "MODS_DIR",
        conflicts_with_all = ["nexus", "archive"]
    )]
    pub from_mo2: Option<PathBuf>,
    /// Write the archive entry from the archive itself: its source (from the
    /// MO2 `.meta` beside it, or --nexus / --manual), file name, layout, FOMOD
    /// defaults and BAIN packages, as `inspect` reports them. Only ever read
    /// from.
    ///
    /// In a terminal, a FOMOD installer or BAIN package is walked through
    /// question by question and your answers written as the entry's
    /// selections; Enter takes the installer's default.
    #[arg(long, value_name = "FILE")]
    pub archive: Option<PathBuf>,
    /// With --archive, take every installer default instead of asking.
    /// Also what happens when input is not a terminal.
    #[arg(long, requires = "archive")]
    pub defaults: bool,
    /// Name of the mod's folder in the --from-mo2 mods directory.
    #[arg(long = "mod", value_name = "FOLDER", requires = "from_mo2")]
    pub mod_folder: Option<String>,
    /// Nexus source as `<modid>/<fileid>`.
    /// With --archive, the source when the archive has no `.meta` naming it.
    #[arg(long, value_name = "MODID/FILEID")]
    pub nexus: Option<String>,
    /// Mod id, which is also the directory it installs into.
    /// Defaults to the MO2 folder name with --from-mo2; required otherwise.
    #[arg(long, value_name = "MOD_ID")]
    pub id: Option<String>,
    /// Section to file the mod under. Repeat for a nested section path,
    /// outermost first. The block is inserted after that section's last mod.
    #[arg(long = "section", value_name = "NAME")]
    pub sections: Vec<String>,
    /// The archive's own filename. With --from-mo2, taken from `meta.ini`'s
    /// `installationFile=` when not given.
    #[arg(long = "file-name", value_name = "ARCHIVE")]
    pub file_name: Option<String>,
    /// The mod's `if`, for an optional mod, e.g. `swearing_rats` or
    /// `mod:"Some Other Mod"`.
    #[arg(long = "if", value_name = "CONDITION")]
    pub condition: Option<String>,
    /// A plugin the mod ships; repeat for several. Written into the entry, and
    /// into the top-level `plugins` load order where --load-after or
    /// --load-last says. A plugin's place in the load order matters, so
    /// mudcrab does not pick one for you.
    #[arg(long = "plugin", value_name = "FILE")]
    pub plugins: Vec<String>,
    /// Put the --plugin files into the load order straight after this plugin.
    #[arg(
        long = "load-after",
        value_name = "PLUGIN",
        requires = "plugins",
        conflicts_with = "load_last"
    )]
    pub load_after: Option<String>,
    /// Put the --plugin files at the end of the load order.
    #[arg(long = "load-last", requires = "plugins")]
    pub load_last: bool,
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
    /// For an archive no automated fetch can reach, and for the case MO2
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

/// List a Nexus mod's files, for choosing the one a modlist entry names.
///
/// Works for any Nexus account: listing files is not a download.
#[derive(Debug, Args)]
pub struct NexusFilesArgs {
    /// The mod id, as in nexusmods.com/<game>/mods/<id>.
    pub mod_id: u64,
    /// Nexus game domain.
    #[arg(long, default_value = "oblivion")]
    pub game: String,
    /// Also list old and archived files.
    #[arg(long)]
    pub all: bool,
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

/// Scaffold a merge-only modlist by reading an installed instance.
///
/// The merge engine needs to know which mod folder each plugin lives in and
/// what the load order is. Both are already on disk, so this reads them rather
/// than asking someone to transcribe them.
#[derive(Debug, Args)]
pub struct NewMergeArgs {
    /// Directory holding the installed mod folders, e.g. an MO2 instance's mods/.
    #[arg(long)]
    pub mods_dir: PathBuf,
    /// Name for the merge. Also the mod id, and the plugin name unless
    /// `--output-plugin` says otherwise.
    #[arg(long)]
    pub name: String,
    /// A plugin to merge, in the order they should be merged: the order decides
    /// clobber precedence, so later wins. Repeatable. Write `<mod>/<plugin>` to
    /// pick a side when two mods ship the same filename.
    #[arg(long = "plugin")]
    pub plugin: Vec<String>,
    /// Read plugin names from a text file, one per line, in merge order;
    /// blank lines and `#` comments are skipped. Combines with `--plugin`.
    #[arg(long = "plugins", alias = "plugins-from", value_name = "FILE")]
    pub plugins_from: Option<PathBuf>,
    /// Filename of the merged plugin. Defaults to `<name>.esp`.
    #[arg(long)]
    pub output_plugin: Option<String>,
    /// The MO2 profile whose load order to build against, by name. The usual
    /// way to choose when an instance has several.
    #[arg(long)]
    pub mo2_profile: Option<String>,
    /// The load order to build against, as a path to a `loadorder.txt` or
    /// `plugins.txt`. Overrides `--mo2-profile`. Found automatically when the
    /// instance has exactly one profile.
    #[arg(long)]
    pub load_order: Option<PathBuf>,
    /// Where to write the modlist. Printed to stdout if omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Overwrite `--output` if it already exists.
    #[arg(long)]
    pub force: bool,
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
    /// Use each input's declared `default` and skip all interactive prompts.
    #[arg(long)]
    pub headless: bool,
    /// Answer an input without being asked, as `key=value`. Repeatable.
    /// Takes precedence over the declared default.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub set: Vec<String>,
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
    /// The downloads folder: archives under their real names, with MO2
    /// `.meta` files. Defaults to `downloads_dir` in tools.toml, then (for an
    /// MO2 install) the instance's own `downloads/`.
    #[arg(long = "downloads", alias = "cache", value_name = "DIR")]
    pub downloads: Option<PathBuf>,
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
    /// The downloads folder: archives under their real names, with MO2
    /// `.meta` files. Defaults to `downloads_dir` in tools.toml, then (for an
    /// MO2 install) the instance's own `downloads/`.
    #[arg(long = "downloads", alias = "cache", value_name = "DIR")]
    pub downloads: Option<PathBuf>,
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
/// Report what a mod contributes to the virtual file system, and which of
/// those files another mod also provides.
///
/// Answers from the same code `install` uses, so what this prints is what
/// `conflicts_with` would act on. An installed mod is read from its folder; one
/// not yet installed is computed from its archives' entry lists.
#[derive(Debug, Args)]
pub struct ConflictsArgs {
    /// Personalized install plan.
    #[arg(long)]
    pub plan: PathBuf,
    /// Installation directory holding the mod folders.
    #[arg(long)]
    pub mods_dir: PathBuf,
    /// The mod whose files are in question. Without it, the whole list is
    /// swept: every installed mod that shares files with another, how many it
    /// wins and loses, and to which mods.
    #[arg(long = "mod")]
    pub mod_id: Option<String>,
    /// Mods to compare against. With none, every file the mod provides is
    /// listed instead.
    #[arg(long = "with", requires = "mod_id")]
    pub with: Vec<String>,
    /// In a sweep, report only these mods (they are still compared against
    /// the whole list).
    #[command(flatten)]
    pub filter: FilterArgs,
    /// Restrict the comparison to one subtree, e.g. `textures/`.
    #[arg(long)]
    pub under: Option<String>,
    /// The downloads folder: archives under their real names, with MO2
    /// `.meta` files. Defaults to `downloads_dir` in tools.toml, then (for an
    /// MO2 install) the instance's own `downloads/`.
    #[arg(long = "downloads", alias = "cache", value_name = "DIR")]
    pub downloads: Option<PathBuf>,
    #[command(flatten)]
    pub archive_sources: ArchiveSourceArgs,
}

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
    /// The downloads folder: archives under their real names, with MO2
    /// `.meta` files. Defaults to `downloads_dir` in tools.toml, then (for an
    /// MO2 install) the instance's own `downloads/`.
    #[arg(long = "downloads", alias = "cache", value_name = "DIR")]
    pub downloads: Option<PathBuf>,
    /// Installation output directory for plain staged installs.
    #[arg(long)]
    pub mods_dir: Option<PathBuf>,
    /// Optional ModOrganizer2 instance root to export into.
    #[arg(long)]
    pub mo2_instance_dir: Option<PathBuf>,
    /// Profile name to use when exporting to ModOrganizer2.
    #[arg(long, default_value = "Default")]
    pub profile_name: String,
    /// The game folder, for game-scoped install actions (e.g. Oblivion.ini edits).
    /// Defaults to `GAME_DIR`, then to the folder the MO2 instance was set up with.
    #[arg(long)]
    pub game_dir: Option<PathBuf>,
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
    /// Install even though Mod Organizer 2 has this instance open. MO2 writes
    /// its own copy of the profile back on exit, undoing the install's.
    #[arg(long)]
    pub ignore_running_mo2: bool,
    /// Path to tools.toml machine-local tool configuration.
    /// Defaults to ~/.config/mudcrab/tools.toml (or %APPDATA%\mudcrab\tools.toml on Windows).
    #[arg(long)]
    pub tools_config: Option<PathBuf>,
    #[command(flatten)]
    pub filter: FilterArgs,
    #[command(flatten)]
    pub archive_sources: ArchiveSourceArgs,
}

/// Compare what mudcrab installed against a reference MO2 instance -- the
/// "Oracle": the same list built another way (by hand in MO2, say) and known
/// to be right.
///
/// Verification is per section, not per list: a 700-mod list is only
/// reproducible if each section can be signed off as it is built, while the
/// decisions that produced it are still fresh.
#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Installation directory holding the mod folders we produced.
    #[arg(long)]
    pub mods_dir: PathBuf,
    /// The reference ("Oracle") instance's mods directory. Only ever read
    /// from.
    #[arg(long, value_name = "REFERENCE_MODS_DIR")]
    pub oracle: PathBuf,
    /// Personalized install plan, which supplies each mod's section and the
    /// archive it was meant to come from. Without it every directory present in
    /// either tree is compared, and `--section` has nothing to match against.
    #[arg(long)]
    pub plan: Option<PathBuf>,
    /// Report format.
    #[arg(long, value_enum, default_value_t = DiffFormat::Text)]
    pub format: DiffFormat,
    /// Print every difference, mod by mod. By default the terminal gets one
    /// line per section and the totals; the full report always goes to the log.
    #[arg(long)]
    pub details: bool,
    /// Exit non-zero when anything differs, to gate a script on a clean diff.
    /// Differences are the report, not an error, so this is off by default.
    #[arg(long)]
    pub fail_on_differences: bool,
    /// Publication date of the guide the list follows, `YYYY-MM-DD`.
    ///
    /// Normally read from the modlist's `[guide]` table and carried through the
    /// plan; give it here to compare two directories without one, or to override
    /// what the plan says. Without either, archive age is not reported.
    #[arg(long, value_name = "YYYY-MM-DD")]
    pub guide_date: Option<String>,
    /// Nexus file id corresponding to `--guide-date`.
    ///
    /// Nexus allocates file ids in ascending order, so an id dates a file whose
    /// name carries no timestamp. Optional; without it those archives report as
    /// unknown age rather than being guessed at.
    #[arg(long, value_name = "ID", requires = "guide_date")]
    pub guide_file_id: Option<u64>,
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

/// Render a modlist as a guide, for a Nexus page, a website, or your own tools.
///
/// An exporter is a built-in name or a folder of templates; `json` writes the
/// document every exporter reads, for rendering a list some other way.
#[derive(Debug, Args)]
pub struct ExportArgs {
    /// The source modlist TOML. Its docs (and `docs_file`) supply the prose.
    #[arg(required_unless_present = "eject")]
    pub input: Option<PathBuf>,
    /// Exporter: a built-in name, or a path to an exporter folder.
    #[arg(long = "to", value_name = "EXPORTER", default_value = "json")]
    pub exporter: String,
    /// Where to write. A file for single-file exporters (stdout when omitted),
    /// a directory for site exporters.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Describe one build: only the mods this plan selected, with its answers.
    #[arg(long)]
    pub plan: Option<PathBuf>,
    /// Where the list's archives are. With them, instructions name the files a
    /// pattern means (`OBSE/Plugins/x.dll` rather than `**/x.dll`) and skip
    /// steps MO2 does by itself. Without them, patterns are put into words.
    #[arg(long, alias = "cache")]
    pub downloads: Option<PathBuf>,
    /// Further folders to look for archives in. Repeatable.
    #[arg(long = "archive-search-path", value_name = "DIR")]
    pub archive_search_paths: Vec<PathBuf>,
    /// Set an exporter option, `key=value`; repeatable. Overrides the
    /// modlist's `[export.<exporter>]`.
    #[arg(long = "opt", value_name = "KEY=VALUE")]
    pub options: Vec<String>,
    /// List the exporter's options and their defaults, and export nothing.
    #[arg(long)]
    pub list_options: bool,
    /// Copy the built-in exporter named by --to into this new folder, to
    /// customise, and export nothing.
    #[arg(long, value_name = "DIR")]
    pub eject: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SetupToolsArgs {
    /// Path to the source modlist TOML to scan for required tools.
    pub input: PathBuf,
    /// Output path for the generated tools.toml template.
    /// Defaults to where every other command looks for it:
    /// ~/.config/mudcrab/tools.toml (%APPDATA%\mudcrab\tools.toml on Windows).
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Overwrite an existing tools.toml file.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Subcommand)]
pub enum BsaCommand {
    /// Print the path of every file in the archive, one per line.
    List(BsaListArgs),
    /// Write the archive's files below a directory.
    Extract(BsaExtractArgs),
}

#[derive(Debug, Args)]
pub struct BsaListArgs {
    /// The archive to read.
    pub archive: PathBuf,
    /// Only files whose path contains this text, ignoring case. Either
    /// slash direction matches.
    #[arg(long)]
    pub filter: Option<String>,
}

#[derive(Debug, Args)]
pub struct BsaExtractArgs {
    /// The archive to read.
    pub archive: PathBuf,
    /// Directory to extract into. Created if missing.
    pub destination: PathBuf,
    /// Only files whose path contains this text, ignoring case. Either
    /// slash direction matches.
    #[arg(long)]
    pub filter: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum DdsCommand {
    /// List DXT1 textures that use punch-through transparency, which some
    /// D3D9 stacks render as opaque black. These are what `dds_decompress`
    /// converts.
    Audit(DdsAuditArgs),
}

#[derive(Debug, Args)]
pub struct DdsAuditArgs {
    /// A mods directory, one mod's folder, or any directory of textures.
    pub root: PathBuf,
    /// Only textures whose path below `root` contains this text, ignoring
    /// case. Either slash direction matches.
    #[arg(long)]
    pub filter: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum DownloadsCommand {
    /// Convert a downloads folder from mudcrab's old derived names
    /// (`{mod}_{index}_{source}` + `.orig-name`) to real names with MO2 `.meta`
    /// files. Prints what it would do; changes nothing without --apply.
    Tidy(DownloadsTidyArgs),
    /// Record the size and MD5 of a modlist's hand-downloaded (`manual:`)
    /// archives in the list itself, so each is still found after its host
    /// renames it. Only archives found on disk and not yet recorded are
    /// written; comments and layout are kept.
    Record(DownloadsRecordArgs),
}

#[derive(Debug, Args)]
pub struct DownloadsRecordArgs {
    /// The modlist to edit in place.
    pub modlist: PathBuf,
    /// The downloads folder. Defaults to `downloads_dir` in tools.toml.
    #[arg(long = "downloads", alias = "cache", value_name = "DIR")]
    pub downloads: Option<PathBuf>,
    #[command(flatten)]
    pub archive_sources: ArchiveSourceArgs,
    /// Print what would be recorded, and change nothing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct DownloadsTidyArgs {
    /// The downloads folder to tidy.
    pub dir: PathBuf,
    /// A plan, so archives named only by their Nexus or URL source get the
    /// `.meta` that lookup needs to find them.
    #[arg(long)]
    pub plan: Option<PathBuf>,
    /// Carry out the plan rather than only printing it.
    #[arg(long)]
    pub apply: bool,
}
