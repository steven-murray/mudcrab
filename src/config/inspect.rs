//! Read an archive and report what a modlist entry for it would have to say.
//!
//! Configuring a mod otherwise means downloading its archive, extracting it by
//! hand, opening `fomod/ModuleConfig.xml` in an editor and transcribing step,
//! group and option names into TOML -- where a typo only surfaces at install
//! time, halfway through a 700-mod run. Everything needed to print that
//! already exists in the installer; this exposes it.
//!
//! Nothing here extracts an archive wholesale. The layout guess, the BAIN
//! subpackages, the plugin list and the file listing all come from
//! `archive::list_archive_paths`, which reads entry headers only. The single
//! exception is a FOMOD's `ModuleConfig.xml`, which has to be read to be
//! parsed, and is extracted on its own with an include filter naming just it.

use crate::archive::{self, ArchiveFilters, extract_with_builtins};
use crate::config::install::is_plugin_file;
use crate::config::install::layout::auto::is_expected_game_content_dir_name;
use crate::config::install::layout::fomod::{
    build_fomod_selection_map, child_elements, collect_fomod_entries, find_child_element,
    fomod_dependencies_match, fomod_option_type, read_xml_text, select_fomod_options,
    FomodInstallEntry, FomodOptionType,
};
use crate::util::fs::{eq_ci, staging_dir_for};
use roxmltree::Document;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// What `inspect` found, in the order a modlist entry needs it.
#[derive(Debug, Serialize)]
pub struct InspectReport {
    pub archive: String,
    pub file_name: String,
    pub file_count: usize,
    pub layout: LayoutGuess,
    /// The `[[mods.archives]]` block this archive wants, ready to paste.
    pub toml_snippet: String,
    pub top_level_dirs: Vec<String>,
    pub top_level_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fomod: Option<FomodReport>,
    /// Plugins in the archive. They have to be placed in the load order by
    /// hand, so they are always reported, whatever the layout is.
    pub plugins: Vec<String>,
    /// Every path in the archive; only populated with `--files`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
}

/// The layout the archive looks like, and what that implies for the TOML.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LayoutGuess {
    /// Has `fomod/ModuleConfig.xml` (or the older `fomod/script.xml`).
    Fomod { config_path: String },
    /// Numbered top-level subpackage directories, e.g. `00 Core`, `01 Option`.
    Bain {
        subpackages: Vec<String>,
        /// Top-level directories that are *not* numbered subpackages, listed
        /// so a `docs/` folder is not mistaken for one that was missed.
        other_top_level: Vec<String>,
    },
    /// Installs as-is: the auto layout already finds the data folder, so the
    /// entry needs no `layout`, `data_folder` or `target_subdir` at all.
    Simple { detail: String },
    /// The data folder is nested somewhere the auto layout will not find, so
    /// it has to be named.
    CustomDataFolder { data_folder: String },
    /// Nothing recognisable. `--files` is the next step.
    Unknown { detail: String },
}

impl LayoutGuess {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fomod { .. } => "FOMOD",
            Self::Bain { .. } => "BAIN",
            Self::Simple { .. } => "plain data folder",
            Self::CustomDataFolder { .. } => "nested data folder",
            Self::Unknown { .. } => "unrecognised",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FomodReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_name: Option<String>,
    pub config_path: String,
    /// Files installed before any choice is made; nothing to configure, but
    /// an empty-looking installer with 200 of these is worth knowing about.
    pub required_install_entries: usize,
    pub steps: Vec<FomodStep>,
}

#[derive(Debug, Serialize)]
pub struct FomodStep {
    pub name: String,
    /// A step behind a `<visible>` condition is shown either way -- the author
    /// still has to know it exists -- but flagged, because whether it runs
    /// depends on the rest of the list.
    pub conditional: bool,
    /// Whether the condition holds with nothing else installed, which is what
    /// `install` evaluates it against for this archive alone.
    pub visible: bool,
    pub groups: Vec<FomodGroup>,
}

