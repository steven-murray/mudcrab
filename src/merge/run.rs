//! Orchestrate a merge: load, allocate, rewrite, clobber, assemble, write.

use super::alloc::{allocate, Allocation};
use super::assemble::Collected;
use super::masters::{self, MasterError};
use super::rewrite::{rewrite_entries, RewriteError, Remapper};
use crate::plugin::{
    FormId, Plugin, PluginError, PluginName, Record, Subrecord,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("merge '{name}' lists no source plugins")]
    NoSources { name: String },

    #[error("merge '{name}': {source}")]
    Plugin { name: String, source: PluginError },

    #[error(transparent)]
    Masters(#[from] MasterError),

    #[error(transparent)]
    Rewrite(#[from] RewriteError),

    #[error(transparent)]
    Audit(#[from] super::audit::AuditError),
}

/// One plugin to merge, already located on disk.
pub struct MergeSource {
    pub plugin: PluginName,
    pub path: PathBuf,
}

pub struct MergeRequest {
    /// Merge name, used in the report and log.
    pub name: String,
    /// Output filename, e.g. `Unique Forts Merged.esp`.
    pub output: String,
    /// Sources in merge order: later ones clobber earlier ones.
    pub sources: Vec<MergeSource>,
    /// The full load order, used to order the merged master list.
    pub load_order: Vec<PluginName>,
}

/// What a merge produced, beyond the plugin itself.
#[derive(Debug, Clone)]
pub struct MergeReport {
    pub name: String,
    pub output: String,
    pub masters: Vec<String>,
    pub source_count: usize,
    pub record_count: usize,
    pub group_count: usize,
    pub remapped: usize,
    /// Records dropped because a later source defined the same FormID.
    pub clobbered: usize,
    /// References in the *sources* whose mod index ran past their own master
    /// list. Translated correctly; reported because it says the source was
    /// written by a tool that does not emit canonical indices.
    pub non_canonical_inputs: usize,
    pub next_object_id: u32,
}

pub struct MergeOutput {
    pub plugin: Plugin,
    pub report: MergeReport,
    pub allocation: Allocation,
}

pub fn run(request: &MergeRequest) -> Result<MergeOutput, MergeError> {
    if request.sources.is_empty() {
        return Err(MergeError::NoSources {
            name: request.name.clone(),
        });
    }

    // 1. Load every source, refusing any that carries plugin-name-keyed or
    //    FormID-keyed assets -- merging renames the plugin and renumbers its
    //    FormIDs, so those lookups would silently stop resolving.
    let mut loaded: Vec<(PluginName, Plugin)> = Vec::with_capacity(request.sources.len());
    for source in &request.sources {
        super::audit::audit_assets(&source.plugin, &source.path)?;
        let plugin = Plugin::read(&source.path).map_err(|err| MergeError::Plugin {
            name: request.name.clone(),
            source: err,
        })?;
        loaded.push((source.plugin.clone(), plugin));
    }

    // 2. Master list for the merged plugin.
    let merged_masters = masters::build(&loaded, &request.load_order)?;
    let merged_set: Vec<PluginName> = loaded.iter().map(|(name, _)| name.clone()).collect();

    // 3. Allocate object indices.
    let alloc_input: Vec<(PluginName, BTreeSet<u32>)> = loaded
        .iter()
        .map(|(name, plugin)| (name.clone(), plugin.own_object_indices()))
        .collect();
    let allocation = allocate(&alloc_input);

    // 4. Rewrite each source into the merged numbering, then absorb it.
    //    Absorbing keys records by post-rewrite FormID, so a later source
    //    replacing an earlier one *is* Clobber's last-writer-wins.
    let mut collected = Collected::default();
    let mut source_records = 0usize;
    let mut non_canonical_inputs = 0usize;

    for (name, plugin) in &loaded {
        let mut entries = plugin.entries.clone();
        let remapper = Remapper::new(
            name,
            &plugin.masters,
            &merged_masters,
            &merged_set,
            &allocation,
        );
        // Check the out-of-scope assumptions before committing to the output,
        // so a modlist that breaks one gets a refusal rather than a plugin
        // that loads and then misbehaves. See merge::audit.
        super::audit::audit_scripts(name, plugin, &remapper)?;

        rewrite_entries(&mut entries, &remapper)?;

        let non_canonical = remapper.non_canonical_count();
        if non_canonical > 0 {
            tracing::warn!(
                merge = %request.name,
                plugin = %name,
                references = non_canonical,
                "merge: source uses mod indices past its own master list; these mean \
                 'my own record' and were translated as such, but the source was written \
                 by a tool that does not emit canonical indices"
            );
            non_canonical_inputs += non_canonical;
        }

        source_records += plugin.records().count();
        collected.absorb(entries);
    }

    let record_count = collected.record_count();
    let entries = collected.build();

    // 5. Header.
    let next_object_id = allocation
        .highest_object_index()
        .into_iter()
        .chain(alloc_input.iter().flat_map(|(_, set)| set.iter().copied()))
        .max()
        .map(|highest| highest + 1)
        .unwrap_or(super::alloc::FIRST_FREE_OBJECT_INDEX);

    let mut plugin = Plugin {
        header: Record::new(b"TES4", FormId::NULL, Vec::new()),
        masters: merged_masters,
        entries,
    };
    let group_count = plugin.record_and_group_count() - record_count;
    plugin.header = build_header(&plugin, record_count + group_count, next_object_id, request);

    let report = MergeReport {
        name: request.name.clone(),
        output: request.output.clone(),
        masters: plugin
            .masters
            .masters()
            .iter()
            .map(|m| m.as_str().to_string())
            .collect(),
        source_count: loaded.len(),
        record_count,
        group_count,
        remapped: allocation.total_remapped(),
        clobbered: source_records.saturating_sub(record_count),
        non_canonical_inputs,
        next_object_id,
    };

    Ok(MergeOutput {
        plugin,
        report,
        allocation,
    })
}

fn build_header(
    plugin: &Plugin,
    num_records: usize,
    next_object_id: u32,
    request: &MergeRequest,
) -> Record {
    let mut hedr = Vec::with_capacity(12);
    hedr.extend_from_slice(&1.0f32.to_le_bytes());
    hedr.extend_from_slice(&(num_records as u32).to_le_bytes());
    hedr.extend_from_slice(&next_object_id.to_le_bytes());

    let mut fields = vec![
        Subrecord::new(b"HEDR", hedr),
        Subrecord::new(b"CNAM", zstring("mudcrab")),
        Subrecord::new(
            b"SNAM",
            zstring(&format!(
                "Merged from {} plugins by mudcrab",
                request.sources.len()
            )),
        ),
    ];

    for master in plugin.masters.masters() {
        fields.push(Subrecord::new(b"MAST", zstring(master.as_str())));
        fields.push(Subrecord::new(b"DATA", 0u64.to_le_bytes().to_vec()));
    }

    // No ESM flag: the merged output is a plugin, not a master.
    Record::new(b"TES4", FormId::NULL, fields)
}

fn zstring(text: &str) -> Vec<u8> {
    let mut out = text.as_bytes().to_vec();
    out.push(0);
    out
}

/// zMerge-compatible `map.json`: `{ "<plugin>": { "OLDHEX": "NEWHEX" } }`.
///
/// Emitted in zEdit's exact shape so it can be diffed directly against a real
/// zMerge run.
pub fn map_json(allocation: &Allocation) -> String {
    let mut out = String::from("{\n");
    let entries: Vec<_> = allocation.iter().collect();
    for (index, (plugin, remaps)) in entries.iter().enumerate() {
        out.push_str(&format!("  {:?}: {{", plugin.as_str()));
        if remaps.is_empty() {
            out.push('}');
        } else {
            out.push('\n');
            let pairs: Vec<String> = remaps
                .iter()
                .map(|(old, new)| format!("    \"{old:06X}\": \"{new:06X}\""))
                .collect();
            out.push_str(&pairs.join(",\n"));
            out.push_str("\n  }");
        }
        if index + 1 < entries.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("}\n");
    out
}
