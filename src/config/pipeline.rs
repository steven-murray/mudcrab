//! The steps from a modlist to an installed game -- `compile`, `query`, and the
//! plan every later step reads -- as a front end calls them. The CLI commands
//! are these plus argument parsing and terminal output; a GUI calls the same.
//! `check`, `download` and `install` take a plan from [`read_plan`] and run as
//! `config::check::check_all`, `config::download::download_all` and
//! `config::install::install_all`.

use crate::config::schema::{CompiledModlist, PersonalizedPlan};
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