#[derive(Debug, Serialize)]
pub struct FomodGroup {
    pub name: String,
    pub group_type: String,
    pub options: Vec<FomodOption>,
    /// Set when the group could not be resolved, e.g. an option type this
    /// installer does not support. Reported rather than fatal: the rest of the
    /// FOMOD is still worth printing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FomodOption {
    pub name: String,
    pub option_type: String,
    /// What `install` picks for this group when the entry declares no
    /// `fomod_selections` for it.
    pub selected_by_default: bool,
    pub install_entries: usize,
}

pub fn inspect_archive(source: &Path, include_files: bool) -> anyhow::Result<InspectReport> {
    if !source.exists() {
        anyhow::bail!("archive does not exist: {}", source.display());
    }
    if source.is_dir() {
        anyhow::bail!("not an archive (it is a directory): {}", source.display());
    }

    let paths = archive::list_archive_paths(source)?;
    let tree = ArchiveTree::from_paths(&paths);
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();

    let fomod_config = tree.find_fomod_config();
    let layout = guess_layout(&tree, fomod_config.as_deref());

    let fomod = match fomod_config {
        Some(config_path) => Some(inspect_fomod(source, &config_path)?),
        None => None,
    };

    let plugins: Vec<String> = paths
        .iter()
        .filter(|path| is_plugin_file(Path::new(path)))
        .cloned()
        .collect();

    let toml_snippet = render_toml_snippet(&file_name, &layout, fomod.as_ref());

    Ok(InspectReport {
        archive: source.display().to_string(),
        file_name,
        file_count: paths.len(),
        layout,
        toml_snippet,
        top_level_dirs: tree.child_dirs("").into_iter().collect(),
        top_level_files: tree.child_files("").into_iter().collect(),
        fomod,
        plugins,
        files: include_files.then_some(paths),
    })
}

// ── Archive shape, derived from the entry listing alone ──────────────────────

/// The directory structure of an archive, without the archive.
///
/// `list_archive_paths` returns files only, so every directory here is inferred
/// from the path segments of the files inside it. The empty string is the root.
struct ArchiveTree {
    dirs: BTreeMap<String, BTreeSet<String>>,
    files: BTreeMap<String, BTreeSet<String>>,
}

impl ArchiveTree {
    fn from_paths(paths: &[String]) -> Self {
        let mut dirs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut files: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for path in paths {
            let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let Some((file_name, dir_segments)) = segments.split_last() else {
                continue;
            };

            let mut parent = String::new();
            for segment in dir_segments {
                dirs.entry(parent.clone())
                    .or_default()
                    .insert((*segment).to_string());
                parent = if parent.is_empty() {
                    (*segment).to_string()
                } else {
                    format!("{parent}/{segment}")
                };
                dirs.entry(parent.clone()).or_default();
            }
            files
                .entry(parent)
                .or_default()
                .insert((*file_name).to_string());
        }

        Self { dirs, files }
    }

