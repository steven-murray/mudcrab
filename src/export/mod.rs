//! `export`: a modlist as a guide a person can read.
//!
//! Every exporter reads the same [`Document`]: the modlist arranged as the
//! reader meets it -- sections nesting sections and mods, in install order --
//! with its prose attached and the install data also put into words (see
//! [`describe`]). The `json` exporter writes this document as it is, which is
//! the contract for anyone rendering a list with their own tools; the template
//! exporters render it.
//!
//! Built from the source modlist rather than the compiled one, because docs are
//! for readers and have no business in an install artifact. With a plan, the
//! document describes that one build: only the mods it selected, conditions
//! already resolved, and the answers that produced it.

pub mod builtin;
pub mod describe;
pub mod exporter;
pub mod markup;

use crate::config::condition::Condition;
use crate::config::docs::Docs;
use crate::config::schema::{
    CompiledArchive, GuideProvenance, InputType, InputValue, ModAction, ModEntry, ModType,
    PersonalizedPlan, SourceModlist,
};
use indexmap::IndexMap;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Bumped when a field changes meaning or goes away; adding one does not.
pub const DOCUMENT_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
pub struct Document {
    pub document_version: u32,
    pub generator: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guide: Option<GuideProvenance>,
    pub docs: Docs,
    pub inputs: Vec<InputDoc>,
    /// Present when the document describes one plan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanDoc>,
    /// Tools to set up before installing any mod: Mod Organizer 2 plugins.
    /// Not part of the mod list, so not in `entries`.
    pub tools: Vec<ModDoc>,
    /// The game's own INI settings the list makes, in words.
    pub game_settings: Vec<StepDoc>,
    /// Top level of the tree: sections, and any mod with no section.
    pub entries: Vec<Entry>,
    pub load_order: Vec<PluginDoc>,
    pub mod_count: usize,
    /// Every kind of thing the guide asks of its reader, in the order first
    /// met: instruction and step kinds, plus `optional` (a mod with a
    /// condition), `manual` (an archive to fetch by hand) and `merge`. An
    /// exporter that marks kinds with symbols prints a key from this, listing
    /// only what the guide uses.
    pub kinds: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct InputDoc {
    pub id: String,
    #[serde(rename = "type")]
    pub input_type: InputType,
    pub query: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<InputValue>,
    /// The plan's answer, when exporting a plan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<InputValue>,
}

#[derive(Debug, Serialize)]
pub struct PlanDoc {
    pub responses: HashMap<String, InputValue>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Entry {
    Section(Box<Section>),
    Mod(Box<ModDoc>),
}

#[derive(Debug, Serialize)]
pub struct Section {
    /// As MO2 names the separator: the path joined with " - ".
    pub name: String,
    /// What to show: `docs.title`, else the last level of the path.
    pub title: String,
    pub path: Vec<String>,
    /// 1 for a top-level section.
    pub depth: usize,
    /// Unique in the document; for links and HTML ids.
    pub anchor: String,
    pub docs: Docs,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Serialize)]
pub struct ModDoc {
    pub id: String,
    pub title: String,
    pub anchor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_type: Option<ModType>,
    /// The Nexus pages its archives come from, each once, in archive order.
    /// Usually one; a mod built from several pages' files has several.
    pub pages: Vec<NexusDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionDoc>,
    pub docs: Docs,
    pub archives: Vec<ArchiveDoc>,
    pub plugins: Vec<String>,
    pub steps: Vec<StepDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge: Option<MergeDoc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NexusDoc {
    pub game: String,
    pub mod_id: String,
    pub mod_url: String,
}

#[derive(Debug, Serialize)]
pub struct ConditionDoc {
    /// As written in the modlist.
    pub expr: String,
    /// As a reader would say it, e.g. "you answer yes to “Install X?”".
    pub text: String,
    /// The question itself, when the condition is just "yes to this
    /// question": a manual guide can then say "Optional: <question>".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
}

/// Where to find the archives, so the document can name the files a pattern
/// means and leave out instructions MO2 makes unnecessary. Optional: without
/// it, patterns are put into words instead.
pub struct Archives<'a> {
    pub downloads: &'a std::path::Path,
    pub search_paths: &'a [std::path::PathBuf],
}

#[derive(Debug, Serialize)]
pub struct ArchiveDoc {
    /// `nexus`, `manual`, `url`, or `build` for one assembled from layers.
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nexus: Option<NexusFileDoc>,
    /// Where to download it, when that is a plain URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionDoc>,
    /// Installer choices and the like, in words; empty to install it as is.
    pub instructions: Vec<describe::Instruction>,
    /// The archive entry as the modlist has it, for templates that want the
    /// details rather than the sentences.
    pub raw: CompiledArchive,
}

#[derive(Debug, Serialize)]
pub struct NexusFileDoc {
    pub game: String,
    pub mod_id: String,
    pub file_id: String,
    pub mod_url: String,
    /// The mod's files tab, scrolled to this file.
    pub file_url: String,
}

#[derive(Debug, Serialize)]
pub struct StepDoc {
    /// The action's name in the modlist, e.g. `ini_set`.
    pub action: String,
    /// What sort of step it is, for symbols; see [`describe::Instruction`].
    pub kind: String,
    pub text: String,
    /// Literal text to go with it: an INI block, a file's contents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionDoc>,
    pub raw: ModAction,
}

#[derive(Debug, Serialize)]
pub struct MergeDoc {
    pub output: String,
    pub hide_sources: bool,
    pub sources: Vec<MergeSourceDoc>,
}

#[derive(Debug, Serialize)]
pub struct MergeSourceDoc {
    #[serde(rename = "mod")]
    pub mod_id: String,
    pub plugin: String,
}

#[derive(Debug, Serialize)]
pub struct PluginDoc {
    pub name: String,
    /// False for a plugin that keeps its slot but is not ticked.
    pub active: bool,
    /// The mod that ships it, or whose merge writes it, when the list says.
    #[serde(rename = "mod", skip_serializing_if = "Option::is_none")]
    pub mod_id: Option<String>,
}

/// Arrange `modlist` (and, if given, the one build `plan` describes) as a
/// document.
pub fn build_document(
    modlist: &SourceModlist,
    plan: Option<&PersonalizedPlan>,
    archives: Option<&Archives<'_>>,
) -> anyhow::Result<Document> {
    let mut tools = Vec::new();
    let planned: Option<HashMap<&str, &crate::config::schema::PersonalizedMod>> =
        plan.map(|plan| plan.mods.iter().map(|m| (m.id.as_str(), m)).collect());

    let mut anchors = Anchors::default();
    let mut root: Vec<Entry> = Vec::new();
    // Open sections, outermost first; each is closed into its parent when the
    // path moves on.
    let mut open: Vec<Section> = Vec::new();
    let mut mod_count = 0;

    for entry in &modlist.mods {
        let planned_mod = match &planned {
            Some(planned) => match planned.get(entry.id.as_str()) {
                Some(found) => Some(*found),
                None => continue,
            },
            None => None,
        };

        if entry.mod_type == Some(ModType::Mo2Plugin) {
            tools.push(mod_doc(
                modlist,
                entry,
                planned_mod,
                &mut anchors,
                archives,
            )?);
            continue;
        }

        let common = open
            .iter()
            .zip(&entry.section)
            .take_while(|(section, level)| section.path.last() == Some(*level))
            .count();
        while open.len() > common {
            close_section(&mut open, &mut root);
        }
        for depth in open.len()..entry.section.len() {
            let path = entry.section[..=depth].to_vec();
            let name = path.join(" - ");
            let docs = modlist.sections.get(&name).cloned().unwrap_or_default();
            let title = docs.title.clone().unwrap_or_else(|| path[depth].clone());
            open.push(Section {
                anchor: anchors.make(&name),
                name,
                title,
                path,
                depth: depth + 1,
                docs,
                entries: Vec::new(),
            });
        }

        let doc = mod_doc(modlist, entry, planned_mod, &mut anchors, archives)?;
        mod_count += 1;
        match open.last_mut() {
            Some(section) => section.entries.push(Entry::Mod(Box::new(doc))),
            None => root.push(Entry::Mod(Box::new(doc))),
        }
    }
    while !open.is_empty() {
        close_section(&mut open, &mut root);
    }

    let game_settings: Vec<StepDoc> =
        crate::config::compiler::compile_top_level_actions(&modlist.ini)
            .into_iter()
            .map(|action| {
                let step = describe::action(&action);
                StepDoc {
                    action: action.name().to_string(),
                    kind: step.kind.to_string(),
                    text: step.text,
                    code: step.code,
                    condition: None,
                    raw: action,
                }
            })
            .collect();
    let mut all_kinds: Vec<String> = tools.iter().flat_map(mod_kinds).collect();
    all_kinds.extend(game_settings.iter().map(|s| s.kind.clone()));
    all_kinds.extend(collect_kinds(&root));
    let kinds = kinds_used(&all_kinds);
    Ok(Document {
        document_version: DOCUMENT_VERSION,
        generator: format!("mudcrab {}", env!("CARGO_PKG_VERSION")),
        name: modlist.name.clone(),
        guide: modlist.guide.clone(),
        docs: modlist.docs.clone(),
        inputs: modlist
            .inputs
            .iter()
            .map(|(id, spec)| InputDoc {
                id: id.clone(),
                input_type: spec.input_type,
                query: spec.query.clone(),
                choices: spec.choices.clone(),
                default: spec.default.clone(),
                answer: plan.and_then(|plan| plan.responses.get(id).cloned()),
            })
            .collect(),
        plan: plan.map(|plan| PlanDoc {
            responses: plan.responses.clone(),
        }),
        tools,
        game_settings,
        entries: root,
        load_order: load_order(modlist, plan),
        mod_count,
        kinds,
    })
}

fn collect_kinds(entries: &[Entry]) -> Vec<String> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            Entry::Section(section) => out.extend(collect_kinds(&section.entries)),
            Entry::Mod(m) => out.extend(mod_kinds(m)),
        }
    }
    out
}

