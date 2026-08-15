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
    pub modlist: IndexMap<String, ModNode>,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default, rename = "post-install-actions")]
    pub post_install_actions: Vec<PostInstallAction>,
}

impl SourceModlist {
    pub fn flatten_mods(&self) -> anyhow::Result<IndexMap<String, ModSpec>> {
        let mut out = IndexMap::new();
        for (key, node) in &self.modlist {
            flatten_mod_node(node, key, &mut out)?;
        }
        Ok(out)
    }

    pub fn mo2_modlist_entries(&self) -> anyhow::Result<Vec<Mo2ModlistEntry>> {
        let mut out = Vec::new();
        let mut seen_mod_ids = HashSet::new();
        let mut section_path = Vec::new();

        for (key, node) in &self.modlist {
            collect_mo2_entries(node, key, &mut section_path, &mut seen_mod_ids, &mut out)?;
        }

        Ok(out)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ModNode {
    Mod(ModSpec),
    Section(IndexMap<String, ModNode>),
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
}

impl ModAction {
    /// Name as written in TOML, for logs and error messages.
    pub fn name(&self) -> &'static str {
        match self {
            ModAction::IniSet(_) => "ini_set",
            ModAction::Qac(_) => "qac",
        }
    }
}

/// Set a key in an INI file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IniSetAction {
    /// Path to the INI. Relative to the mod's staged data folder for
    /// `scope = "mod"`, or to the game/profile INI location for `scope = "game"`.
    pub file: String,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModSpec {
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(rename = "if")]
    pub condition: Option<String>,
    #[serde(default)]
    pub archives: Vec<ArchiveSpec>,
    /// Mod type. Currently supported: `"build-from-files"`.
    #[serde(rename = "type")]
    pub mod_type: Option<String>,
    #[serde(default)]
    pub plugins: Vec<String>,
    /// Source file globs for mods of type `"build-from-files"`.
    /// Paths may use `%GAME_DIR%`, which is expanded at install time.
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub actions: Vec<ModAction>,
}

fn flatten_mod_node(
    node: &ModNode,
    key: &str,
    out: &mut IndexMap<String, ModSpec>,
) -> anyhow::Result<()> {
    match node {
        ModNode::Mod(spec) => {
            if out.contains_key(key) {
                anyhow::bail!("duplicate mod id {key}");
            }
            out.insert(key.to_string(), spec.clone());
            Ok(())
        }
        ModNode::Section(children) => {
            for (child_key, child_node) in children {
                flatten_mod_node(child_node, child_key, out)?;
            }
            Ok(())
        }
    }
}

fn collect_mo2_entries(
    node: &ModNode,
    key: &str,
    section_path: &mut Vec<String>,
    seen_mod_ids: &mut HashSet<String>,
    out: &mut Vec<Mo2ModlistEntry>,
) -> anyhow::Result<()> {
    match node {
        ModNode::Mod(_) => {
            if !seen_mod_ids.insert(key.to_string()) {
                anyhow::bail!("duplicate mod id {key}");
            }
            out.push(Mo2ModlistEntry::Mod { id: key.to_string() });
            Ok(())
        }
        ModNode::Section(children) => {
            section_path.push(key.to_string());
            out.push(Mo2ModlistEntry::Section {
                name: section_path.join(" - "),
            });
            for (child_key, child_node) in children {
                collect_mo2_entries(child_node, child_key, section_path, seen_mod_ids, out)?;
            }
            section_path.pop();
            Ok(())
        }
    }
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

#[derive(Debug, Clone, Deserialize)]
pub struct ArchiveSpec {
    #[serde(default)]
    pub path: Option<String>,
    pub download_handler: Option<String>,
    pub layout: Option<String>,
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
pub struct FomodSelection {
    pub step: String,
    pub group: String,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_type: Option<String>,
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
    pub download_handler: Option<String>,
    pub layout: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizedMod {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_type: Option<String>,
    pub archives: Vec<CompiledArchive>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub actions: Vec<ModAction>,
}