    fn child_dirs(&self, parent: &str) -> Vec<String> {
        self.dirs
            .get(parent)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn child_files(&self, parent: &str) -> Vec<String> {
        self.files
            .get(parent)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn all_dirs(&self) -> impl Iterator<Item = &String> {
        self.dirs.keys().filter(|key| !key.is_empty())
    }

    fn has_child_dir_named(&self, parent: &str, name: &str) -> bool {
        self.child_dirs(parent)
            .iter()
            .any(|child| eq_ci(child, name))
    }

    /// Does this directory look like a mod's data folder -- game content
    /// directories or loose plugins sitting directly inside it?
    fn is_content_root(&self, dir: &str) -> bool {
        if self
            .child_dirs(dir)
            .iter()
            .any(|name| is_expected_game_content_dir_name(name))
        {
            return true;
        }
        self.child_files(dir)
            .iter()
            .any(|name| is_plugin_file(Path::new(name)))
    }

    /// The `fomod/ModuleConfig.xml` (or `fomod/script.xml`) entry, if any.
    ///
    /// Matches the installer's own rule: any depth, case-insensitive, and
    /// ModuleConfig wins over the older script.xml.
    fn find_fomod_config(&self) -> Option<String> {
        let mut script_xml = None;
        for (dir, names) in &self.files {
            let leaf = dir.rsplit('/').next().unwrap_or(dir);
            if !eq_ci(leaf, "fomod") {
                continue;
            }
            for name in names {
                let full = format!("{dir}/{name}");
                if eq_ci(name, "ModuleConfig.xml") {
                    return Some(full);
                }
                if eq_ci(name, "script.xml") && script_xml.is_none() {
                    script_xml = Some(full);
                }
            }
        }
        script_xml
    }
}

/// A BAIN subpackage directory: an ordering prefix, a separator, then a name.
///
/// The prefix is digits and may carry a letter suffix -- real packages number
/// mutually exclusive alternatives `01a`, `01b`, `01c` -- so it is matched as
/// "alphanumeric run that starts with a digit", not as an integer.
fn is_numbered_subpackage(name: &str) -> bool {
    let prefix_len = name
        .char_indices()
        .find(|(_, c)| !c.is_ascii_alphanumeric())
        .map(|(idx, _)| idx);
    let Some(prefix_len) = prefix_len else {
        return false;
    };
    if prefix_len == 0 || !name.starts_with(|c: char| c.is_ascii_digit()) {
        return false;
    }
    let mut rest = name[prefix_len..].chars();
    matches!(rest.next(), Some(' ' | '-' | '_')) && rest.next().is_some()
}

fn guess_layout(tree: &ArchiveTree, fomod_config: Option<&str>) -> LayoutGuess {
    if let Some(config_path) = fomod_config {
        return LayoutGuess::Fomod {
            config_path: config_path.to_string(),
        };
    }

    let top_dirs = tree.child_dirs("");
    let top_files = tree.child_files("");

    let (numbered, other): (Vec<String>, Vec<String>) = top_dirs
        .iter()
        .cloned()
        .partition(|name| is_numbered_subpackage(name));
    // Two is the point at which numbering means something; a single `00 Core`
    // is just a folder whose author liked a prefix. Requiring the numbered ones
    // to be at least half of the top level keeps a stray `01 readme` inside an
    // ordinary mod from turning the whole archive into a BAIN package.
    if numbered.len() >= 2 && numbered.len() * 2 >= top_dirs.len() {
        return LayoutGuess::Bain {
            subpackages: numbered,
            other_top_level: other,
        };
    }

    if tree.has_child_dir_named("", "Data") {
        return LayoutGuess::Simple {
            detail: "a top-level Data/ folder, which install finds on its own".to_string(),
        };
    }
    if tree.is_content_root("") {
        return LayoutGuess::Simple {
            detail: "the archive root is already the data folder".to_string(),
        };
    }

    // A single wrapper directory is unwrapped automatically, but only when it
    // is the *only* thing at the top level -- that is the condition the auto
    // layout applies, and guessing looser here would print a snippet that then
    // fails at install time.
    if top_files.is_empty()
        && top_dirs.len() == 1
        && (tree.is_content_root(&top_dirs[0]) || tree.has_child_dir_named(&top_dirs[0], "Data"))
    {
        return LayoutGuess::Simple {
            detail: format!(
                "a single wrapper folder '{}', which install unwraps on its own",
                top_dirs[0]
            ),
        };
    }

    // Otherwise the data folder has to be named. Prefer the shallowest one;
    // several at the same depth is genuinely ambiguous and is said so.
    let mut candidates: Vec<String> = tree
        .all_dirs()
        .filter(|dir| tree.is_content_root(dir))
        .cloned()
        .collect();
    candidates.sort_by_key(|dir| (dir.matches('/').count(), dir.len()));

    let Some(shallowest_depth) = candidates.first().map(|dir| dir.matches('/').count()) else {
        return LayoutGuess::Unknown {
            detail: "no plugins and no recognisable game-content folders; run again with --files"
                .to_string(),
        };
    };
    let tied: Vec<String> = candidates
        .into_iter()
        .filter(|dir| dir.matches('/').count() == shallowest_depth)
        .collect();

    if tied.len() == 1 {
        return LayoutGuess::CustomDataFolder {
            data_folder: tied.into_iter().next().unwrap_or_default(),
        };
    }

    LayoutGuess::Unknown {
        detail: format!(
            "several folders look like a data folder ({}); pick one for data_folder, or declare \
             bain_subpackages if they are alternatives",
            tied.join(", ")
        ),
    }
}

// ── FOMOD ────────────────────────────────────────────────────────────────────

/// Pull one file out of the archive without unpacking the rest of it.
///
/// The include pattern is the entry path escaped as a literal, because mod
/// archives are full of `[` and `{` in folder names and a raw glob would either
/// fail to compile or match the wrong thing.
fn extract_single_entry(source: &Path, entry_path: &str) -> anyhow::Result<String> {
    let staging = staging_dir_for(Path::new("inspect"))?;
    std::fs::create_dir_all(&staging)
        .map_err(|err| anyhow::anyhow!("failed to create staging dir {}: {err}", staging.display()))?;

    let include = vec![globset::escape(entry_path)];
    let result = ArchiveFilters::new(&include, &[])
        .and_then(|filters| extract_with_builtins(source, &staging, &filters))
        .and_then(|_| {
            let extracted: PathBuf = staging.join(entry_path);
            if !extracted.is_file() {
                anyhow::bail!(
                    "{} lists {entry_path} but it could not be extracted",
                    source.display()
                );
            }
            read_xml_text(&extracted)
        });

    let _ = std::fs::remove_dir_all(&staging);
    result
}

fn inspect_fomod(source: &Path, config_path: &str) -> anyhow::Result<FomodReport> {
    let xml = extract_single_entry(source, config_path)?;
    let doc = Document::parse(&xml)
        .map_err(|err| anyhow::anyhow!("failed to parse FOMOD config {config_path}: {err}"))?;
    let root = doc.root_element();

    let module_name = find_child_element(root, "moduleName")
        .and_then(|node| node.text())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());

    let mut required_install_entries = 0usize;
    if let Some(required) = find_child_element(root, "requiredInstallFiles") {
        let mut entries = Vec::<FomodInstallEntry>::new();
        // A malformed required-files block should not cost the author the whole
        // report; the count is a convenience, not the point of the command.
        if collect_fomod_entries(required, &mut entries).is_ok() {
            required_install_entries = entries.len();
        }
    }

    // Empty, exactly as `install` evaluates a lone archive: nothing else is on
    // disk yet, so this is the answer the author will get from a first run.
    let active_plugins: HashSet<String> = HashSet::new();
    let mut flags = HashMap::<String, String>::new();
    let no_selections = build_fomod_selection_map(&[]);

    let mut steps = Vec::new();
    if let Some(install_steps) = find_child_element(root, "installSteps") {
        for step in child_elements(install_steps, "installStep") {
            let step_name = step.attribute("name").unwrap_or("").to_string();
            let visible_node = find_child_element(step, "visible");
            let conditional = visible_node.is_some();
            let visible = match visible_node.and_then(|node| find_child_element(node, "dependencies"))
            {
                Some(deps) => {
                    fomod_dependencies_match(deps, &active_plugins, &flags).unwrap_or(false)
                }
                None => true,
            };

            let mut groups = Vec::new();
            for file_groups in child_elements(step, "optionalFileGroups") {
                for group in child_elements(file_groups, "group") {
                    let group_name = group.attribute("name").unwrap_or("").to_string();
                    let group_type = group.attribute("type").unwrap_or("SelectAny").to_string();

                    let Some(plugins) = find_child_element(group, "plugins") else {
                        groups.push(FomodGroup {
                            name: group_name,
                            group_type,
                            options: Vec::new(),
                            note: Some("group declares no <plugins>".to_string()),
                        });
                        continue;
                    };

                    let plugin_nodes: Vec<roxmltree::Node<'_, '_>> =
                        child_elements(plugins, "plugin").collect();
                    let mut note: Option<String> = None;
                    let mut option_types: Vec<FomodOptionType> =
                        Vec::with_capacity(plugin_nodes.len());
                    for plugin in &plugin_nodes {
                        match fomod_option_type(*plugin, &active_plugins, &flags) {
                            Ok(option_type) => option_types.push(option_type),
                            Err(err) => {
                                // Reported, then treated as an ordinary option
                                // so the rest of the group still prints.
                                note.get_or_insert_with(|| err.to_string());
                                option_types.push(FomodOptionType::Optional);
                            }
                        }
                    }

                    let selected = match select_fomod_options(
                        &step_name,
                        &group_name,
                        &group_type,
                        &plugin_nodes,
                        &option_types,
                        &no_selections,
                    ) {
                        Ok(selected) => selected,
                        Err(err) => {
                            note.get_or_insert_with(|| err.to_string());
                            Vec::new()
                        }
                    };

                    let mut options = Vec::with_capacity(plugin_nodes.len());
                    for (idx, plugin) in plugin_nodes.iter().enumerate() {
                        let mut entries = Vec::<FomodInstallEntry>::new();
                        if let Some(files) = find_child_element(*plugin, "files") {
                            let _ = collect_fomod_entries(files, &mut entries);
                        }
                        options.push(FomodOption {
                            name: plugin.attribute("name").unwrap_or("").to_string(),
                            option_type: format!("{:?}", option_types[idx]),
                            selected_by_default: selected.contains(&idx),
                            install_entries: entries.len(),
                        });
                    }

                    // Mirror the installer: a default-selected option's
                    // condition flags are what later steps are evaluated
                    // against, so later groups report the same types install
                    // would compute.
                    for idx in &selected {
                        if let Some(condition_flags) =
                            find_child_element(plugin_nodes[*idx], "conditionFlags")
                        {
                            for flag in child_elements(condition_flags, "flag") {
                                let Some(name) = flag.attribute("name") else {
                                    continue;
                                };
                                flags.insert(
                                    name.to_ascii_lowercase(),
                                    flag.text().unwrap_or("").trim().to_string(),
                                );
                            }
                        }
                    }

                    groups.push(FomodGroup {
                        name: group_name,
                        group_type,
                        options,
                        note,
                    });
                }
            }

            steps.push(FomodStep {
                name: step_name,
                conditional,
                visible,
                groups,
            });
        }
    }

    Ok(FomodReport {
        module_name,
        config_path: config_path.to_string(),
        required_install_entries,
        steps,
    })
}

// ── Rendering ────────────────────────────────────────────────────────────────

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn render_toml_snippet(
    file_name: &str,
    layout: &LayoutGuess,
    fomod: Option<&FomodReport>,
) -> String {
    let mut out = String::from("[[mods.archives]]\n");
    out.push_str("path = \"nexus:oblivion/<mod id>/<file id>\"\n");
    out.push_str(&format!("file_name = {}\n", toml_string(file_name)));

    match layout {
        LayoutGuess::Fomod { .. } => {
            out.push_str("layout = \"fomod\"\n");
            if let Some(fomod) = fomod {
                for step in &fomod.steps {
                    for group in &step.groups {
                        let chosen: Vec<String> = group
                            .options
                            .iter()
                            .filter(|option| option.selected_by_default)
                            .map(|option| toml_string(&option.name))
                            .collect();
                        out.push_str("\n[[mods.archives.fomod_selections]]\n");
                        out.push_str(&format!("step = {}\n", toml_string(&step.name)));
                        out.push_str(&format!("group = {}\n", toml_string(&group.name)));
                        out.push_str(&format!("options = [{}]\n", chosen.join(", ")));
                    }
                }
            }
        }
        LayoutGuess::Bain { subpackages, .. } => {
            out.push_str("layout = \"bain\"\n");
            // Every subpackage, one per line, because some of them are always
            // alternatives to each other (`01a`, `01b`, ...) and the author has
            // to delete the ones they are not taking.
            out.push_str("bain_subpackages = [\n");
            for name in subpackages {
                out.push_str(&format!("  {},\n", toml_string(name)));
            }
            out.push_str("]\n");
        }
        LayoutGuess::Simple { .. } => {
            out.push_str("# no layout, data_folder or target_subdir needed\n");
        }
        LayoutGuess::CustomDataFolder { data_folder } => {
            out.push_str("layout = \"custom-data-folder\"\n");
            out.push_str(&format!("data_folder = {}\n", toml_string(data_folder)));
        }
        LayoutGuess::Unknown { .. } => {
            out.push_str("# layout could not be guessed; see the notes above\n");
        }
    }

    out
}

pub fn render_text(report: &InspectReport) -> String {
    let mut out = String::new();

    out.push_str(&format!("archive: {}\n", report.archive));
    out.push_str(&format!(
        "files:   {} ({} top-level {}, {} top-level {})\n",
        report.file_count,
        report.top_level_dirs.len(),
        plural(report.top_level_dirs.len(), "directory", "directories"),
        report.top_level_files.len(),
        plural(report.top_level_files.len(), "file", "files"),
    ));

    out.push_str(&format!("\nlayout guess: {}\n", report.layout.label()));
    match &report.layout {
        LayoutGuess::Fomod { config_path } => {
            out.push_str(&format!("  installer at {config_path}\n"));
        }
        LayoutGuess::Bain {
            subpackages,
            other_top_level,
        } => {
            out.push_str(&format!("  {} subpackages:\n", subpackages.len()));
            for name in subpackages {
                out.push_str(&format!("    {name}\n"));
            }
            if !other_top_level.is_empty() {
                out.push_str(&format!(
                    "  not subpackages: {}\n",
                    other_top_level.join(", ")
                ));
            }
            out.push_str(
                "  every subpackage is listed; delete the ones that are alternatives to each \
                 other.\n",
            );
        }
        LayoutGuess::Simple { detail } => out.push_str(&format!("  {detail}\n")),
        LayoutGuess::CustomDataFolder { data_folder } => {
            out.push_str(&format!("  data folder is '{data_folder}'\n"));
        }
        LayoutGuess::Unknown { detail } => out.push_str(&format!("  {detail}\n")),
    }

    // BAIN already printed its top level as the subpackage list.
    if !report.top_level_dirs.is_empty() && !matches!(report.layout, LayoutGuess::Bain { .. }) {
        out.push_str(&format!(
            "  top level: {}\n",
            summarise(&report.top_level_dirs, 12)
        ));
    }

    if let Some(fomod) = &report.fomod {
        out.push_str("\nFOMOD");
        if let Some(name) = &fomod.module_name {
            out.push_str(&format!(": {name}"));
        }
        out.push('\n');
        if fomod.required_install_entries > 0 {
            out.push_str(&format!(
                "  {} required install {} (no choice involved)\n",
                fomod.required_install_entries,
                plural(fomod.required_install_entries, "entry", "entries")
            ));
        }
        if fomod.steps.is_empty() {
            out.push_str("  no install steps\n");
        }
        for step in &fomod.steps {
            let flag = match (step.conditional, step.visible) {
                (false, _) => String::new(),
                (true, true) => "  [conditional: shown for this archive alone]".to_string(),
                (true, false) => "  [conditional: hidden for this archive alone]".to_string(),
            };
            out.push_str(&format!("  step \"{}\"{flag}\n", step.name));
            for group in &step.groups {
                out.push_str(&format!(
                    "    group \"{}\" ({})\n",
                    group.name, group.group_type
                ));
                if let Some(note) = &group.note {
                    out.push_str(&format!("      ! {note}\n"));
                }
                for option in &group.options {
                    let marker = if option.selected_by_default { '*' } else { ' ' };
                    let quoted = format!("\"{}\"", option.name);
                    let entries = option.install_entries;
                    out.push_str(&format!(
                        "      {marker} {quoted:<30} {:<14} {entries} install {}\n",
                        option.option_type,
                        plural(entries, "entry", "entries"),
                    ));
                }
            }
        }
        out.push_str(
            "\n  * marks what install picks for a group with no fomod_selections entry.\n",
        );
    }

    if !report.plugins.is_empty() {
        out.push_str(&format!(
            "\nplugins ({}) -- these go in the modlist's `plugins` load order by hand:\n",
            report.plugins.len()
        ));
        for plugin in &report.plugins {
            out.push_str(&format!("  {plugin}\n"));
        }
    }

    out.push_str("\npaste into the modlist:\n\n");
    for line in report.toml_snippet.lines() {
        out.push_str(&format!("  {line}\n"));
    }

    match &report.files {
        Some(files) => {
            out.push_str(&format!("\nfiles ({}):\n", files.len()));
            for path in files {
                out.push_str(&format!("  {path}\n"));
            }
        }
        None => {
            out.push_str(&format!(
                "\n{} {} in the archive; pass --files to list them.\n",
                report.file_count,
                plural(report.file_count, "file", "files"),
            ));
        }
    }

    out
}

fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 { one } else { many }
}

