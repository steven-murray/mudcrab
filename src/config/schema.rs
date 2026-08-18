use indexmap::IndexMap;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Deserialize)]
pub struct SourceModlist {
    pub name: String,
    #[serde(default)]
    pub inputs: HashMap<String, InputSpec>,
    #[serde(default)]
    pub ini: toml::Table,
    #[serde(default)]
    pub mods: Vec<ModEntry>,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default, rename = "post-install-actions")]
    pub post_install_actions: Vec<PostInstallAction>,
}

impl SourceModlist {
    /// Mods keyed by id, in declaration order. Errors on duplicate ids.
    pub fn flatten_mods(&self) -> anyhow::Result<IndexMap<String, ModEntry>> {
        let mut out = IndexMap::new();
        for entry in &self.mods {
            if out.contains_key(&entry.id) {
                anyhow::bail!("duplicate mod id {}", entry.id);
            }
            out.insert(entry.id.clone(), entry.clone());
        }
        Ok(out)
    }

    /// The MO2 modlist as an ordered mix of separators and mods.
    ///
    /// A separator is emitted for each level of a mod's section path at the
    /// point that level first differs from the previous mod's, so
    /// `section = ["OBSE PLUGINS", "Core"]` yields separators `OBSE PLUGINS`
    /// and `OBSE PLUGINS - Core` before the mod itself.
    pub fn mo2_modlist_entries(&self) -> anyhow::Result<Vec<Mo2ModlistEntry>> {
        let mut out = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut previous: Vec<String> = Vec::new();

        for entry in &self.mods {
            for depth in 0..entry.section.len() {
                if previous.len() > depth && previous[..=depth] == entry.section[..=depth] {
                    continue;
                }
                out.push(Mo2ModlistEntry::Section {
                    name: entry.section[..=depth].join(" - "),
                });
            }
            previous = entry.section.clone();

            if !seen_ids.insert(entry.id.clone()) {
                anyhow::bail!("duplicate mod id {}", entry.id);
            }
            out.push(Mo2ModlistEntry::Mod {
                id: entry.id.clone(),
            });
        }

        Ok(out)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputSpec {
    #[serde(rename = "type")]
    pub input_type: InputType,
    pub query: String,
    #[serde(default)]
    pub choices: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    Bool,
    Choice,
    Text,
}

/// A per-mod action to execute during installation.
///
/// Each entry is dispatched by `action` name. Additional parameters
/// (such as `file`, `key`, `value`) live in the same table alongside `action`.
/// Glob patterns in action parameters (e.g. `plugins`) are resolved relative
/// to the mod's own staged data folder, never against the raw game directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ModAction {
    IniSet(IniSetAction),
    Qac(QacAction),
    PackBsa(PackBsaAction),
    CreateDummyPlugin(CreateDummyPluginAction),
    FilePrune(FilePruneAction),
    FileHide(FileHideAction),
    FileMove(FileMoveAction),
    ExtractBsa(ExtractBsaAction),
}

impl ModAction {
    /// Whether this action writes somewhere other than the mod's staged folder.
    ///
    /// Actions are normally applied once and latched in the manifest, on the
    /// grounds that a mod whose definition has not changed does not need its
    /// staged folder rebuilt. That reasoning does not extend to an action whose
    /// target is *outside* the folder: MO2 recreates a profile's `Oblivion.ini`
    /// whenever it likes, so a game-scoped `ini_set` can be undone without
    /// anything about the mod changing. Those run on every install.
    pub fn writes_outside_mod_folder(&self) -> bool {
        match self {
            ModAction::IniSet(spec) => spec.scope == IniScope::Game,
            // The rest write only into the staged folder.
            ModAction::Qac(_)
            | ModAction::PackBsa(_)
            | ModAction::CreateDummyPlugin(_)
            | ModAction::FilePrune(_)
            | ModAction::FileHide(_)
            | ModAction::FileMove(_)
            | ModAction::ExtractBsa(_) => false,
        }
    }

    /// Name as written in TOML, for logs and error messages.
    pub fn name(&self) -> &'static str {
        match self {
            ModAction::IniSet(_) => "ini_set",
            ModAction::Qac(_) => "qac",
            ModAction::PackBsa(_) => "pack_bsa",
            ModAction::CreateDummyPlugin(_) => "create_dummy_plugin",
            ModAction::FilePrune(_) => "file_prune",
            ModAction::FileHide(_) => "file_hide",
            ModAction::FileMove(_) => "file_move",
            ModAction::ExtractBsa(_) => "extract_bsa",
        }
    }
}

