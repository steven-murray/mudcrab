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

use crate::archive::{self, ArchiveFilters};
use crate::config::install::is_plugin_file;
use crate::config::install::layout::auto::{content_roots_below, detect_auto_root};
use crate::config::install::layout::fomod::{
    FomodInstallEntry, FomodOptionType, build_fomod_selection_map, child_elements,
    collect_fomod_entries, find_child_element, find_fomod_config, fomod_dependencies_match,
    fomod_option_type, select_fomod_options,
};
use crate::config::install::layout::plan::{Listing, plan_data_folder};
use crate::util::fs::eq_ci;
use roxmltree::Document;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

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

    let fomod_config = find_fomod_config(&paths);
    let layout = guess_layout(&tree, &paths, fomod_config.as_deref());

    let fomod = match fomod_config {
        Some(config_path) => Some(inspect_fomod(source, &config_path)?),
        None => None,
    };

    let plugins: Vec<String> = paths
        .iter()
        .filter(|path| is_plugin_file(Path::new(path)))
        .cloned()
        .collect();

    // A download MO2 (or mudcrab) made has a `.meta` naming its Nexus ids, so
    // the snippet can say where it came from instead of leaving a blank.
    let source_path = crate::config::archive_store::meta_source(source);
    let toml_snippet =
        render_toml_snippet(&file_name, source_path.as_deref(), &layout, fomod.as_ref());

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

fn guess_layout(tree: &ArchiveTree, paths: &[String], fomod_config: Option<&str>) -> LayoutGuess {
    if let Some(config_path) = fomod_config {
        return LayoutGuess::Fomod {
            config_path: config_path.to_string(),
        };
    }

    let top_dirs = tree.child_dirs("");

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

    // From here the question is the one install answers, so install's code
    // answers it: the same detection the archive will meet when it is installed
    // with no layout declared. Two implementations agreed on every archive tried
    // and were guaranteed to agree on none.
    let listing = Listing::new(paths);
    // No mod id: install's "folder named after the mod" shape is a lone
    // wrapper folder, which the wrapper rule already unwraps by any name.
    match detect_auto_root(&listing, "\u{0}", "this archive") {
        Ok(root) if root.is_empty() => LayoutGuess::Simple {
            detail: "the archive root is already the data folder".to_string(),
        },
        Ok(root) if eq_ci(&root, "Data") => LayoutGuess::Simple {
            detail: "a top-level Data/ folder, which install finds on its own".to_string(),
        },
        Ok(root) => LayoutGuess::Simple {
            detail: format!("'{root}', which install finds on its own"),
        },
        Err(install_says) => {
            // Install will not take it as it stands. Suggest the shallowest
            // folder holding content, if there is exactly one, and only after
            // checking that install's own data_folder path accepts it.
            let roots = content_roots_below(&listing, "");
            let shallowest = roots.first().map(|dir| dir.matches('/').count());
            let tied: Vec<&String> = roots
                .iter()
                .filter(|dir| Some(dir.matches('/').count()) == shallowest)
                .collect();
            match tied.as_slice() {
                [only]
                    if ArchiveFilters::new(&[], &[]).is_ok_and(|filters| {
                        plan_data_folder(paths, Some(only), &filters, "this archive")
                            .is_ok_and(|plan| !plan.is_empty())
                    }) =>
                {
                    LayoutGuess::CustomDataFolder {
                        data_folder: (*only).clone(),
                    }
                }
                _ => LayoutGuess::Unknown {
                    detail: format!("install would refuse it: {install_says}"),
                },
            }
        }
    }
}

// ── FOMOD ────────────────────────────────────────────────────────────────────

