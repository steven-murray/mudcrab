//! The steps from a modlist to an installed game -- `compile`, `query`, and the
//! plan every later step reads -- as a front end calls them. The CLI commands
//! are these plus argument parsing and terminal output; a GUI calls the same.
//! `check`, `download` and `install` take a plan from [`read_plan`] and run as
//! `config::check::check_all`, `config::download::download_all` and
//! `config::install::install_all`.

use crate::config::schema::{CompiledArchive, CompiledModlist, PersonalizedMod, PersonalizedPlan};
use std::collections::HashSet;
use std::path::Path;

/// A compiled list, and the validator's warnings about its source.
pub struct Compiled {
    pub modlist: CompiledModlist,
    pub warnings: Vec<String>,
}

/// Load, validate and compile the modlist at `input`. With `strict`, a
/// warning is an error.
pub fn compile(input: &Path, strict: bool) -> anyhow::Result<Compiled> {
    let source = crate::config::loader::load_modlist(input)?;
    let warnings = crate::config::validator::validate(&source)?;
    if strict && !warnings.is_empty() {
        anyhow::bail!(
            "{} warning(s), and --strict makes them errors:\n{}",
            warnings.len(),
            warnings.join("\n")
        );
    }
    Ok(Compiled {
        modlist: crate::config::compiler::compile(source)?,
        warnings,
    })
}

/// As [`compile`], for `text` read as the modlist at `path`: for a list being
/// edited, before it is written.
pub fn compile_text(text: &str, path: &Path) -> anyhow::Result<Compiled> {
    let source = crate::config::loader::parse_modlist(text, path)?;
    let warnings = crate::config::validator::validate(&source)?;
    Ok(Compiled {
        modlist: crate::config::compiler::compile(source)?,
        warnings,
    })
}

/// The compiled list `compile` wrote to `path`.
pub fn read_compiled(path: &Path) -> anyhow::Result<CompiledModlist> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&raw).map_err(|err| {
        anyhow::anyhow!(
            "failed to parse compiled artifact {}: {err}",
            path.display()
        )
    })
}

/// The plan `query` wrote to `path`.
pub fn read_plan(path: &Path) -> anyhow::Result<PersonalizedPlan> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&raw).map_err(|err| {
        anyhow::anyhow!(
            "failed to parse personalized plan {}: {err}",
            path.display()
        )
    })
}

/// Write `value` to `path` as pretty JSON, making its folder if need be.
pub fn write_json(path: &Path, value: &impl serde::Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| {
            anyhow::anyhow!(
                "failed to create output directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let data = serde_json::to_string_pretty(value)
        .map_err(|err| anyhow::anyhow!("failed to serialize {}: {err}", path.display()))?;
    std::fs::write(path, data)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
}

/// One of a mod's archives as a plan would have it.
pub struct PlannedArchive {
    pub entry: PersonalizedMod,
    pub archive: CompiledArchive,
    /// The plan's load order, lowercased: what a layout needs to choose a
    /// FOMOD's plugin-dependent files.
    pub active_plugins: HashSet<String>,
}

/// Mod `id`'s `number`th archive (counting from 1) in `text`, read as the
/// modlist at `path`, compiled and planned with `overrides` answering its
/// questions (the rest take their defaults). Planned for writing the list
/// ([`crate::config::query::build_authoring_plan`]): a mod the answers leave
/// out is planned as it would install if it were in.
pub fn planned_archive(
    text: &str,
    path: &Path,
    id: &str,
    number: usize,
    overrides: &crate::config::query::Overrides,
) -> anyhow::Result<PlannedArchive> {
    let source = crate::config::loader::parse_modlist(text, path)?;
    let spec = source
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("the list has no mod with id '{id}'"))?
        .archives
        .get(number.wrapping_sub(1))
        .ok_or_else(|| anyhow::anyhow!("mod '{id}' has no archive {number}"))?
        .clone();
    let compiled = compile_text(text, path)?;
    let planned = crate::config::query::build_authoring_plan(&compiled.modlist, overrides)?;
    let plan = planned.plan;
    let entry = plan
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| {
            let why = planned
                .notes
                .iter()
                .find(|note| note.contains(&format!("mod {id}:")))
                .cloned()
                .unwrap_or_else(|| "it cannot be planned with these answers".to_string());
            anyhow::anyhow!("'{id}' cannot be planned: {why}")
        })?
        .clone();
    // Every archive, as it would install: one the answers leave out too.
    let same = |a: &CompiledArchive| {
        a.path == spec.path && (spec.file_name.is_none() || a.file_name == spec.file_name)
    };
    let archive = entry
        .archives
        .iter()
        .find(|a| same(a))
        .cloned()
        .or_else(|| {
            planned
                .every_archive
                .get(id)?
                .iter()
                .flatten()
                .find(|a| same(a))
                .cloned()
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "with these answers, this archive would install nothing: every BAIN \
                 subpackage's `if` leaves it out"
            )
        })?;
    Ok(PlannedArchive {
        entry,
        archive,
        active_plugins: plan
            .plugins
            .iter()
            .map(|p| p.to_ascii_lowercase())
            .collect(),
    })
}