/// Pack the mod's staged files into a BSA.
///
/// Runs in declaration order alongside the other actions, so a `file_prune`
/// listed after it deletes the loose copies it just packed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackBsaAction {
    /// Archive name, relative to the mod's staged data folder. Oblivion loads
    /// `Foo.bsa` when a `Foo.esp` is in the load order, so this normally
    /// shares its stem with a `create_dummy_plugin` output.
    pub output: String,
    /// Glob patterns selecting what to pack, relative to the staged folder.
    /// Empty means everything.
    #[serde(default)]
    pub include: Vec<String>,
    /// Glob patterns excluded from packing, applied after `include`.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Delete the loose files that were packed, once the archive is written.
    ///
    /// Equivalent to a `file_prune` listing exactly what went in, but taken
    /// from the pack's own file list rather than from a glob written by hand --
    /// so it cannot name a folder the archive does not have, or miss one it
    /// does. Nothing outside the archive is touched.
    #[serde(default)]
    pub prune_packed: bool,
}

/// Write an empty plugin whose name matches a BSA.
///
/// Oblivion only loads an archive when a plugin of the same stem is active, so
/// a mod shipped as a bare BSA needs one of these next to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateDummyPluginAction {
    /// Plugin name, relative to the mod's staged data folder.
    pub output: String,
}

/// Delete staged files matching globs.
///
/// This is the post-install ordered deletion that `pack_bsa`'s `exclude`
/// cannot express, because it has to run *after* the archive is written.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilePruneAction {
    /// Glob patterns resolved relative to the mod's staged data folder.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Mod ids whose files this mod should yield to.
    ///
    /// The guide's "Winning File conflicts -> Overwritten mods" and "Losing
    /// file conflicts -> Providing Mod" rows, said once instead of as a list
    /// of paths read off somebody's finished install. See
    /// `notes/conflict-resolution-design.md` for why the *direction* has to be
    /// declared rather than derived from mod priority.
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    /// Restrict a `conflicts_with` selection to one subtree.
    #[serde(default)]
    pub under: Option<String>,
}

/// Rename staged files to `<name>.mohidden`, MO2's way of dropping something
/// out of the virtual file system without deleting it.
///
/// The guide's usual phrasing is "hide or delete"; this is the hide half, and
/// the one a hand-built MO2 instance ends up containing. Naming a directory
/// hides everything below it in a single rename, exactly as MO2 does.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileHideAction {
    /// Paths relative to the mod's staged data folder, matched
    /// case-insensitively a segment at a time. Literal paths, not globs: each
    /// one is a file or folder the guide named, and one that is not there is an
    /// error rather than a silent skip.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Mod ids whose files this mod should yield to. See
    /// [`FilePruneAction::conflicts_with`].
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    /// Restrict a `conflicts_with` selection to one subtree.
    #[serde(default)]
    pub under: Option<String>,
}

/// Move a staged file or folder somewhere else inside the same mod.
///
/// The guide says "move X to the optional folder" for a plugin it does not want
/// in the load order but does not want gone either.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileMoveAction {
    /// Source path, relative to the staged folder, matched case-insensitively.
    pub from: String,
    /// Destination path, relative to the staged folder. Parent directories are
    /// created.
    pub to: String,
}

/// Unpack a BSA the mod ships, leaving its contents loose in the staged folder.
///
/// Paired with `pack_bsa` this replaces the guide's BAE-then-BSArch round trip,
/// which exists only so that something can be merged into an existing archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractBsaAction {
    /// Archive to unpack, relative to the staged folder.
    pub archive: String,
    /// Keep the archive afterwards. Off by default: leaving it means a later
    /// `pack_bsa` folds the old archive into the new one, and the loose files
    /// are shadowed by the archive they came from.
    #[serde(default)]
    pub keep_archive: bool,
}

/// Set a key in an INI file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IniSetAction {
    /// Path to the INI. Relative to the mod's staged data folder for
    /// `scope = "mod"`, or to the game/profile INI location for `scope = "game"`.
    pub file: String,
    /// `[Section]` the key belongs to, without the brackets.
    ///
    /// Omit it and the key is matched anywhere in the file, which is right for
    /// the many keys that occur exactly once. When a key occurs in more than
    /// one section -- Part 14's `Fog.ini` has `Amount` under both `[World]` and
    /// `[Interior]` -- omitting it is an error rather than a silent write to
    /// both, because there is no reading of "set Amount" that means both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    pub key: String,
    pub value: IniValue,
    #[serde(default)]
    pub format: IniSetFormat,
    #[serde(default)]
    pub scope: IniScope,
}