fn mod_kinds(m: &ModDoc) -> Vec<String> {
    let mut out = Vec::new();
    if m.condition.is_some() {
        out.push("optional".to_string());
    }
    if m.archives.len() > 1 {
        out.push("combine".to_string());
    }
    for archive in &m.archives {
        if archive.source == "manual" {
            out.push("manual".to_string());
        }
        out.extend(archive.instructions.iter().map(|i| i.kind.to_string()));
    }
    if m.merge.is_some() {
        out.push("merge".to_string());
    }
    out.extend(m.steps.iter().map(|step| step.kind.clone()));
    out
}

fn kinds_used(all: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for kind in all {
        if !seen.contains(kind) {
            seen.push(kind.clone());
        }
    }
    seen
}

fn close_section(open: &mut Vec<Section>, root: &mut Vec<Entry>) {
    let Some(section) = open.pop() else { return };
    match open.last_mut() {
        Some(parent) => parent.entries.push(Entry::Section(Box::new(section))),
        None => root.push(Entry::Section(Box::new(section))),
    }
}

fn condition_doc(expr: &str, modlist: &SourceModlist) -> anyhow::Result<ConditionDoc> {
    let condition = Condition::parse(expr)?;
    let question = match &condition {
        Condition::Input {
            key,
            test: crate::config::condition::InputTest::Truthy,
        } => modlist
            .inputs
            .get(key)
            .filter(|spec| spec.input_type == InputType::Bool)
            .map(|spec| spec.query.clone()),
        _ => None,
    };
    Ok(ConditionDoc {
        expr: expr.to_string(),
        text: describe::condition(&condition, &modlist.inputs),
        question,
    })
}