/// First `limit` names, then a count of the rest. A texture pack's top level is
/// short, but a badly packed one is not, and this is the summary view.
fn summarise(values: &[String], limit: usize) -> String {
    if values.len() <= limit {
        return values.join(", ");
    }
    format!(
        "{}, ... (+{} more)",
        values[..limit].join(", "),
        values.len() - limit
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(paths: &[&str]) -> ArchiveTree {
        let owned: Vec<String> = paths.iter().map(|p| (*p).to_string()).collect();
        ArchiveTree::from_paths(&owned)
    }

    #[test]
    fn a_top_level_data_folder_needs_no_declaration() {
        let tree = tree(&["Data/Meshes/foo.nif", "Data/Thing.esp"]);
        assert!(matches!(
            guess_layout(&tree, None),
            LayoutGuess::Simple { .. }
        ));
    }

    #[test]
    fn a_bare_content_root_needs_no_declaration() {
        let tree = tree(&["Textures/foo.dds", "readme.txt"]);
        assert!(matches!(
            guess_layout(&tree, None),
            LayoutGuess::Simple { .. }
        ));
    }

    #[test]
    fn a_nested_data_folder_is_named() {
        // Two top-level entries, so the auto layout will not unwrap either.
        let tree = tree(&["Docs/readme.txt", "Optional/Textures/foo.dds"]);
        match guess_layout(&tree, None) {
            LayoutGuess::CustomDataFolder { data_folder } => {
                assert_eq!(data_folder, "Optional");
            }
            other => panic!("expected a nested data folder, got {other:?}"),
        }
    }

    #[test]
    fn numbered_top_level_directories_are_bain_subpackages() {
        let tree = tree(&[
            "00 Core/Thing.esp",
            "01 Option/Textures/foo.dds",
            "02 Extra/readme.txt",
        ]);
        match guess_layout(&tree, None) {
            LayoutGuess::Bain { subpackages, .. } => {
                assert_eq!(subpackages, vec!["00 Core", "01 Option", "02 Extra"]);
            }
            other => panic!("expected BAIN, got {other:?}"),
        }
    }

    #[test]
    fn a_single_numbered_folder_is_not_a_bain_package() {
        // One prefixed folder is a naming habit, not a package structure.
        let tree = tree(&["00 Core/Meshes/foo.nif"]);
        assert!(matches!(
            guess_layout(&tree, None),
            LayoutGuess::Simple { .. }
        ));
    }

    #[test]
    fn the_fomod_config_is_found_at_any_depth_and_any_casing() {
        let tree = tree(&["Wrapper/FOMOD/moduleconfig.xml", "Wrapper/base.txt"]);
        assert_eq!(
            tree.find_fomod_config().as_deref(),
            Some("Wrapper/FOMOD/moduleconfig.xml")
        );
    }

    #[test]
    fn toml_snippet_quotes_names_containing_quotes_and_backslashes() {
        let layout = LayoutGuess::CustomDataFolder {
            data_folder: "Odd\"Name\\Here".to_string(),
        };
        let snippet = render_toml_snippet("Mod-1-0.7z", &layout, None);
        assert!(snippet.contains(r#"data_folder = "Odd\"Name\\Here""#), "{snippet}");
    }
}
