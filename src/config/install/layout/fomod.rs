//! FOMOD installer support: parse ModuleConfig.xml and apply the selected options.

use super::plan::{folded_destination, strip_dir_prefix, LayoutPlan};
use super::{apply_plan, destination_for, with_staged_archive};
use crate::archive::ArchiveFilters;
use crate::config::schema::{CompiledArchive, FomodSelection};
use crate::util::fs::{eq_ci, normalize_relative_path};
use roxmltree::{Document, Node};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FomodOptionType {
    Required,
    Recommended,
    Optional,
    CouldBeUsable,
    NotUsable,
}

#[derive(Debug, Clone)]
pub(crate) enum FomodInstallEntry {
    File {
        source: String,
        destination: String,
        priority: i32,
    },
    Folder {
        source: String,
        destination: String,
        priority: i32,
    },
}

impl FomodInstallEntry {
    fn priority(&self) -> i32 {
        match self {
            Self::File { priority, .. } | Self::Folder { priority, .. } => *priority,
        }
    }
}

pub(crate) fn extract_archive_with_fomod_layout(
    source: &Path,
    target_root: &Path,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    if archive.data_folder.is_some() {
        anyhow::bail!(
            "FOMOD layout for {} cannot be combined with data_folder",
            source.display()
        );
    }
    if !archive.bain_subpackages.is_empty() {
        anyhow::bail!(
            "FOMOD layout for {} cannot be combined with bain_subpackages",
            source.display()
        );
    }

    let destination_root = destination_for(target_root, archive.target_subdir.as_deref())?;
    with_staged_archive(source, target_root, |staging_dir| {
        apply_fomod_from_staging(
            staging_dir,
            &destination_root,
            archive,
            filters,
            active_plugins,
        )
    })
}

pub(crate) fn apply_fomod_from_staging(
    staging_dir: &Path,
    destination_root: &Path,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    let paths = super::bain::list_relative_paths(staging_dir)?;
    let config_path = find_fomod_config(&paths).ok_or_else(|| {
        anyhow::anyhow!(
            "FOMOD archive {} does not contain fomod/ModuleConfig.xml or fomod/script.xml",
            staging_dir.display()
        )
    })?;

    let xml = read_xml_text(&staging_dir.join(&config_path))?;
    let plan = plan_fomod(
        &paths,
        &config_path,
        &xml,
        archive,
        filters,
        active_plugins,
    )?;
    apply_plan(staging_dir, destination_root, &plan)
}

/// The `fomod/ModuleConfig.xml` (or `fomod/script.xml`) entry, if the archive
/// has one. `ModuleConfig` wins over the older `script.xml` wherever both exist.
///
/// An archive may ship several -- a wrapper holding two alternative installers,
/// or a nested FOMOD inside a subpackage. The shallowest wins, then the
/// alphabetically first, because the previous version took whichever
/// `read_dir` happened to yield last, and that is not a decision.
pub(crate) fn find_fomod_config(paths: &[String]) -> Option<String> {
    let mut module_config: Option<String> = None;
    let mut script_xml: Option<String> = None;

    for path in paths {
        let normalized = path.replace('\\', "/");
        let Some((dir, name)) = normalized.rsplit_once('/') else {
            continue;
        };
        // The FOMOD spec puts the config in a `fomod/` folder, and the content
        // root is that folder's parent -- so a config found anywhere else has
        // no content root to speak of.
        if !eq_ci(dir.rsplit('/').next().unwrap_or(dir), "fomod") {
            continue;
        }

        let slot = if eq_ci(name, "ModuleConfig.xml") {
            &mut module_config
        } else if eq_ci(name, "script.xml") {
            &mut script_xml
        } else {
            continue;
        };
        if slot
            .as_deref()
            .is_none_or(|seen| sort_key(&normalized) < sort_key(seen))
        {
            *slot = Some(normalized);
        }
    }

    module_config.or(script_xml)
}

fn sort_key(path: &str) -> (usize, &str) {
    (path.matches('/').count(), path)
}