/// Quick Auto Clean: run xEdit's QAC over the named plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QacAction {
    /// Glob patterns resolved relative to the mod's staged data folder.
    /// Required: a qac action with nothing to clean is always a mistake.
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IniScope {
    /// An INI shipped by the mod itself.
    #[default]
    Mod,
    /// A game-scoped INI (Oblivion.ini). Never edited in place in the game
    /// directory -- resolved to the MO2 profile-local copy.
    Game,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IniSetFormat {
    /// `key = value`
    #[default]
    Standard,
    /// `set key to value` -- used by Oblivion script-style INIs.
    SetTo,
}

/// An INI value.
///
/// Accepts any TOML scalar and normalises to its string form, so `value = 0`,
/// `value = "0"` and `value = false` are all accepted. Booleans become 1/0,
/// matching Oblivion's INI conventions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct IniValue(pub String);

impl std::fmt::Display for IniValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for IniValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let value = toml::Value::deserialize(deserializer)?;
        Ok(IniValue(match value {
            toml::Value::String(text) => text,
            toml::Value::Integer(number) => number.to_string(),
            toml::Value::Float(number) => number.to_string(),
            toml::Value::Boolean(flag) => if flag { "1" } else { "0" }.to_string(),
            other => {
                return Err(D::Error::custom(format!(
                    "ini value must be a string, number or boolean, got {}",
                    other.type_str()
                )))
            }
        }))
    }
}

/// One mod in the flat `[[mods]]` list.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModEntry {
    /// Unique id; also the mod's directory name.
    pub id: String,
    /// The mod's folder name in a reference instance, used only by `diff` when
    /// our id is deliberately different; it does NOT affect installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_name: Option<String>,
    /// Section path, outermost first. Each level becomes an MO2 separator.
    /// Always a list, so there is a single shape to read, write and test.
    #[serde(default)]
    pub section: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(rename = "if")]
    pub condition: Option<String>,
    #[serde(default)]
    pub archives: Vec<ArchiveSpec>,
    #[serde(rename = "type")]
    pub mod_type: Option<ModType>,
    /// Merge definition. Required when `type = "merge"`, rejected otherwise.
    pub merge: Option<MergeSpec>,
    #[serde(default)]
    pub plugins: Vec<String>,
    /// Source file globs for mods of type `"build-from-files"`.
    /// Paths may use `%GAME_DIR%`, which is expanded at install time.
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub actions: Vec<ModAction>,
}

/// A mod that is produced rather than extracted.
///
/// Absent means the ordinary case: a mod installed from its archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModType {
    /// Assembled from files already on disk, listed in `files`.
    BuildFromFiles,
    /// Produced by merging plugins from other mods; see `[mods.merge]`.
    Merge,
}

/// How overlapping records are resolved when two sources define the same one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeMethod {
    /// Last writer wins, by source order. zMerge's "Clobber".
    #[default]
    Clobber,
}

/// One plugin to merge, named by the mod that owns it.
///
/// Naming the mod id rather than a data-folder path is the main improvement
/// over zMerge's `merges.json`: the path is derived at install time, so it
/// survives mod renames and does not encode a Windows drive letter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MergeSource {
    #[serde(rename = "mod")]
    pub mod_id: String,
    pub plugin: String,
}

/// A merge definition, attached to the mod that will contain the output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MergeSpec {
    /// Filename of the merged plugin, written into this mod's folder.
    pub output: String,
    #[serde(default)]
    pub method: MergeMethod,
    /// Rename each source plugin to `<name>.mohidden` so MO2 drops it from the
    /// virtual filesystem while leaving its mod enabled to serve assets.
    #[serde(default = "default_true")]
    pub hide_sources: bool,
    /// Ordered: this defines both clobber precedence and FormID allocation
    /// order, so reordering changes the output. There is deliberately no
    /// `load_order` field -- it is derived from the modlist's `plugins`.
    pub sources: Vec<MergeSource>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Mo2ModlistEntry {
    Mod { id: String },
    Section { name: String },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PostInstallAction {
    LootSort,
}

impl<'de> Deserialize<'de> for PostInstallAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "loot-sort" => Ok(Self::LootSort),
            other => Err(de::Error::custom(format!(
                "unknown post-install action '{other}'. Supported values: loot-sort"
            ))),
        }
    }
}

