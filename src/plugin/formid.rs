//! FormIDs and the master-list indirection they are relative to.

use std::fmt;

/// A FormID: `(mod_index << 24) | object_index`.
///
/// The mod index is an index into the *owning plugin's* master list, so the
/// same 32-bit value means different things in different plugins. Converting
/// between plugins is the entire job of a merge, so it is never done
/// implicitly -- see [`MasterTable::resolve`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FormId(pub u32);

impl FormId {
    pub const NULL: FormId = FormId(0);

    pub const fn new(mod_index: u8, object_index: u32) -> Self {
        FormId(((mod_index as u32) << 24) | (object_index & 0x00FF_FFFF))
    }

    pub const fn mod_index(self) -> u8 {
        (self.0 >> 24) as u8
    }

    pub const fn object_index(self) -> u32 {
        self.0 & 0x00FF_FFFF
    }

    pub const fn with_mod_index(self, mod_index: u8) -> Self {
        FormId::new(mod_index, self.object_index())
    }

    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Debug for FormId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}

impl fmt::Display for FormId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}

/// A plugin filename compared case-insensitively.
///
/// Oblivion master references are case-insensitive and real load orders mix
/// cases for the same file (`AFK_Weye.esp` vs `afk_weye.esp`), so comparing
/// raw strings silently fails to match masters.
#[derive(Clone)]
pub struct PluginName {
    original: String,
    folded: String,
}

impl PluginName {
    pub fn new(name: impl Into<String>) -> Self {
        let original = name.into();
        let folded = original.to_ascii_lowercase();
        PluginName { original, folded }
    }

    pub fn as_str(&self) -> &str {
        &self.original
    }
}

impl PartialEq for PluginName {
    fn eq(&self, other: &Self) -> bool {
        self.folded == other.folded
    }
}
impl Eq for PluginName {}

impl std::hash::Hash for PluginName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.folded.hash(state);
    }
}

impl fmt::Debug for PluginName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.original, f)
    }
}

impl fmt::Display for PluginName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.original)
    }
}

impl From<&str> for PluginName {
    fn from(value: &str) -> Self {
        PluginName::new(value)
    }
}

/// Where a FormID actually points, independent of any plugin's numbering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// A record defined by one of the plugin's masters.
    Master {
        plugin: PluginName,
        object_index: u32,
    },
    /// A record defined by the plugin itself.
    Own { object_index: u32 },
}

/// The ordered master list of one plugin. Index into it == mod index.
#[derive(Debug, Clone, Default)]
pub struct MasterTable {
    masters: Vec<PluginName>,
}

impl MasterTable {
    pub fn new(masters: Vec<PluginName>) -> Self {
        MasterTable { masters }
    }

    pub fn masters(&self) -> &[PluginName] {
        &self.masters
    }

    pub fn len(&self) -> usize {
        self.masters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.masters.is_empty()
    }

    /// The mod index the plugin uses for its own records: one past its masters.
    pub fn own_mod_index(&self) -> u8 {
        self.masters.len() as u8
    }

    pub fn index_of(&self, name: &PluginName) -> Option<u8> {
        self.masters
            .iter()
            .position(|m| m == name)
            .map(|i| i as u8)
    }

    pub fn get(&self, mod_index: u8) -> Option<&PluginName> {
        self.masters.get(mod_index as usize)
    }

    /// Resolve a FormID to what it actually refers to.
    ///
    /// Returns `None` for a dangling reference: a mod index beyond the master
    /// list and not the plugin's own index. Dirty plugins do contain these, and
    /// callers must decide what to do rather than have a wrong answer invented.
    pub fn resolve(&self, form_id: FormId) -> Option<Origin> {
        let index = form_id.mod_index();
        if index as usize == self.masters.len() {
            return Some(Origin::Own {
                object_index: form_id.object_index(),
            });
        }
        self.masters.get(index as usize).map(|plugin| Origin::Master {
            plugin: plugin.clone(),
            object_index: form_id.object_index(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_rebuilds_form_ids() {
        let f = FormId(0x0102_26B6);
        assert_eq!(f.mod_index(), 0x01);
        assert_eq!(f.object_index(), 0x0226B6);
        assert_eq!(f.with_mod_index(0x03), FormId(0x0302_26B6));
        assert_eq!(FormId::new(0x01, 0x0226B6), f);
        assert!(FormId::NULL.is_null());
    }

    #[test]
    fn object_index_is_masked_to_24_bits() {
        // A caller passing a full FormID as the object index must not corrupt
        // the mod index.
        assert_eq!(FormId::new(2, 0x0100_0801), FormId(0x0200_0801));
    }

    #[test]
    fn plugin_names_compare_case_insensitively() {
        assert_eq!(PluginName::new("AFK_Weye.esp"), PluginName::new("afk_weye.esp"));
        assert_ne!(PluginName::new("a.esp"), PluginName::new("b.esp"));
        // but the original spelling is preserved for output
        assert_eq!(PluginName::new("AFK_Weye.esp").as_str(), "AFK_Weye.esp");
    }

    #[test]
    fn resolves_against_the_master_list() {
        let table = MasterTable::new(vec!["Oblivion.esm".into(), "Knights.esp".into()]);
        assert_eq!(table.own_mod_index(), 2);

        assert_eq!(
            table.resolve(FormId(0x0000_0014)),
            Some(Origin::Master {
                plugin: "Oblivion.esm".into(),
                object_index: 0x14
            })
        );
        assert_eq!(
            table.resolve(FormId(0x0200_0801)),
            Some(Origin::Own {
                object_index: 0x800 + 1
            })
        );
        // mod index past the master list and not our own index == dangling
        assert_eq!(table.resolve(FormId(0x0500_0801)), None);
    }
}