/// The folder the installer's `source` paths are relative to: the parent of the
/// `fomod/` directory, or the archive root when `fomod/` sits at the top.
fn fomod_content_root(config_path: &str) -> String {
    config_path
        .rsplit_once('/')
        .and_then(|(fomod_dir, _)| fomod_dir.rsplit_once('/'))
        .map(|(root, _)| root.to_string())
        .unwrap_or_default()
}

/// What a FOMOD contributes, from the archive's entry list and its config.
///
/// The config is the one piece of file *content* any layout needs, so it is
/// read first and everything else is decided from paths alone.
pub(crate) fn plan_fomod(
    paths: &[String],
    config_path: &str,
    xml: &str,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<LayoutPlan> {
    let doc = Document::parse(xml)
        .map_err(|err| anyhow::anyhow!("failed to parse FOMOD config {config_path}: {err}"))?;
    let entries = collect_selected_entries(doc.root_element(), archive, active_plugins)?;
    plan_fomod_entries(
        paths,
        &fomod_content_root(config_path),
        &entries,
        filters,
        config_path,
    )
}

/// Walk the install steps, applying the modlist's answers, and gather every
/// file and folder the resulting selection installs.
fn collect_selected_entries(
    root: Node<'_, '_>,
    archive: &CompiledArchive,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<Vec<FomodInstallEntry>> {
    let selection_map = build_fomod_selection_map(&archive.fomod_selections);
    let mut flags = HashMap::<String, String>::new();
    let mut entries = Vec::<FomodInstallEntry>::new();

    if let Some(required) = find_child_element(root, "requiredInstallFiles") {
        collect_fomod_entries(required, &mut entries)?;
    }

    let Some(steps) = find_child_element(root, "installSteps") else {
        return Ok(entries);
    };

    for step in child_elements(steps, "installStep") {
        if let Some(visible) = find_child_element(step, "visible")
            && let Some(deps) = find_child_element(visible, "dependencies")
            && !fomod_dependencies_match(deps, active_plugins, &flags)?
        {
            continue;
        }

        let step_name = step.attribute("name").unwrap_or("");
        for file_groups in child_elements(step, "optionalFileGroups") {
            for group in child_elements(file_groups, "group") {
                let group_name = group.attribute("name").unwrap_or("");
                let group_type = group.attribute("type").unwrap_or("SelectAny");
                let plugins = find_child_element(group, "plugins").ok_or_else(|| {
                    anyhow::anyhow!(
                        "FOMOD group '{}' in step '{}' is missing <plugins>",
                        group_name,
                        step_name
                    )
                })?;

                let plugin_nodes: Vec<Node<'_, '_>> = child_elements(plugins, "plugin").collect();
                let option_types: Vec<FomodOptionType> = plugin_nodes
                    .iter()
                    .map(|plugin| fomod_option_type(*plugin, active_plugins, &flags))
                    .collect::<anyhow::Result<_>>()?;

                let selected_indices = select_fomod_options(
                    step_name,
                    group_name,
                    group_type,
                    &plugin_nodes,
                    &option_types,
                    &selection_map,
                )?;

                for idx in selected_indices {
                    let plugin = plugin_nodes[idx];
                    if let Some(condition_flags) = find_child_element(plugin, "conditionFlags") {
                        for flag in child_elements(condition_flags, "flag") {
                            let Some(name) = flag.attribute("name") else {
                                continue;
                            };
                            let value = flag.text().unwrap_or("").trim().to_string();
                            flags.insert(name.to_ascii_lowercase(), value);
                        }
                    }

                    if let Some(files) = find_child_element(plugin, "files") {
                        collect_fomod_entries(files, &mut entries)?;
                    }
                }
            }
        }
    }

    Ok(entries)
}

/// Resolve the selected install entries against the archive's entry list.
///
/// Entries are laid down lowest priority first, so a higher-priority entry
/// writing the same destination replaces it -- which `LayoutPlan::from_pairs`
/// settles by keeping the last writer.
pub(crate) fn plan_fomod_entries(
    paths: &[String],
    content_root: &str,
    entries: &[FomodInstallEntry],
    filters: &ArchiveFilters,
    source_label: &str,
) -> anyhow::Result<LayoutPlan> {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(FomodInstallEntry::priority);

    let mut pairs: Vec<(String, String)> = Vec::new();

    for entry in &sorted {
        match entry {
            FomodInstallEntry::File {
                source,
                destination,
                ..
            } => {
                // Filtered before resolving, so an `exclude`d entry naming a
                // file the archive lacks stays as harmless as it was when the
                // copy simply never happened.
                let rel_source = normalize_fomod_path(source)?;
                if !filters.should_extract(&rel_source) {
                    continue;
                }

                let wanted = join_under(content_root, &rel_source);
                let archive_path = find_path_ignoring_case(paths, &wanted).ok_or_else(|| {
                    anyhow::anyhow!(
                        "FOMOD install in {source_label} names a file the archive does not \
                         contain: '{source}'"
                    )
                })?;
                pairs.push((
                    archive_path,
                    folded_destination(&normalize_fomod_path(destination)?),
                ));
            }
            FomodInstallEntry::Folder {
                source,
                destination,
                ..
            } => {
                let prefix = join_under(content_root, &normalize_fomod_path(source)?);
                // Every component is a directory, so the whole prefix folds.
                let dest_prefix = normalize_fomod_path_allow_empty(destination)?.to_lowercase();

                let mut matched = 0usize;
                for path in paths {
                    let Some(rest) = strip_dir_prefix(path, &prefix) else {
                        continue;
                    };
                    matched += 1;
                    // A folder's filters apply within the folder, not from the
                    // content root -- which is what the recursive copy this
                    // replaced did, and what `exclude` patterns were written
                    // against.
                    if !filters.should_extract(&rest) {
                        continue;
                    }
                    let folded = folded_destination(&rest);
                    pairs.push((path.clone(), join_under(&dest_prefix, &folded)));
                }

                if matched == 0 {
                    anyhow::bail!(
                        "FOMOD install in {source_label} names a folder the archive does not \
                         contain: '{source}'"
                    );
                }
            }
        }
    }

    Ok(LayoutPlan::from_pairs(pairs))
}

fn join_under(prefix: &str, rest: &str) -> String {
    if prefix.is_empty() {
        rest.to_string()
    } else {
        format!("{prefix}/{rest}")
    }
}

/// FOMOD configs are authored on Windows and routinely disagree with the
/// archive about case, so the lookup cannot be exact.
fn find_path_ignoring_case(paths: &[String], wanted: &str) -> Option<String> {
    paths
        .iter()
        .find(|path| path.replace('\\', "/").eq_ignore_ascii_case(wanted))
        .cloned()
}

pub(crate) fn read_xml_text(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    decode_xml(&bytes, &path.display().to_string())
}

/// Decode a FOMOD config, which ships as UTF-8, UTF-16 or BOM-prefixed UTF-8
/// depending on which tool authored it.
pub(crate) fn decode_xml(bytes: &[u8], label: &str) -> anyhow::Result<String> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let mut values = Vec::new();
        for chunk in bytes[2..].chunks_exact(2) {
            values.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        return String::from_utf16(&values)
            .map_err(|err| anyhow::anyhow!("failed to decode UTF-16LE XML {label}: {err}"));
    }

    if bytes.starts_with(&[0xFE, 0xFF]) {
        let mut values = Vec::new();
        for chunk in bytes[2..].chunks_exact(2) {
            values.push(u16::from_be_bytes([chunk[0], chunk[1]]));
        }
        return String::from_utf16(&values)
            .map_err(|err| anyhow::anyhow!("failed to decode UTF-16BE XML {label}: {err}"));
    }

    let payload = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };

    String::from_utf8(payload.to_vec())
        .map_err(|err| anyhow::anyhow!("failed to decode UTF-8 XML {label}: {err}"))
}
pub(crate) fn build_fomod_selection_map(
    selections: &[FomodSelection],
) -> HashMap<(String, String), Vec<String>> {
    let mut out = HashMap::new();
    for selection in selections {
        out.insert(
            (
                selection.step.to_ascii_lowercase(),
                selection.group.to_ascii_lowercase(),
            ),
            selection.options.clone(),
        );
    }
    out
}