/// Declared archive layout. `None` means "detect automatically".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchiveLayout {
    /// Archive root is the mod's data folder; copied as-is.
    Simple,
    /// FOMOD installer: driven by fomod/ModuleConfig.xml + fomod_selections.
    Fomod,
    /// BAIN package: selected top-level subpackages merged into the mod root.
    Bain,
    /// The mod's data folder is somewhere other than the archive root; the
    /// location is given by `data_folder`, which is then required.
    CustomDataFolder,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveSpec {
    #[serde(default)]
    pub path: Option<String>,
    /// The archive's own filename, as the host serves it.
    ///
    /// `path` identifies *where* to get the archive, not what it is called: a
    /// `nexus:oblivion/<mod_id>/<file_id>` descriptor names neither. Stating the
    /// filename here is what lets an install find a copy already sitting in an
    /// `--archive-search-path` instead of downloading it again, and it is also
    /// the name the archive is exported under into MO2's `downloads/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    pub download_handler: Option<String>,
    pub layout: Option<ArchiveLayout>,
    pub data_folder: Option<String>,
    pub target_subdir: Option<String>,
    #[serde(default)]
    pub bain_subpackages: Vec<String>,
    #[serde(default)]
    pub fomod_selections: Vec<FomodSelection>,
    #[serde(default)]
    pub build: Vec<BuildLayer>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Glob patterns for files to extract directly to the game-root output directory.
    /// Matched files are automatically excluded from normal mod installation.
    #[serde(default)]
    pub game_root_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FomodSelection {
    pub step: String,
    pub group: String,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BuildLayer {
    pub path: String,
    #[serde(default)]
    pub download_handler: Option<String>,
    /// Subdirectory inside the merged staging dir to overlay this layer's extracted files into.
    /// Defaults to the staging root when absent.
    #[serde(default)]
    pub dest_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledModlist {
    pub schema_version: u32,
    pub name: String,
    pub input_count: usize,
    pub mod_count: usize,
    pub plugin_count: usize,
    pub inputs: HashMap<String, InputSpec>,
    /// Install-wide actions, currently only those desugared from the top-level
    /// `[ini]` table. Same type as per-mod actions so there is one dispatcher.
    #[serde(default)]
    pub actions: Vec<ModAction>,
    pub plugins: Vec<String>,
    #[serde(default)]
    pub post_install_actions: Vec<PostInstallAction>,
    #[serde(default)]
    pub mo2_modlist_entries: Vec<Mo2ModlistEntry>,
    pub mods: Vec<CompiledMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledMod {
    pub id: String,
    /// The mod's folder name in a reference instance, used only by `diff` when
    /// our id is deliberately different; it does NOT affect installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_name: Option<String>,
    /// Section path, carried through from the source entry.
    ///
    /// The flattened separator names in `mo2_modlist_entries` are a rendering
    /// of this, not a substitute for it: they cannot be matched back to a mod,
    /// so anything that wants to act on one section of the list needs the path
    /// itself. Defaulted so compiled artifacts written before this field
    /// existed still parse.
    #[serde(default)]
    pub section: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_type: Option<ModType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<MergeSpec>,
    pub dependencies: Vec<String>,
    pub archives: Vec<CompiledArchive>,
    pub condition: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub actions: Vec<ModAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledArchive {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The archive's own filename; see `ArchiveSpec::file_name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    pub download_handler: Option<String>,
    pub layout: Option<ArchiveLayout>,
    pub data_folder: Option<String>,
    pub target_subdir: Option<String>,
    #[serde(default)]
    pub bain_subpackages: Vec<String>,
    #[serde(default)]
    pub fomod_selections: Vec<FomodSelection>,
    #[serde(default)]
    pub build: Vec<BuildLayer>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    #[serde(default)]
    pub game_root_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputValue {
    Bool(bool),
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizedPlan {
    pub schema_version: u32,
    pub name: String,
    pub responses: HashMap<String, InputValue>,
    pub mod_order: Vec<String>,
    pub selected_mods: Vec<String>,
    #[serde(default)]
    pub actions: Vec<ModAction>,
    #[serde(default)]
    pub post_install_actions: Vec<PostInstallAction>,
    #[serde(default)]
    pub mo2_modlist_entries: Vec<Mo2ModlistEntry>,
    pub mods: Vec<PersonalizedMod>,
    pub plugins: Vec<String>,
}

impl PersonalizedPlan {
    /// Selected merges, in modlist order.
    ///
    /// Derived rather than stored: a second list would be a second source of
    /// truth that could disagree with `mods` after conditions are resolved.
    pub fn merges(&self) -> impl Iterator<Item = (&PersonalizedMod, &MergeSpec)> {
        self.mods
            .iter()
            .filter_map(|entry| entry.merge.as_ref().map(|merge| (entry, merge)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizedMod {
    pub id: String,
    /// The mod's folder name in a reference instance, used only by `diff` when
    /// our id is deliberately different; it does NOT affect installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_name: Option<String>,
    /// Section path, carried through from the compiled entry so `download`,
    /// `check` and `install` can be scoped to part of the list. Defaulted so
    /// plans written before this field existed still parse.
    #[serde(default)]
    pub section: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_type: Option<ModType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<MergeSpec>,
    pub archives: Vec<CompiledArchive>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub actions: Vec<ModAction>,
}
