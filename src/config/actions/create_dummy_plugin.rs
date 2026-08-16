//! `create_dummy_plugin`: write an empty `.esp` so Oblivion loads a BSA.
//!
//! Oblivion only reads `Foo.bsa` when a plugin named `Foo.esp` is active, so a
//! mod distributed as a bare archive -- or one whose loose files were just
//! packed by `pack_bsa` -- needs an empty plugin beside it.

use super::ActionCx;
use crate::config::schema::CreateDummyPluginAction;
use crate::plugin::{FormId, MasterTable, Plugin, PluginName, Record, Subrecord};
use crate::util::fs::normalize_relative_path;

pub(super) fn apply(action: &CreateDummyPluginAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!(
            "{}: create_dummy_plugin is only valid as a per-mod action",
            cx.owner
        );
    };

    let output = mod_target.join(normalize_relative_path(&action.output)?);

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            output = %output.display(),
            "install dry-run create_dummy_plugin action"
        );
        return Ok(());
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
    }

    let bytes = empty_plugin().to_bytes();
    std::fs::write(&output, bytes)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", output.display()))?;

    tracing::info!(owner = cx.owner, output = %output.display(), "wrote dummy plugin");
    Ok(())
}

/// A TES4 header and nothing else: no records, no groups.
fn empty_plugin() -> Plugin {
    let mut hedr = Vec::with_capacity(12);
    hedr.extend_from_slice(&1.0f32.to_le_bytes());
    // No records below the header, and no object ever allocated. 0x800 is where
    // Oblivion starts a plugin's own FormIDs.
    hedr.extend_from_slice(&0u32.to_le_bytes());
    hedr.extend_from_slice(&0x800u32.to_le_bytes());

    // Oblivion.esm as the sole master. Nothing here references it -- there are
    // no records at all -- but a plugin with an empty master list is unusual
    // enough that tools treat it as suspect, and every dummy plugin produced by
    // Wrye Bash or xEdit declares it. Costs 25 bytes.
    let master = PluginName::new("Oblivion.esm");

    let fields = vec![
        Subrecord::new(b"HEDR", hedr),
        Subrecord::new(b"CNAM", zstring("mudcrab")),
        Subrecord::new(b"SNAM", zstring("Dummy plugin so Oblivion loads the matching BSA")),
        Subrecord::new(b"MAST", zstring(master.as_str())),
        Subrecord::new(b"DATA", 0u64.to_le_bytes().to_vec()),
    ];

    Plugin {
        // No ESM flag: this is a plugin, not a master.
        header: Record::new(b"TES4", FormId::NULL, fields),
        masters: MasterTable::new(vec![master]),
        entries: Vec::new(),
    }
}

fn zstring(text: &str) -> Vec<u8> {
    let mut out = text.as_bytes().to_vec();
    out.push(0);
    out
}