pub(crate) fn find_child_element<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
}

pub(crate) fn child_elements<'a, 'input>(
    node: Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    node.children()
        .filter(move |child| child.is_element() && child.tag_name().name() == name)
}

pub(crate) fn collect_fomod_entries(node: Node<'_, '_>, out: &mut Vec<FomodInstallEntry>) -> anyhow::Result<()> {
    for child in node.children().filter(|child| child.is_element()) {
        let priority = child
            .attribute("priority")
            .unwrap_or("0")
            .parse::<i32>()
            .map_err(|err| anyhow::anyhow!("invalid FOMOD file priority: {err}"))?;
        let source = child
            .attribute("source")
            .ok_or_else(|| anyhow::anyhow!("FOMOD install entry missing source attribute"))?
            .to_string();
        let destination = child.attribute("destination").unwrap_or(&source).to_string();

        match child.tag_name().name() {
            "file" => out.push(FomodInstallEntry::File {
                source,
                destination,
                priority,
            }),
            "folder" => out.push(FomodInstallEntry::Folder {
                source,
                destination,
                priority,
            }),
            other => anyhow::bail!("unsupported FOMOD install entry '{}'; only file/folder are supported", other),
        }
    }
    Ok(())
}

pub(crate) fn fomod_option_type(
    plugin: Node<'_, '_>,
    active_plugins: &HashSet<String>,
    flags: &HashMap<String, String>,
) -> anyhow::Result<FomodOptionType> {
    let Some(type_descriptor) = find_child_element(plugin, "typeDescriptor") else {
        return Ok(FomodOptionType::Optional);
    };

    if let Some(type_node) = find_child_element(type_descriptor, "type") {
        return parse_fomod_option_type(type_node.attribute("name").unwrap_or("Optional"));
    }

    if let Some(dep_type) = find_child_element(type_descriptor, "dependencyType") {
        let default_type = find_child_element(dep_type, "defaultType")
            .and_then(|node| node.attribute("name"))
            .unwrap_or("Optional");
        if let Some(patterns) = find_child_element(dep_type, "patterns") {
            for pattern in child_elements(patterns, "pattern") {
                let matches = if let Some(deps) = find_child_element(pattern, "dependencies") {
                    fomod_dependencies_match(deps, active_plugins, flags)?
                } else {
                    true
                };
                if !matches {
                    continue;
                }

                if let Some(type_node) = find_child_element(pattern, "type") {
                    return parse_fomod_option_type(type_node.attribute("name").unwrap_or(default_type));
                }
            }
        }

        return parse_fomod_option_type(default_type);
    }

    Ok(FomodOptionType::Optional)
}