/// Pull one file out of the archive without unpacking the rest of it.
///
/// The include pattern is the entry path escaped as a literal, because mod
/// archives are full of `[` and `{` in folder names and a raw glob would either
/// fail to compile or match the wrong thing.
/// One text file out of an archive -- a FOMOD config -- pulled out into the
/// user's cache and decoded. Never the whole archive, never `/tmp`.
pub(crate) fn extract_single_entry(source: &Path, entry_path: &str) -> anyhow::Result<String> {
    let scratch = crate::util::fs::cache_scratch_dir("inspect");
    let bytes = crate::archive::read_entry(source, entry_path, &scratch)?;
    crate::config::install::layout::fomod::decode_xml(&bytes, entry_path)
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
            let visible =
                match visible_node.and_then(|node| find_child_element(node, "dependencies")) {
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

/// Choices made for an installer, in place of its defaults.
pub(crate) enum Chosen {
    Fomod(Vec<crate::config::schema::FomodSelection>),
    Bain(Vec<String>),
    /// What a BAIN wizard selected: subpackages, and plugins in them it left
    /// unselected (written as `exclude`).
    Wizard {
        subpackages: Vec<String>,
        exclude: Vec<String>,
    },
}

pub(crate) fn render_toml_snippet(
    file_name: &str,
    source_path: Option<&str>,
    layout: &LayoutGuess,
    fomod: Option<&FomodReport>,
) -> String {
    render_toml_snippet_with(file_name, source_path, layout, fomod, None)
}

pub(crate) fn render_toml_snippet_with(
    file_name: &str,
    source_path: Option<&str>,
    layout: &LayoutGuess,
    fomod: Option<&FomodReport>,
    chosen: Option<&Chosen>,
) -> String {
    let mut out = String::from("[[mods.archives]]\n");
    out.push_str(&format!(
        "path = {}\n",
        toml_string(source_path.unwrap_or("nexus:oblivion/<mod id>/<file id>"))
    ));
    out.push_str(&format!("file_name = {}\n", toml_string(file_name)));

    match layout {
        LayoutGuess::Fomod { .. } => {
            out.push_str("layout = \"fomod\"\n");
            if let Some(Chosen::Fomod(selections)) = chosen {
                for selection in selections {
                    let options: Vec<String> =
                        selection.options.iter().map(|o| toml_string(o)).collect();
                    out.push_str("\n[[mods.archives.fomod_selections]]\n");
                    out.push_str(&format!("step = {}\n", toml_string(&selection.step)));
                    out.push_str(&format!("group = {}\n", toml_string(&selection.group)));
                    out.push_str(&format!("options = [{}]\n", options.join(", ")));
                }
            } else if let Some(fomod) = fomod {
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
            let (subpackages, exclude) = match chosen {
                Some(Chosen::Bain(picked)) => (picked, None),
                Some(Chosen::Wizard {
                    subpackages,
                    exclude,
                }) => (subpackages, Some(exclude)),
                _ => (subpackages, None),
            };
            for name in subpackages {
                out.push_str(&format!("  {},\n", toml_string(name)));
            }
            out.push_str("]\n");
            if let Some(exclude) = exclude.filter(|e| !e.is_empty()) {
                // Globs, matched inside each subpackage: escaped, so a
                // plugin named with brackets matches itself.
                let names: Vec<String> = exclude
                    .iter()
                    .map(|name| toml_string(&globset::escape(name)))
                    .collect();
                out.push_str(&format!("exclude = [{}]\n", names.join(", ")));
            }
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
            "\nplugins ({}) -- `add` places them with --plugin and --load-after or --load-last:\n",
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

    fn guess(paths: &[&str]) -> LayoutGuess {
        let owned: Vec<String> = paths.iter().map(|p| (*p).to_string()).collect();
        guess_layout(&ArchiveTree::from_paths(&owned), &owned, None)
    }

    #[test]
    fn a_top_level_data_folder_needs_no_declaration() {
        assert!(matches!(
            guess(&["Data/Meshes/foo.nif", "Data/Thing.esp"]),
            LayoutGuess::Simple { .. }
        ));
    }

    #[test]
    fn a_bare_content_root_needs_no_declaration() {
        assert!(matches!(
            guess(&["Textures/foo.dds", "readme.txt"]),
            LayoutGuess::Simple { .. }
        ));
    }

    #[test]
    fn a_nested_data_folder_is_named() {
        // Two top-level entries, so the auto layout will not unwrap either.
        match guess(&["Docs/readme.txt", "Optional/Textures/foo.dds"]) {
            LayoutGuess::CustomDataFolder { data_folder } => {
                assert_eq!(data_folder, "Optional");
            }
            other => panic!("expected a nested data folder, got {other:?}"),
        }
    }

    #[test]
    fn numbered_top_level_directories_are_bain_subpackages() {
        match guess(&[
            "00 Core/Thing.esp",
            "01 Option/Textures/foo.dds",
            "02 Extra/readme.txt",
        ]) {
            LayoutGuess::Bain { subpackages, .. } => {
                assert_eq!(subpackages, vec!["00 Core", "01 Option", "02 Extra"]);
            }
            other => panic!("expected BAIN, got {other:?}"),
        }
    }

    #[test]
    fn a_single_numbered_folder_is_not_a_bain_package() {
        // One prefixed folder is a naming habit, not a package structure.
        assert!(matches!(
            guess(&["00 Core/Meshes/foo.nif"]),
            LayoutGuess::Simple { .. }
        ));
    }

    #[test]
    fn a_plugin_beside_a_data_folder_follows_the_plugin_as_install_does() {
        // The old guess preferred Data/; install follows the plugin to the root.
        match guess(&["Thing.esp", "Data/Textures/foo.dds"]) {
            LayoutGuess::Simple { detail } => assert!(detail.contains("root"), "{detail}"),
            other => panic!("expected the root, got {other:?}"),
        }
    }

    #[test]
    fn a_lone_folder_holding_the_plugin_is_unwrapped_whatever_it_is_called() {
        // Install's "folder named after the mod" rule is a special case of
        // this one, which is why inspect needs no mod id.
        assert!(matches!(
            guess(&["Lost Spires/LostSpires.esp", "Lost Spires/Meshes/a.nif"]),
            LayoutGuess::Simple { .. }
        ));
    }

    #[test]
    fn the_snippet_takes_its_source_from_the_archives_meta() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("Mod-123-1-0.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        zip.start_file("Mod.esp", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, b"TES4").unwrap();
        zip.finish().unwrap();
        std::fs::write(
            dir.path().join("Mod-123-1-0.zip.meta"),
            "[General]\ngameName=Oblivion\nmodID=123\nfileID=456\n",
        )
        .unwrap();

        let report = inspect_archive(&archive, false).unwrap();
        assert!(
            report
                .toml_snippet
                .contains("path = \"nexus:oblivion/123/456\""),
            "{}",
            report.toml_snippet
        );
    }

    #[test]
    fn toml_snippet_quotes_names_containing_quotes_and_backslashes() {
        let layout = LayoutGuess::CustomDataFolder {
            data_folder: "Odd\"Name\\Here".to_string(),
        };
        let snippet = render_toml_snippet("Mod-1-0.7z", None, &layout, None);
        assert!(
            snippet.contains(r#"data_folder = "Odd\"Name\\Here""#),
            "{snippet}"
        );
    }
}

// --- BSA ---------------------------------------------------------------------

/// What `inspect` reports for a `.bsa`.
///
/// A BSA is not an installable archive -- it has no layout to guess and no
/// modlist entry to paste -- so it gets its own report rather than being forced
/// into `InspectReport`. What matters about one is its shape: how many files,
/// how big they are unpacked, and whether the payloads are compressed, which is
/// what decides whether two archives of the same content are the same size.
#[derive(Debug, Serialize)]
pub struct BsaReport {
    pub archive: String,
    pub file_name: String,
    pub folder_count: usize,
    pub file_count: usize,
    /// Bytes the payloads occupy in the archive, as stored.
    pub stored_bytes: u64,
    /// Archive-wide flags, including `FLAG_COMPRESSED`.
    pub archive_flags: u32,
    pub file_flags: u32,
    /// Whether the archive's default is compressed payloads.
    pub compressed_by_default: bool,
    /// Files whose payload is actually compressed. A file may opt out of the
    /// archive default, so this is counted rather than inferred from the flag.
    pub compressed_files: usize,
    pub plugins: Vec<String>,
    /// Every path in the archive; only populated with `--files`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
}

pub fn inspect_bsa(source: &Path, include_files: bool) -> anyhow::Result<BsaReport> {
    let bytes = std::fs::read(source)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", source.display()))?;
    let archive = crate::bsa::Bsa::parse(&bytes)
        .map_err(|err| anyhow::anyhow!("failed to parse {}: {err}", source.display()))?;

    let mut stored_bytes = 0u64;
    let mut compressed_files = 0usize;
    for (_, file) in archive.files() {
        stored_bytes += file.stored_bytes().len() as u64;
        if file.compressed {
            compressed_files += 1;
        }
    }

    let mut paths: Vec<String> = archive.paths().collect();
    paths.sort();
    let plugins: Vec<String> = paths
        .iter()
        .filter(|path| is_plugin_file(Path::new(&path.replace('\\', "/"))))
        .cloned()
        .collect();

    Ok(BsaReport {
        archive: source.display().to_string(),
        file_name: source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string(),
        folder_count: archive.folders.len(),
        file_count: archive.file_count(),
        stored_bytes,
        archive_flags: archive.archive_flags,
        file_flags: archive.file_flags,
        compressed_by_default: archive.compressed_by_default(),
        compressed_files,
        plugins,
        files: include_files.then_some(paths),
    })
}

pub fn render_bsa_text(report: &BsaReport) -> String {
    let mut out = String::new();

    out.push_str(&format!("archive: {}\n", report.archive));
    out.push_str(&format!(
        "files:   {} in {} {}\n",
        report.file_count,
        report.folder_count,
        plural(report.folder_count, "folder", "folders"),
    ));
    out.push_str(&format!("payload: {} bytes stored\n", report.stored_bytes));
    out.push_str(&format!(
        "flags:   archive 0x{:08x}, file 0x{:08x}\n",
        report.archive_flags, report.file_flags
    ));
    out.push_str(&format!(
        "packed:  {} ({} of {} {} compressed)\n",
        if report.compressed_by_default {
            "compressed by default"
        } else {
            "uncompressed by default"
        },
        report.compressed_files,
        report.file_count,
        plural(report.file_count, "file", "files"),
    ));

    if !report.plugins.is_empty() {
        out.push_str(&format!("\n{} plugins:\n", report.plugins.len()));
        for plugin in &report.plugins {
            out.push_str(&format!("  {plugin}\n"));
        }
    }

    match &report.files {
        Some(files) => {
            out.push_str(&format!("\n{} files:\n", files.len()));
            for path in files {
                out.push_str(&format!("  {path}\n"));
            }
        }
        None => {
            out.push_str(&format!(
                "\n{} files in the archive; pass --files to list them.\n",
                report.file_count
            ));
        }
    }

    out
}
