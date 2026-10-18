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
    /// Use this tools.toml instead of the usual one
    /// (~/.config/mudcrab/tools.toml, or %APPDATA%\mudcrab\tools.toml on
    /// Windows). MUDCRAB_TOOLS_CONFIG does the same for every command run
    /// while it is set.
    #[arg(long, global = true, value_name = "FILE")]
    pub tools_config: Option<PathBuf>,
}

/// `--help` groups the commands by who reaches for them; clap lists them in
/// one alphabetical run. Every command is in exactly one group (a test checks).
pub const COMMAND_GROUPS: &[(&str, &[&str])] = &[
    (
        "Installing a modlist",
        &[
            "game-dir",
            "setup-tools",
            "compile",
            "query",
            "download",
            "check",
            "install",
            "assets",
            "steps",
        ],
    ),
    (
        "Writing a modlist",
        &[
            "inspect",
            "add",
            "nexus-files",
            "identify",
            "validate",
            "conflicts",
            "diff",
            "export",
        ],
    ),
    (
        "Editing a modlist",
        &[
            "move", "set", "remove", "plugins", "files", "action", "input",
        ],
    ),
    (
        "Utilities",
        &[
            "downloads",
            "merge",
            "new-merge",
            "unhide-merges",
            "bsa",
            "dds",
        ],
    ),
];

/// The CLI, with `--help` listing its commands in [`COMMAND_GROUPS`].
pub fn command() -> clap::Command {
    use clap::CommandFactory;
    let command = Cli::command();
    let width = command
        .get_subcommands()
        .map(|c| c.get_name().len())
        .max()
        .unwrap_or(0);
    let mut groups = String::new();
    for (title, names) in COMMAND_GROUPS {
        groups.push_str(&format!("{title}:\n"));
        for name in *names {
            let about = command
                .find_subcommand(name)
                .and_then(|c| c.get_about())
                .map(|a| a.to_string())
                .unwrap_or_default();
            groups.push_str(&format!("  {name:<width$}  {about}\n"));
        }
        groups.push('\n');
    }
    command.help_template(format!(
        "{{about-with-newline}}\n{{usage-heading}} {{usage}}\n\n{groups}Options:\n{{options}}{{after-help}}"
    ))
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add a mod to a source modlist, preserving its comments and formatting.
    Add(AddArgs),
    /// Move a mod to another section or position, comments and all.
    Move(MoveArgs),
    /// Set or clear fields of a mod, or of one of its archives.
    Set(SetArgs),
    /// Remove a mod, and its plugins from the load order.
    Remove(RemoveArgs),
    /// The load order: list it, or move a plugin within it.
    #[command(subcommand)]
    Plugins(PluginsCommand),
    /// What an archive installs, and its include and exclude filters.
    Files(FilesArgs),
    /// List, add or remove a mod's actions.
    #[command(subcommand)]
    Action(ActionCommand),
    /// List, declare, change or remove the list's questions.
    #[command(subcommand)]
    Input(InputCommand),
    /// Validate and compile a source modlist into a machine-friendly artifact.
    Compile(CompileArgs),
    /// Resolve interactive inputs into a personalized install plan.
    Query(QueryArgs),
    /// Download archives required by a compiled or personalized modlist.
    Download(DownloadArgs),
    /// Before installing: archives, layouts, actions, masters, load order and BSAs, all checked.
    Check(CheckArgs),
    /// Report an archive's layout, FOMOD options and plugins, for writing its
    /// modlist entry.
    Inspect(InspectArgs),
    /// Ask Nexus which mod and file an archive is, by its MD5.
    Identify(IdentifyArgs),
    /// List the files a Nexus mod publishes, with the descriptor for each.
    NexusFiles(NexusFilesArgs),
    /// Which installed mods share files, and with whom; or the files two mods
    /// both provide.
    Conflicts(ConflictsArgs),
    /// Install mods from downloaded archives.
    Install(InstallArgs),
    /// Compare an installed mods directory against a reference MO2 instance.
    Diff(DiffArgs),
    /// Validate a source modlist without producing compiled output.
    Validate(ValidateArgs),
    /// Render a modlist as a guide: JSON, or through an exporter's templates.
    Export(ExportArgs),
    /// Set up tools.toml: the downloads folder, Wine/Proton and xEdit, found and offered.
    SetupTools(SetupToolsArgs),
    /// Find Oblivion's game folder, and print it as MO2's setup asks for it.
    GameDir(GameDirArgs),
    /// After an install: the meshes and sounds the active plugins name that
    /// the game would not find.
    Assets(AssetsArgs),
    /// After an install: what the list leaves to do by hand, and what of it
    /// is done.
    Steps(StepsArgs),
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
/// `section` line changes. Validated before anything is written.
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
    /// Accepted for older scripts: compile never uses the network.
    #[arg(long, hide = true)]
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

/// Point a command at archives this machine already has: `download`, which
/// links what it finds there into the downloads folder, so every later step
/// looks only in that folder.
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
    /// For archives that cannot be downloaded directly (a free Nexus account,
    /// no API key, a hand download), open their pages in the browser, a batch
    /// at a time, and wait while you download them: each file is picked up as
    /// it arrives, in the browser's own downloads folder or in the downloads
    /// folder (from MO2's "Mod Manager Download", offered on Windows). The
    /// default in a terminal when a browser can be opened.
    #[arg(long, conflicts_with = "no_browser")]
    pub browser: bool,
    /// Do not open pages in the browser: report what could not be fetched,
    /// and fail.
    #[arg(long)]
    pub no_browser: bool,
    /// With the browser: how many pages to have open at once.
    #[arg(long, default_value_t = 10, value_name = "N")]
    pub batch: usize,
    /// With the browser: the folder your browser saves downloads to. Defaults
    /// to your Downloads folder.
    #[arg(long, value_name = "DIR")]
    pub watch: Option<PathBuf>,
    /// With the browser: what to do with an archive that arrives in the
    /// browser's folder, instead of asking: leave it there, move it into the
    /// downloads folder, or hard-link it there.
    #[arg(long, value_enum, value_name = "WHAT")]
    pub browser_downloads: Option<BrowserDownloads>,
}