fn mod_doc(
    modlist: &SourceModlist,
    entry: &ModEntry,
    planned: Option<&crate::config::schema::PersonalizedMod>,
    anchors: &mut Anchors,
    lookup: Option<&Archives<'_>>,
) -> anyhow::Result<ModDoc> {
    let context = |err: anyhow::Error| anyhow::anyhow!("mod {}: {err:#}", entry.id);

    // A plan has already resolved every condition and dropped what did not
    // apply; without one, every row is shown with its condition in words.
    let archives: Vec<(CompiledArchive, Option<String>)> = match planned {
        Some(planned) => planned.archives.iter().map(|a| (a.clone(), None)).collect(),
        None => entry
            .archives
            .iter()
            .map(|archive| {
                let compiled = crate::config::compiler::compile_archive(&entry.id, archive)?;
                Ok((compiled, archive.condition.clone()))
            })
            .collect::<anyhow::Result<_>>()
            .map_err(context)?,
    };
    let actions: Vec<(ModAction, Option<String>)> = match planned {
        Some(planned) => planned.actions.iter().map(|a| (a.clone(), None)).collect(),
        None => entry
            .actions
            .iter()
            .map(|a| (a.action.clone(), a.condition.clone()))
            .collect(),
    };

    let mut archive_docs = Vec::new();
    for (archive, condition) in archives {
        archive_docs.push(ArchiveDoc {
            source: archive_source(&archive),
            path: archive.path.clone(),
            file_name: archive.file_name.clone(),
            nexus: archive.path.as_deref().and_then(nexus_file),
            url: archive
                .path
                .as_deref()
                .filter(|p| p.starts_with("http://") || p.starts_with("https://"))
                .map(str::to_string),
            condition: condition
                .as_deref()
                .map(|c| condition_doc(c, modlist))
                .transpose()
                .map_err(context)?,
            instructions: if entry.mod_type == Some(ModType::Mo2Plugin) {
                vec![describe::Instruction {
                    kind: "mo2_plugin",
                    text: format!(
                        "Extract {} into Mod Organizer 2's `plugins/` folder (MO2 closed).",
                        archive
                            .data_folder
                            .as_deref()
                            .map(|f| format!("`{f}`"))
                            .unwrap_or_else(|| "the archive".to_string())
                    ),
                }]
            } else {
                let listing = lookup.and_then(|lookup| list_archive(lookup, &archive));
                describe::archive_instructions(&archive, listing.as_deref())
            },
            raw: archive,
        });
    }

    let mut steps = Vec::new();
    for (action, condition) in actions {
        let step = describe::action(&action);
        steps.push(StepDoc {
            action: action.name().to_string(),
            kind: step.kind.to_string(),
            text: step.text,
            code: step.code,
            condition: condition
                .as_deref()
                .map(|c| condition_doc(c, modlist))
                .transpose()
                .map_err(context)?,
            raw: action,
        });
    }

    Ok(ModDoc {
        anchor: anchors.make(&entry.id),
        title: entry.docs.title.clone().unwrap_or_else(|| entry.id.clone()),
        id: entry.id.clone(),
        mod_type: entry.mod_type,
        pages: {
            let mut pages: Vec<NexusDoc> = Vec::new();
            for n in archive_docs.iter().filter_map(|a| a.nexus.as_ref()) {
                if !pages.iter().any(|p| p.mod_url == n.mod_url) {
                    pages.push(NexusDoc {
                        game: n.game.clone(),
                        mod_id: n.mod_id.clone(),
                        mod_url: n.mod_url.clone(),
                    });
                }
            }
            pages
        },
        condition: match planned {
            Some(_) => None,
            None => entry
                .condition
                .as_deref()
                .map(|c| condition_doc(c, modlist))
                .transpose()
                .map_err(context)?,
        },
        docs: entry.docs.clone(),
        archives: archive_docs,
        plugins: entry.plugins.clone(),
        steps,
        merge: entry.merge.as_ref().map(|merge| MergeDoc {
            output: merge.output.clone(),
            hide_sources: merge.hide_sources,
            sources: merge
                .sources
                .iter()
                .map(|s| MergeSourceDoc {
                    mod_id: s.mod_id.clone(),
                    plugin: s.plugin.clone(),
                })
                .collect(),
        }),
    })
}