pub(crate) fn parse_fomod_option_type(name: &str) -> anyhow::Result<FomodOptionType> {
    match name {
        "Required" => Ok(FomodOptionType::Required),
        "Recommended" => Ok(FomodOptionType::Recommended),
        "Optional" => Ok(FomodOptionType::Optional),
        "CouldBeUsable" => Ok(FomodOptionType::CouldBeUsable),
        "NotUsable" => Ok(FomodOptionType::NotUsable),
        other => anyhow::bail!("unsupported FOMOD option type '{other}'"),
    }
}

pub(crate) fn fomod_dependencies_match(
    dependencies: Node<'_, '_>,
    active_plugins: &HashSet<String>,
    flags: &HashMap<String, String>,
) -> anyhow::Result<bool> {
    match dependencies.tag_name().name() {
        "dependencies" => {
            let operator = dependencies.attribute("operator").unwrap_or("And");
            let mut matched = Vec::new();
            for child in dependencies.children().filter(|child| child.is_element()) {
                matched.push(fomod_dependencies_match(child, active_plugins, flags)?);
            }
            Ok(if operator.eq_ignore_ascii_case("Or") {
                matched.into_iter().any(|v| v)
            } else {
                matched.into_iter().all(|v| v)
            })
        }
        "fileDependency" => {
            let file = dependencies
                .attribute("file")
                .ok_or_else(|| anyhow::anyhow!("FOMOD fileDependency missing file attribute"))?
                .to_ascii_lowercase();
            let state = dependencies.attribute("state").unwrap_or("Active");
            let is_active = active_plugins.contains(&file);
            Ok(match state {
                "Active" => is_active,
                "Inactive" => !is_active,
                "Missing" => !is_active,
                other => anyhow::bail!("unsupported FOMOD fileDependency state '{other}'"),
            })
        }
        "flagDependency" => {
            let flag = dependencies
                .attribute("flag")
                .ok_or_else(|| anyhow::anyhow!("FOMOD flagDependency missing flag attribute"))?
                .to_ascii_lowercase();
            let value = dependencies.attribute("value").unwrap_or("On");
            Ok(flags
                .get(&flag)
                .map(|found| found.eq_ignore_ascii_case(value))
                .unwrap_or(false))
        }
        other => anyhow::bail!("unsupported FOMOD dependency node '{other}'"),
    }
}