pub use crate::config::by_hand::BrowserDownloads;

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
    /// Reinstall the mods that have an action of this name -- `qac`, say --
    /// even though their entries are unchanged. Repeatable; combines with
    /// --section and --only.
    #[arg(long = "redo-action", value_name = "ACTION")]
    pub redo_actions: Vec<String>,
    /// Print planned operations without applying them.
    #[arg(long)]
    pub dry_run: bool,
    /// Install even though Mod Organizer 2 has this instance open. MO2 writes
    /// its own copy of the profile back on exit, undoing the install's.
    #[arg(long)]
    pub ignore_running_mo2: bool,
    #[command(flatten)]
    pub filter: FilterArgs,
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

/// The meshes and sounds an installed instance's active plugins name, checked
/// against everything the game would see: its own `Data` and BSAs, each
/// enabled mod's files and BSAs, and `overwrite/`.
///
/// A report, not a gate: well-known mods name files that were never shipped.
/// What the game's own plugins already lack is left out. Textures are named by
/// meshes, not plugins, and are not checked.
#[derive(Debug, Args)]
pub struct AssetsArgs {
    /// The installed MO2 instance.
    #[arg(long)]
    pub mo2_instance_dir: PathBuf,
    /// The profile whose active plugins and enabled mods to read.
    #[arg(long, default_value = "Default")]
    pub profile_name: String,
    /// The game folder. Defaults to the one the instance was set up with.
    #[arg(long)]
    pub game_dir: Option<PathBuf>,
    /// List every missing file, not the first few per plugin.
    #[arg(long)]
    pub all: bool,
}