/// The archive's entries, if it can be found and read; `None` otherwise,
/// which only costs the reader some precision.
fn list_archive(lookup: &Archives<'_>, archive: &CompiledArchive) -> Option<Vec<String>> {
    if archive.inner_archive.is_some() {
        return None;
    }
    let source = archive.path.as_deref()?;
    let reference =
        crate::config::archive_store::ArchiveRef::new(source, archive.file_name.as_deref());
    let path =
        crate::config::archive_store::find(lookup.downloads, lookup.search_paths, &reference)?;
    crate::archive::list_archive_paths(&path).ok()
}

fn archive_source(archive: &CompiledArchive) -> String {
    match archive.path.as_deref() {
        None => "build".to_string(),
        Some(path) if path.starts_with("http://") || path.starts_with("https://") => {
            "url".to_string()
        }
        Some(path) => path
            .split_once(':')
            .map(|(scheme, _)| scheme.to_string())
            .unwrap_or_else(|| "file".to_string()),
    }
}

/// `nexus:<game>/<mod>/<file>` as links.
fn nexus_file(path: &str) -> Option<NexusFileDoc> {
    let rest = path.strip_prefix("nexus:")?;
    let mut parts = rest.split('/');
    let (game, mod_id, file_id) = (parts.next()?, parts.next()?, parts.next()?);
    let mod_url = format!("https://www.nexusmods.com/{game}/mods/{mod_id}");
    Some(NexusFileDoc {
        file_url: format!("{mod_url}?tab=files&file_id={file_id}"),
        mod_url,
        game: game.to_string(),
        mod_id: mod_id.to_string(),
        file_id: file_id.to_string(),
    })
}