pub(crate) fn select_fomod_options(
    step_name: &str,
    group_name: &str,
    group_type: &str,
    plugins: &[Node<'_, '_>],
    option_types: &[FomodOptionType],
    selections: &HashMap<(String, String), Vec<String>>,
) -> anyhow::Result<Vec<usize>> {
    let selection_key = (step_name.to_ascii_lowercase(), group_name.to_ascii_lowercase());
    if let Some(wanted) = selections.get(&selection_key) {
        let mut out = Vec::new();
        for desired in wanted {
            let Some((idx, _)) = plugins.iter().enumerate().find(|(_, plugin)| {
                plugin
                    .attribute("name")
                    .map(|name| name.eq_ignore_ascii_case(desired))
                    .unwrap_or(false)
            }) else {
                let available = plugins
                    .iter()
                    .filter_map(|plugin| plugin.attribute("name"))
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::bail!(
                    "FOMOD selection for step '{}' group '{}' references unknown option '{}'. Available options: {}",
                    step_name,
                    group_name,
                    desired,
                    available
                );
            };
            if option_types[idx] == FomodOptionType::NotUsable {
                anyhow::bail!(
                    "FOMOD selection for step '{}' group '{}' chose unusable option '{}'",
                    step_name,
                    group_name,
                    desired
                );
            }
            out.push(idx);
        }

        if group_type == "SelectExactlyOne" && out.len() != 1 {
            anyhow::bail!(
                "FOMOD step '{}' group '{}' requires exactly one selected option",
                step_name,
                group_name
            );
        }
        if group_type == "SelectAtLeastOne" && out.is_empty() {
            anyhow::bail!(
                "FOMOD step '{}' group '{}' requires at least one selected option",
                step_name,
                group_name
            );
        }

        return Ok(out);
    }

    let usable: Vec<usize> = option_types
        .iter()
        .enumerate()
        .filter_map(|(idx, ty)| (*ty != FomodOptionType::NotUsable).then_some(idx))
        .collect();
    let required: Vec<usize> = option_types
        .iter()
        .enumerate()
        .filter_map(|(idx, ty)| (*ty == FomodOptionType::Required).then_some(idx))
        .collect();
    let recommended: Vec<usize> = option_types
        .iter()
        .enumerate()
        .filter_map(|(idx, ty)| (*ty == FomodOptionType::Recommended).then_some(idx))
        .collect();
    let could_be_usable: Vec<usize> = option_types
        .iter()
        .enumerate()
        .filter_map(|(idx, ty)| (*ty == FomodOptionType::CouldBeUsable).then_some(idx))
        .collect();

    let selected = match group_type {
        "SelectAll" => usable,
        "SelectAny" => {
            let mut out = required;
            out.extend(recommended);
            out
        }
        "SelectExactlyOne" | "SelectAtLeastOne" | "SelectAtMostOne" => {
            if let Some(idx) = required
                .first()
                .or_else(|| recommended.first())
                .or_else(|| could_be_usable.first())
                .or_else(|| usable.first())
            {
                vec![*idx]
            } else {
                Vec::new()
            }
        }
        other => anyhow::bail!(
            "unsupported FOMOD option group type '{}' in step '{}' group '{}'",
            other,
            step_name,
            group_name
        ),
    };

    Ok(selected)
}
/// Normalise an installer-declared path to a `/`-separated relative path.
///
/// FOMOD configs are Windows documents: backslashes throughout, and the odd
/// `.\` prefix.
pub(crate) fn normalize_fomod_path(value: &str) -> anyhow::Result<String> {
    let normalised = value.replace('\\', "/");
    Ok(normalize_relative_path(&normalised)?
        .to_string_lossy()
        .replace('\\', "/"))
}