/// Where Oblivion is installed: Steam's copy (in any Steam library), then any
/// `Oblivion.exe` under `~/Games`.
///
/// Setting up an MO2 instance asks for this folder. Under Proton, MO2 sees the
/// Linux filesystem as drive `Z:`, so the path is also printed in that form,
/// ready to paste into the file window's "File name" box.
/// What a list leaves to do by hand after installing -- building a Bashed
/// Patch, settings to load in game -- with ✔ for each step known to be done
/// (its plugin exists in a mod or in MO2's overwrite) and ○ for the rest. A
/// step with nothing to tell it is done by is always ○. `install` prints the
/// ones not done at the end of every run; this shows them any time.
#[derive(Debug, Args)]
pub struct StepsArgs {
    /// Path to the personalized install plan.
    pub input: PathBuf,
    /// The MO2 instance the list is installed in.
    #[arg(long = "mo2-instance-dir", value_name = "DIR")]
    pub mo2_instance_dir: PathBuf,
    /// The game folder, for where Wrye Bash is. Defaults as for `install`.
    #[arg(long = "game-dir", value_name = "DIR")]
    pub game_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct GameDirArgs {
    /// Print only the first folder found, in the form MO2 takes (the `Z:`
    /// form on Linux), for a script or the clipboard.
    #[arg(long)]
    pub plain: bool,
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
    /// Take the best guess for each value instead of asking. Also what
    /// happens when input is not a terminal.
    #[arg(long)]
    pub defaults: bool,
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
    /// Record in a modlist what lets each archive be recognised wherever it
    /// turns up: the file name of a Nexus or URL archive that has none (so a
    /// browser download, with no MO2 `.meta`, is found), and the size and MD5
    /// of a hand-downloaded (`manual:`) one (so it is found after its host
    /// renames it). Only archives found on disk are written; comments and
    /// layout are kept.
    Record(DownloadsRecordArgs),
    /// Print where each archive a plan needs is: one tab-separated line per
    /// archive -- mod id, archive number, source, and its path, or MISSING.
    /// For scripts and for checking a downloads folder by eye. Only reads.
    Locate(DownloadsLocateArgs),
}

#[derive(Debug, Args)]
pub struct DownloadsLocateArgs {
    /// Path to a personalized install plan.
    pub plan: PathBuf,
    /// The downloads folder. Defaults to `downloads_dir` in tools.toml.
    #[arg(long = "downloads", alias = "cache", value_name = "DIR")]
    pub downloads: Option<PathBuf>,
    #[command(flatten)]
    pub archive_sources: ArchiveSourceArgs,
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

#[cfg(test)]
mod help_tests {
    #[test]
    fn every_command_is_in_exactly_one_help_group() {
        use clap::CommandFactory;
        let listed: Vec<&str> = super::COMMAND_GROUPS
            .iter()
            .flat_map(|(_, names)| names.iter().copied())
            .collect();
        for command in super::Cli::command().get_subcommands() {
            let name = command.get_name();
            assert_eq!(
                listed.iter().filter(|n| **n == name).count(),
                1,
                "{name} must be in exactly one group in COMMAND_GROUPS"
            );
        }
        assert_eq!(
            listed.len(),
            super::Cli::command().get_subcommands().count()
        );
    }
}

/// Set or clear fields of a mod, or with --archive of one of its archives:
/// `layout=bain`, `if=some_input`, `data_folder="Data Files"`,
/// `merge.output=Merged.esp`. A value is read as TOML when it is one
/// (`true`, `3`, `["a", "b"]`) and as text otherwise; quote it to force text
/// (`version='"1.0"'`). Comments, and a replaced value's own comment, are
/// kept. The list is validated before anything is written.
#[derive(Debug, Args)]
pub struct SetArgs {
    /// The modlist to edit in place.
    pub input: PathBuf,
    /// The mod to change.
    #[arg(long = "mod", value_name = "MOD_ID")]
    pub mod_id: String,
    /// Change this archive of the mod (counting from 1) instead of the mod.
    #[arg(long, value_name = "N")]
    pub archive: Option<usize>,
    /// `key=value` to set.
    #[arg(value_name = "KEY=VALUE")]
    pub sets: Vec<String>,
    /// A key to remove; repeat for more.
    #[arg(long = "unset", value_name = "KEY")]
    pub unsets: Vec<String>,
    /// Print the change as a diff, and write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// Remove a mod's block -- its comments, archives and actions -- and its
/// plugins from `plugins` and `inactive-plugins`. Refused, naming what is in
/// the way, if anything else still refers to it.
#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// The modlist to edit in place.
    pub input: PathBuf,
    /// The mod to remove.
    #[arg(long = "mod", value_name = "MOD_ID")]
    pub mod_id: String,
    /// Print the change as a diff, and write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum PluginsCommand {
    /// Print the load order, one plugin a line, tab-separated: the plugin,
    /// the mods that declare it in their `plugins` (or make it, for a merge;
    /// `-` when none does), and `inactive` for one the list leaves unticked.
    List(PluginsListArgs),
    /// Move a plugin within the load order. Comment lines in the array stay
    /// where they are: a heading above the plugin stays with what follows it.
    Move(PluginsMoveArgs),
}

#[derive(Debug, Args)]
pub struct PluginsListArgs {
    /// The modlist.
    pub input: PathBuf,
}

#[derive(Debug, Args)]
#[command(group(clap::ArgGroup::new("to").required(true).args(["after", "before", "first", "last"])))]
pub struct PluginsMoveArgs {
    /// The modlist to edit in place.
    pub input: PathBuf,
    /// The plugin to move.
    pub plugin: String,
    /// Put it directly after this plugin.
    #[arg(long, value_name = "PLUGIN")]
    pub after: Option<String>,
    /// Put it directly before this plugin (below any heading comment above it).
    #[arg(long, value_name = "PLUGIN")]
    pub before: Option<String>,
    /// Put it first.
    #[arg(long)]
    pub first: bool,
    /// Put it last.
    #[arg(long)]
    pub last: bool,
    /// Print the change as a diff, and write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// What one of a mod's archives installs: the files `install` would take
/// from it, before the mod's actions, as `check` plans them. With filters to
/// add or drop, the list is edited and what that changes is shown.
#[derive(Debug, Args)]
pub struct FilesArgs {
    /// The modlist.
    pub input: PathBuf,
    /// The mod.
    #[arg(long = "mod", value_name = "MOD_ID")]
    pub mod_id: String,
    /// Which of the mod's archives, counting from 1.
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub archive: usize,
    /// Add an `include` glob: only matching files are taken.
    #[arg(long = "include", value_name = "GLOB")]
    pub include: Vec<String>,
    /// Add an `exclude` glob: matching files are left out.
    #[arg(long = "exclude", value_name = "GLOB")]
    pub exclude: Vec<String>,
    /// Take an `include` glob out.
    #[arg(long = "drop-include", value_name = "GLOB")]
    pub drop_include: Vec<String>,
    /// Take an `exclude` glob out.
    #[arg(long = "drop-exclude", value_name = "GLOB")]
    pub drop_exclude: Vec<String>,
    /// Choose a BAIN package, as Wrye Bash's installer tab ticks one: a
    /// top-level folder of a BAIN archive, added to `bain_subpackages`.
    #[arg(long = "package", value_name = "NAME")]
    pub package: Vec<String>,
    /// Stop choosing a BAIN package.
    #[arg(long = "drop-package", value_name = "NAME")]
    pub drop_package: Vec<String>,
    /// The downloads folder. Defaults to `downloads_dir` in tools.toml.
    #[arg(long = "downloads", value_name = "DIR")]
    pub downloads: Option<PathBuf>,
    /// Answer the list's questions for this (`key=value`), where they decide
    /// the mod's archives; the rest take their defaults.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub set: Vec<String>,
    /// Print the change as a diff, and write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum ActionCommand {
    /// Print a mod's actions, numbered.
    List(ActionListArgs),
    /// Add an action to the end of a mod's actions: its name, then its fields
    /// as `key=value` (`paths='["a/b"]'`, `conflicts_with='["Other Mod"]'`).
    /// See the modlist format for each action's fields.
    Add(ActionAddArgs),
    /// Remove a mod's action by its number from `action list`.
    Remove(ActionRemoveArgs),
}

#[derive(Debug, Args)]
pub struct ActionListArgs {
    /// The modlist.
    pub input: PathBuf,
    /// The mod.
    #[arg(long = "mod", value_name = "MOD_ID")]
    pub mod_id: String,
}

#[derive(Debug, Args)]
pub struct ActionAddArgs {
    /// The modlist to edit in place.
    pub input: PathBuf,
    /// The mod.
    #[arg(long = "mod", value_name = "MOD_ID")]
    pub mod_id: String,
    /// The action: `file_hide`, `file_move`, `ini_set`, ...
    pub action: String,
    /// The action's fields, `key=value`.
    #[arg(value_name = "KEY=VALUE")]
    pub fields: Vec<String>,
    /// Only when this condition holds.
    #[arg(long = "if", value_name = "CONDITION")]
    pub condition: Option<String>,
    /// Print the change as a diff, and write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ActionRemoveArgs {
    /// The modlist to edit in place.
    pub input: PathBuf,
    /// The mod.
    #[arg(long = "mod", value_name = "MOD_ID")]
    pub mod_id: String,
    /// The action's number, from `action list`.
    pub number: usize,
    /// Print the change as a diff, and write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum InputCommand {
    /// Print the list's questions: name, type, default, and the question.
    List(InputListArgs),
    /// Declare a question.
    Add(InputAddArgs),
    /// Change a question's fields (`query=...`, `default=...`,
    /// `choices='["a", "b"]'`).
    Set(InputSetArgs),
    /// Remove a question. Refused, by validation, while a condition uses it.
    Remove(InputRemoveArgs),
}

#[derive(Debug, Args)]
pub struct InputListArgs {
    /// The modlist.
    pub input: PathBuf,
}

#[derive(Debug, Args)]
pub struct InputAddArgs {
    /// The modlist to edit in place.
    pub input: PathBuf,
    /// The question's name, as conditions use it.
    pub name: String,
    /// What kind of answer it takes.
    #[arg(long = "type", value_name = "TYPE", value_parser = ["bool", "choice", "text"])]
    pub input_type: String,
    /// The question as the player sees it.
    #[arg(long)]
    pub query: String,
    /// One of the answers to a `choice`; repeat for each.
    #[arg(long = "choice", value_name = "CHOICE")]
    pub choices: Vec<String>,
    /// The answer when nobody is asked (`query --headless`).
    #[arg(long)]
    pub default: Option<String>,
    /// Print the change as a diff, and write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct InputSetArgs {
    /// The modlist to edit in place.
    pub input: PathBuf,
    /// The question.
    pub name: String,
    /// `key=value` to set.
    #[arg(value_name = "KEY=VALUE")]
    pub sets: Vec<String>,
    /// A key to remove; repeat for more.
    #[arg(long = "unset", value_name = "KEY")]
    pub unsets: Vec<String>,
    /// Print the change as a diff, and write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct InputRemoveArgs {
    /// The modlist to edit in place.
    pub input: PathBuf,
    /// The question.
    pub name: String,
    /// Print the change as a diff, and write nothing.
    #[arg(long)]
    pub dry_run: bool,
}