fn load_order(modlist: &SourceModlist, plan: Option<&PersonalizedPlan>) -> Vec<PluginDoc> {
    let mut owner: HashMap<String, String> = HashMap::new();
    for entry in &modlist.mods {
        for plugin in &entry.plugins {
            owner
                .entry(plugin.to_ascii_lowercase())
                .or_insert_with(|| entry.id.clone());
        }
        if let Some(merge) = &entry.merge {
            owner
                .entry(merge.output.to_ascii_lowercase())
                .or_insert_with(|| entry.id.clone());
        }
    }
    let (plugins, inactive) = match plan {
        Some(plan) => (&plan.plugins, &plan.inactive_plugins),
        None => (&modlist.plugins, &modlist.inactive_plugins),
    };
    let inactive: HashSet<String> = inactive.iter().map(|p| p.to_ascii_lowercase()).collect();
    plugins
        .iter()
        .map(|name| PluginDoc {
            name: name.clone(),
            active: !inactive.contains(&name.to_ascii_lowercase()),
            mod_id: owner.get(&name.to_ascii_lowercase()).cloned(),
        })
        .collect()
}

/// Ids for links: lowercase words joined by hyphens, numbered when two
/// things would share one.
#[derive(Default)]
struct Anchors {
    used: IndexMap<String, usize>,
}

impl Anchors {
    fn make(&mut self, text: &str) -> String {
        let mut slug = String::new();
        for c in text.chars() {
            if c.is_alphanumeric() {
                slug.extend(c.to_lowercase());
            } else if !slug.ends_with('-') && !slug.is_empty() {
                slug.push('-');
            }
        }
        let slug = slug.trim_end_matches('-').to_string();
        let slug = if slug.is_empty() {
            "item".to_string()
        } else {
            slug
        };
        let count = self.used.entry(slug.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            slug
        } else {
            format!("{slug}-{count}")
        }
    }
}