/// As `normalize_fomod_path`, but an empty destination is legal and means the
/// mod root.
pub(crate) fn normalize_fomod_path_allow_empty(value: &str) -> anyhow::Result<String> {
    if value.trim().is_empty() {
        return Ok(String::new());
    }
    normalize_fomod_path(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(entries: &[&str]) -> Vec<String> {
        entries.iter().map(ToString::to_string).collect()
    }

    fn passthrough() -> ArchiveFilters {
        ArchiveFilters::new(&[] as &[String], &[] as &[String]).expect("filters")
    }

    fn file(source: &str, destination: &str, priority: i32) -> FomodInstallEntry {
        FomodInstallEntry::File {
            source: source.to_string(),
            destination: destination.to_string(),
            priority,
        }
    }

    fn folder(source: &str, destination: &str, priority: i32) -> FomodInstallEntry {
        FomodInstallEntry::Folder {
            source: source.to_string(),
            destination: destination.to_string(),
            priority,
        }
    }

    fn plan(
        entries: &[&str],
        content_root: &str,
        install: &[FomodInstallEntry],
    ) -> anyhow::Result<Vec<(String, String)>> {
        let paths = paths(entries);
        let plan = plan_fomod_entries(&paths, content_root, install, &passthrough(), "test.7z")?;
        Ok(plan
            .files
            .into_iter()
            .map(|f| (f.source, f.destination))
            .collect())
    }

    #[test]
    fn a_folder_entry_rebases_onto_its_destination() {
        let files = plan(
            &[
                "fomod/ModuleConfig.xml",
                "00 Core/Textures/Rock.dds",
                "00 Core/Meshes/a.nif",
                "01 Extra/Textures/b.dds",
            ],
            "",
            &[folder("00 Core", "", 0)],
        )
        .expect("plan");

        assert_eq!(
            files,
            [
                ("00 Core/Meshes/a.nif".to_string(), "meshes/a.nif".to_string()),
                (
                    "00 Core/Textures/Rock.dds".to_string(),
                    "textures/Rock.dds".to_string()
                ),
            ],
            "the subpackage name is stripped, directories fold, file names do not"
        );
    }

    #[test]
    fn a_destination_prefix_is_prepended_and_folded() {
        let files = plan(
            &["Options/Loud/Sound/fx/a.wav"],
            "",
            &[folder("Options/Loud", "Sound/FX", 0)],
        )
        .expect("plan");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1, "sound/fx/sound/fx/a.wav");
    }

    #[test]
    fn the_content_root_is_stripped_from_installer_sources() {
        // A FOMOD nested under a wrapper folder: `source` is relative to the
        // folder holding `fomod/`, not to the archive root.
        let files = plan(
            &[
                "AWLS/fomod/ModuleConfig.xml",
                "AWLS/00 Core/meshes/a.nif",
            ],
            "AWLS",
            &[folder("00 Core", "", 0)],
        )
        .expect("plan");

        assert_eq!(files, [("AWLS/00 Core/meshes/a.nif".to_string(), "meshes/a.nif".to_string())]);
    }

    #[test]
    fn higher_priority_entries_replace_what_lower_ones_wrote() {
        let files = plan(
            &["Base/textures/a.dds", "Patch/textures/a.dds"],
            "",
            &[folder("Patch", "textures", 10), folder("Base", "textures", 0)],
        )
        .expect("plan");

        assert_eq!(files.len(), 1, "one destination");
        assert_eq!(
            files[0].0, "Patch/textures/a.dds",
            "the priority-10 entry wins regardless of declaration order"
        );
    }

    #[test]
    fn a_file_entry_can_be_renamed_on_the_way_in() {
        let files = plan(
            &["Optional/DarNified.ini"],
            "",
            &[file("Optional/DarNified.ini", "Menus/DarN.ini", 0)],
        )
        .expect("plan");

        assert_eq!(files, [("Optional/DarNified.ini".to_string(), "menus/DarN.ini".to_string())]);
    }

    #[test]
    fn installer_case_need_not_match_the_archive() {
        // Real configs disagree with their own archives about case constantly;
        // on Windows nobody notices, and on Linux the copy used to fail.
        let files = plan(
            &["00 core/Meshes/A.nif"],
            "",
            &[folder("00 Core", "", 0)],
        )
        .expect("plan");
        assert_eq!(files.len(), 1);

        let files = plan(&["data/thing.esp"], "", &[file("Data/Thing.esp", "Thing.esp", 0)])
            .expect("plan");
        assert_eq!(files, [("data/thing.esp".to_string(), "Thing.esp".to_string())]);
    }

    #[test]
    fn an_entry_naming_nothing_in_the_archive_is_an_error() {
        // Silence here means installing a mod quietly missing a third of its
        // files, which is exactly how a mistyped selection would look.
        let err = plan(&["00 Core/a.nif"], "", &[folder("01 Typo", "", 0)])
            .expect_err("should reject");
        assert!(err.to_string().contains("01 Typo"), "{err}");

        let err = plan(&["a.nif"], "", &[file("missing.nif", "missing.nif", 0)])
            .expect_err("should reject");
        assert!(err.to_string().contains("missing.nif"), "{err}");
    }

    #[test]
    fn excluded_files_are_dropped_without_being_resolved() {
        let paths = paths(&["Docs/readme.txt", "Core/meshes/a.nif"]);
        let filters =
            ArchiveFilters::new(&[] as &[String], &["Docs/**".to_string()]).expect("filters");
        let plan = plan_fomod_entries(
            &paths,
            "",
            &[
                file("Docs/readme.txt", "readme.txt", 0),
                folder("Core", "", 0),
            ],
            &filters,
            "test.7z",
        )
        .expect("plan");

        assert_eq!(plan.len(), 1);
        assert_eq!(plan.files[0].destination, "meshes/a.nif");
    }

    #[test]
    fn the_config_is_found_at_any_depth_with_module_config_preferred() {
        assert_eq!(
            find_fomod_config(&paths(&["fomod/script.xml", "fomod/ModuleConfig.xml"])).as_deref(),
            Some("fomod/ModuleConfig.xml")
        );
        assert_eq!(
            find_fomod_config(&paths(&["AWLS 5.1/FOMOD/moduleconfig.xml"])).as_deref(),
            Some("AWLS 5.1/FOMOD/moduleconfig.xml")
        );
        // A stray XML outside a `fomod/` folder has no content root to imply.
        assert_eq!(find_fomod_config(&paths(&["docs/ModuleConfig.xml"])), None);
        // Two candidates: the shallowest wins, deterministically.
        assert_eq!(
            find_fomod_config(&paths(&[
                "Wrapper/Inner/fomod/ModuleConfig.xml",
                "Wrapper/fomod/ModuleConfig.xml",
            ]))
            .as_deref(),
            Some("Wrapper/fomod/ModuleConfig.xml")
        );
    }

    #[test]
    fn the_content_root_is_the_parent_of_the_fomod_folder() {
        assert_eq!(fomod_content_root("fomod/ModuleConfig.xml"), "");
        assert_eq!(fomod_content_root("AWLS/fomod/ModuleConfig.xml"), "AWLS");
        assert_eq!(fomod_content_root("a/b/fomod/script.xml"), "a/b");
    }
}
