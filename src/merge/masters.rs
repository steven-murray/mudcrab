//! Build the merged plugin's master list.

use crate::plugin::{MasterTable, Plugin, PluginName};

#[derive(Debug, thiserror::Error)]
pub enum MasterError {
    #[error(
        "master {master} (required by {required_by}) is not in the load order.\n\
         Add it to the modlist's `plugins` list, or remove {required_by} from the merge."
    )]
    NotInLoadOrder {
        master: PluginName,
        required_by: PluginName,
    },

    #[error(
        "merged plugin would need {count} masters, but a TES4 plugin can address at most 255"
    )]
    TooManyMasters { count: usize },
}

/// Compute the master list for a merge.
///
/// 1. union every source plugin's masters
/// 2. add any source plugin that another source masters (`UFM Consistency
///    Patch.esp` masters several of the forts it patches)
/// 3. subtract the merged plugins themselves -- they cease to exist, which is
///    the "Clobbering merge masters / Removing master X" step in zMerge's log
/// 4. order by the game load order
///
/// A master missing from the load order is an error rather than a guess:
/// appending it in an arbitrary position silently changes which record wins.
pub fn build(
    sources: &[(PluginName, Plugin)],
    load_order: &[PluginName],
) -> Result<MasterTable, MasterError> {
    let merged: Vec<&PluginName> = sources.iter().map(|(name, _)| name).collect();
    let is_merged = |name: &PluginName| merged.contains(&name);

    let mut required: Vec<PluginName> = Vec::new();
    let push = |name: &PluginName, required: &mut Vec<PluginName>| {
        if !is_merged(name) && !required.contains(name) {
            required.push(name.clone());
        }
    };

    for (source_name, plugin) in sources {
        for master in plugin.masters.masters() {
            push(master, &mut required);
        }
        // A source that another source masters is itself merged away, so it
        // must not appear; `push` already skips those. This loop exists for
        // the symmetric case: a master that happens to be a merge source is
        // dropped rather than kept.
        let _ = source_name;
    }

    // Order by load order, erroring on anything not placed.
    let mut ordered: Vec<PluginName> = Vec::with_capacity(required.len());
    for name in load_order {
        if required.contains(name) {
            ordered.push(name.clone());
        }
    }

    if let Some(missing) = required.iter().find(|name| !ordered.contains(name)) {
        let required_by = sources
            .iter()
            .find(|(_, plugin)| plugin.masters.masters().contains(missing))
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| missing.clone());
        return Err(MasterError::NotInLoadOrder {
            master: missing.clone(),
            required_by,
        });
    }

    // The merged plugin's own records use index masters.len(), so the list
    // itself must stay below the 255 addressable mod indices.
    if ordered.len() >= u8::MAX as usize {
        return Err(MasterError::TooManyMasters {
            count: ordered.len(),
        });
    }

    Ok(MasterTable::new(ordered))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{Entry, Group, GroupType, Record, Subrecord};

    fn plugin_with_masters(masters: &[&str]) -> Plugin {
        Plugin {
            header: Record::new(b"TES4", crate::plugin::FormId(0), Vec::new()),
            masters: MasterTable::new(masters.iter().map(|m| PluginName::new(*m)).collect()),
            entries: vec![Entry::Group(Group::new(
                GroupType::TopLevel(*b"STAT"),
                vec![Entry::Record(Record::new(
                    b"STAT",
                    crate::plugin::FormId(0x0100_0801),
                    vec![Subrecord::new(b"EDID", b"x\0".to_vec())],
                ))],
            ))],
        }
    }

    fn names(table: &MasterTable) -> Vec<String> {
        table
            .masters()
            .iter()
            .map(|m| m.as_str().to_string())
            .collect()
    }

    #[test]
    fn unions_masters_and_orders_them_by_load_order() {
        let sources = vec![
            ("b.esp".into(), plugin_with_masters(&["Oblivion.esm", "Knights.esp"])),
            ("c.esp".into(), plugin_with_masters(&["Oblivion.esm"])),
        ];
        let load_order = vec![
            "Oblivion.esm".into(),
            "Knights.esp".into(),
            "b.esp".into(),
            "c.esp".into(),
        ];
        let table = build(&sources, &load_order).unwrap();
        assert_eq!(names(&table), vec!["Oblivion.esm", "Knights.esp"]);
        assert_eq!(table.own_mod_index(), 2);
    }

    #[test]
    fn drops_masters_that_are_themselves_being_merged() {
        // The consistency patch masters the plugins it patches; once they are
        // merged into the same file they must not remain masters.
        let sources = vec![
            ("fort.esp".into(), plugin_with_masters(&["Oblivion.esm"])),
            (
                "patch.esp".into(),
                plugin_with_masters(&["Oblivion.esm", "fort.esp"]),
            ),
        ];
        let load_order = vec!["Oblivion.esm".into(), "fort.esp".into(), "patch.esp".into()];
        let table = build(&sources, &load_order).unwrap();
        assert_eq!(names(&table), vec!["Oblivion.esm"]);
    }

    #[test]
    fn master_comparison_is_case_insensitive() {
        let sources = vec![("a.esp".into(), plugin_with_masters(&["OBLIVION.ESM"]))];
        let load_order = vec!["Oblivion.esm".into(), "a.esp".into()];
        let table = build(&sources, &load_order).unwrap();
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn master_missing_from_load_order_is_an_error() {
        let sources = vec![(
            "a.esp".into(),
            plugin_with_masters(&["Oblivion.esm", "Missing.esp"]),
        )];
        let load_order = vec!["Oblivion.esm".into(), "a.esp".into()];
        let err = build(&sources, &load_order).unwrap_err();
        assert!(matches!(err, MasterError::NotInLoadOrder { .. }));
        assert!(err.to_string().contains("Missing.esp"));
    }

    #[test]
    fn rejects_more_masters_than_a_plugin_can_address() {
        let many: Vec<String> = (0..300).map(|i| format!("m{i}.esm")).collect();
        let refs: Vec<&str> = many.iter().map(String::as_str).collect();
        let sources = vec![("a.esp".into(), plugin_with_masters(&refs))];
        let mut load_order: Vec<PluginName> = many.iter().map(PluginName::new).collect();
        load_order.push("a.esp".into());
        assert!(matches!(
            build(&sources, &load_order).unwrap_err(),
            MasterError::TooManyMasters { .. }
        ));
    }
}
