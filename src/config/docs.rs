//! Prose and pictures for `export`: what a reader of the guide needs that an
//! install does not.
//!
//! Docs sit in three places -- the guide as a whole (`[docs]`), each section
//! (`[sections."<name>"]`, the name as MO2 shows the separator) and each mod
//! (`docs` in its `[[mods]]` table) -- either inline in the modlist or in a
//! separate file named by `docs_file`, so a long list need not bury its install
//! data in prose. The two are merged at load time; something documented in
//! both places is an error rather than a silent preference.
//!
//! Text fields are Markdown. Exporters convert it to whatever their target
//! needs (BBCode for Nexus, HTML for a site), so a list's author writes it once.
//! Keys beyond the known ones are kept and handed to templates as they are:
//! that is how an author adds something no built-in exporter knows about.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Documentation for the guide, a section or a mod.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Docs {
    /// Display title, where the id or section name is not what a reader
    /// should see.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// One line: what this is, or why it is in the list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Longer prose: instructions, caveats, the author's reasoning. Markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Prose that belongs after the contents rather than before them: for the
    /// guide, closing notes and credits; for a section, steps to take once its
    /// mods are installed. Markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<DocImage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<DocLink>,
    /// Anything else, passed to templates untouched.
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Docs {
    pub fn is_empty(&self) -> bool {
        *self == Docs::default()
    }
}

/// A picture. A local `path` suits an exporter that writes a site and copies
/// it in; a `url` suits one whose output is pasted somewhere that cannot hold
/// files, such as a Nexus page. Give both and each exporter takes what it can.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DocImage {
    /// Relative to the modlist file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DocLink {
    pub label: String,
    pub url: String,
}

/// The contents of a `docs_file`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocsFile {
    #[serde(default)]
    pub docs: Option<Docs>,
    #[serde(default)]
    pub sections: IndexMap<String, Docs>,
    /// Keyed by mod id.
    #[serde(default)]
    pub mods: IndexMap<String, Docs>,
}

impl DocsFile {
    pub fn load(path: &Path) -> anyhow::Result<DocsFile> {
        let raw = std::fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("failed to read docs file {}: {err}", path.display()))?;
        toml::from_str(&raw)
            .map_err(|err| anyhow::anyhow!("failed to parse docs file {}: {err}", path.display()))
    }
}
