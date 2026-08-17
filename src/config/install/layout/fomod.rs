//! FOMOD installer support: parse ModuleConfig.xml and apply the selected options.

use super::{destination_for, with_staged_archive};
use crate::archive::ArchiveFilters;
use crate::config::schema::{CompiledArchive, FomodSelection};
use crate::util::fs::{
    copy_filtered_tree_folded, lowercase_dir_components, lowercase_path, normalize_relative_path,
};
use roxmltree::{Document, Node};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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

pub(crate) fn find_fomod_config(staging_dir: &Path) -> anyhow::Result<PathBuf> {
    let mut module_config = None;
    let mut script_xml = None;
    find_fomod_config_recursive(staging_dir, &mut module_config, &mut script_xml)?;

    if let Some(path) = module_config {
        return Ok(path);
    }
    if let Some(path) = script_xml {
        return Ok(path);
    }

    anyhow::bail!(
        "FOMOD archive {} does not contain fomod/ModuleConfig.xml or fomod/script.xml",
        staging_dir.display()
    )
}

pub(crate) fn apply_fomod_from_staging(
    staging_dir: &Path,
    destination_root: &Path,
    archive: &CompiledArchive,
    filters: &ArchiveFilters,
    active_plugins: &HashSet<String>,
) -> anyhow::Result<usize> {
    let module_config = find_fomod_config(staging_dir)?;
    let xml = read_xml_text(&module_config)?;
    let doc = Document::parse(&xml).map_err(|err| {
        anyhow::anyhow!("failed to parse FOMOD config {}: {err}", module_config.display())
    })?;
    let content_root = module_config
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow::anyhow!("invalid FOMOD structure in {}", staging_dir.display()))?;

    let root = doc.root_element();
    let selection_map = build_fomod_selection_map(&archive.fomod_selections);
    let mut flags = HashMap::<String, String>::new();
    let mut entries = Vec::<FomodInstallEntry>::new();

    if let Some(required) = find_child_element(root, "requiredInstallFiles") {
        collect_fomod_entries(required, &mut entries)?;
    }

    if let Some(steps) = find_child_element(root, "installSteps") {
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
    }

    apply_fomod_entries(content_root, destination_root, &entries, filters)
}

pub(crate) fn find_fomod_config_recursive(
    current: &Path,
    module_config: &mut Option<PathBuf>,
    script_xml: &mut Option<PathBuf>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to iterate {}: {err}", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("failed to read file type for {}: {err}", path.display()))?;

        if file_type.is_dir() {
            find_fomod_config_recursive(&path, module_config, script_xml)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case("ModuleConfig.xml") {
            *module_config = Some(path);
        } else if name.eq_ignore_ascii_case("script.xml") {
            *script_xml = Some(path);
        }
    }

    Ok(())
}

pub(crate) fn read_xml_text(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;

    if bytes.starts_with(&[0xFF, 0xFE]) {
        let mut values = Vec::new();
        for chunk in bytes[2..].chunks_exact(2) {
            values.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        return String::from_utf16(&values)
            .map_err(|err| anyhow::anyhow!("failed to decode UTF-16LE XML {}: {err}", path.display()));
    }

    if bytes.starts_with(&[0xFE, 0xFF]) {
        let mut values = Vec::new();
        for chunk in bytes[2..].chunks_exact(2) {
            values.push(u16::from_be_bytes([chunk[0], chunk[1]]));
        }
        return String::from_utf16(&values)
            .map_err(|err| anyhow::anyhow!("failed to decode UTF-16BE XML {}: {err}", path.display()));
    }

    let payload = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        &bytes[..]
    };

    String::from_utf8(payload.to_vec())
        .map_err(|err| anyhow::anyhow!("failed to decode UTF-8 XML {}: {err}", path.display()))
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

pub(crate) fn apply_fomod_entries(
    content_root: &Path,
    destination_root: &Path,
    entries: &[FomodInstallEntry],
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|entry| match entry {
        FomodInstallEntry::File { priority, .. } => *priority,
        FomodInstallEntry::Folder { priority, .. } => *priority,
    });

    let mut copied = 0usize;
    for entry in &sorted {
        match entry {
            FomodInstallEntry::File {
                source,
                destination,
                ..
            } => {
                let rel_source = normalize_fomod_relative_path(source)?;
                let rel_source_norm = rel_source.to_string_lossy().replace('\\', "/");
                if !filters.should_extract(&rel_source_norm) {
                    continue;
                }

                let source_path = content_root.join(&rel_source);
                let rel_dest = normalize_fomod_relative_path(destination)?;
                let destination_path = destination_root.join(lowercase_dir_components(&rel_dest));
                copy_fomod_file(&source_path, &destination_path)?;
                copied += 1;
            }
            FomodInstallEntry::Folder {
                source,
                destination,
                ..
            } => {
                let source_rel = normalize_fomod_relative_path(source)?;
                let source_root = content_root.join(&source_rel);
                let destination_rel = normalize_fomod_relative_path_allow_empty(destination)?;
                let destination = destination_root.join(lowercase_path(&destination_rel));
                copied += copy_filtered_tree_folded(&source_root, &destination, filters)?;
            }
        }
    }

    Ok(copied)
}

pub(crate) fn copy_fomod_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
    }

    std::fs::copy(source, destination).map_err(|err| {
        anyhow::anyhow!(
            "failed to copy FOMOD file {} to {}: {err}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(())
}

pub(crate) fn normalize_fomod_relative_path(value: &str) -> anyhow::Result<PathBuf> {
    let normalised = value.replace('\\', "/");
    normalize_relative_path(&normalised)
}

pub(crate) fn normalize_fomod_relative_path_allow_empty(value: &str) -> anyhow::Result<PathBuf> {
    if value.trim().is_empty() {
        return Ok(PathBuf::new());
    }
    normalize_fomod_relative_path(value)
}

