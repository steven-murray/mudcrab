//! Selecting a subset of a modlist to operate on.
//!
//! A 700-mod list is built section by section, so `download`, `check` and
//! `install` all need the same answer to "is this mod in scope?". The predicate
//! lives here once rather than in each command, because three copies of it
//! would be three chances for `--section` to mean something slightly different
//! depending on which command you ran.

use std::collections::HashSet;

/// Which mods a command should act on.
///
/// An empty filter means the whole modlist, which is what every command did
/// before the flags existed -- so "no flags" and "filter that matches
/// everything" are the same code path, not two.
#[derive(Debug, Clone, Default)]
pub struct ModFilter {
    /// Section names to match, lowercased once here so `matches` does not
    /// re-lowercase them for every mod.
    sections: HashSet<String>,
    /// Mod ids to match exactly. Ids are the mods' directory names, so they are
    /// compared verbatim rather than case-insensitively.
    ids: HashSet<String>,
}

impl ModFilter {
    pub fn new(sections: &[String], ids: &[String]) -> Self {
        Self {
            sections: sections
                .iter()
                .map(|name| name.trim().to_lowercase())
                .collect(),
            ids: ids.iter().map(|id| id.trim().to_string()).collect(),
        }
    }

    /// True when no flags were given and the whole modlist is in scope.
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty() && self.ids.is_empty()
    }

    /// Whether a mod is in scope.
    ///
    /// `--section` matches *any* level of the section path, so
    /// `--section "5 - LOD"` selects both `["5 - LOD"]` and
    /// `["5 - LOD", "Meshes"]`; asking for a section always means the whole
    /// subtree under it. `--section` and `--only` union, so naming one extra
    /// mod alongside a section adds it rather than narrowing to the
    /// intersection (which would always be empty for a mod outside the
    /// section).
    pub fn matches(&self, section: &[String], id: &str) -> bool {
        if self.is_empty() {
            return true;
        }

        if self.ids.contains(id) {
            return true;
        }

        section
            .iter()
            .any(|level| self.sections.contains(&level.trim().to_lowercase()))
    }

    /// Human-readable summary for logs, so a filtered run says what it narrowed
    /// to rather than silently doing a fraction of the work.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if !self.sections.is_empty() {
            let mut sections: Vec<&str> = self.sections.iter().map(String::as_str).collect();
            sections.sort_unstable();
            parts.push(format!("sections [{}]", sections.join(", ")));
        }
        if !self.ids.is_empty() {
            let mut ids: Vec<&str> = self.ids.iter().map(String::as_str).collect();
            ids.sort_unstable();
            parts.push(format!("mods [{}]", ids.join(", ")));
        }

        if parts.is_empty() {
            "everything".to_string()
        } else {
            parts.join(" + ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ModFilter;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn an_empty_filter_matches_everything() {
        let filter = ModFilter::default();
        assert!(filter.is_empty());
        assert!(filter.matches(&[], "anything"));
        assert!(filter.matches(&strings(&["5 - LOD"]), "anything"));
    }

    #[test]
    fn section_matching_is_case_insensitive_and_matches_any_level() {
        let filter = ModFilter::new(&strings(&["5 - lod"]), &[]);

        assert!(filter.matches(&strings(&["5 - LOD"]), "a"));
        assert!(filter.matches(&strings(&["5 - LOD", "Meshes"]), "b"));
        // A nested level counts too: the path is searched, not just its root.
        assert!(filter.matches(&strings(&["Graphics", "5 - LOD"]), "c"));
        assert!(!filter.matches(&strings(&["6 - Weather"]), "d"));
        assert!(!filter.matches(&[], "e"));
    }

    #[test]
    fn only_matches_the_exact_id_and_unions_with_section() {
        let filter = ModFilter::new(&strings(&["B"]), &strings(&["Loose Mod"]));

        assert!(filter.matches(&strings(&["A"]), "Loose Mod"));
        assert!(filter.matches(&strings(&["B"]), "Something Else"));
        assert!(!filter.matches(&strings(&["A"]), "loose mod"));
        assert!(!filter.matches(&strings(&["A"]), "Other"));
    }
}
